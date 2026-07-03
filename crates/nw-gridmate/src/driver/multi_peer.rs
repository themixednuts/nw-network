//! Multi-peer DTLS listener.
//!
//! [`MultiPeerListener`] owns one bound UDP socket and runs a
//! demux loop that routes inbound datagrams by source address into
//! per-peer SSL sessions. Each new peer's first ClientHello spawns a
//! per-peer task that drives `SSL_accept` to completion and then
//! reads decrypted plaintext until the connection ends.
//!
//! Compared to [`super::secure_connection::SecureSocketListener`] (which
//! consumes the listener after one accept and is only safe with a
//! single peer), this listener handles many concurrent peers:
//!
//! - Inbound datagrams: a top-level `recv_from` loop matches
//!   `peer_addr` against an `Arc<Mutex<HashMap<...>>>` of known
//!   peers and forwards into that peer's `Sender<Bytes>` queue.
//! - Outbound: each peer's task uses the same shared
//!   `Arc<Async<UdpSocket>>` for `send_to(plaintext)` — UDP allows
//!   concurrent sends from one socket.
//! - Lifecycle: established / data / disconnect events fan into a
//!   single application-facing [`MultiPeerEvent`] channel.
//!
//! The application drives the listener via [`MultiPeerListener::next_event`]
//! and [`MultiPeerListener::send_to`]. Bevy integration (Resource +
//! drain system) lives in the embedding application.

#![cfg(feature = "server")]

use crate::driver::DriverError;
use crate::driver::secure_connection::{
    MemBio, bio_drain, bio_feed, bio_new_mem_pair, build_server_ssl_ctx,
};
use async_channel::{Receiver, Sender, TrySendError, bounded};
use async_io::Async;
use bytes::Bytes;
use dashmap::DashMap;
use foreign_types::ForeignType;
use openssl::ssl::{Ssl, SslContext};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use tracing::{debug, trace, warn};

const SSL_ERROR_WANT_READ: i32 = 2;
const SSL_ERROR_WANT_WRITE: i32 = 3;

/// Lifecycle events fanned out by the DTLS listener for every peer.
///
/// Internal to gridmate — application code consumes the higher-level
/// [`crate::session_service::Event`] via
/// [`crate::ServerListenerHandle`], which translates this enum and
/// adds carrier-level state on top.
#[derive(Debug, Clone)]
pub(crate) enum MultiPeerEvent {
    /// A peer's `SSL_accept` returned 1 — DTLS handshake complete.
    Established { peer_addr: SocketAddr },
    /// Plaintext bytes from a peer (one DTLS application record per
    /// event, modulo SSL coalescing).
    Data {
        peer_addr: SocketAddr,
        plaintext: Bytes,
    },
    /// Peer's task ended (clean close, SSL error, or task abort).
    Disconnected {
        peer_addr: SocketAddr,
        reason: String,
    },
    /// Listener-level error (e.g. socket recv failure).
    Error { description: String },
}

/// Shared per-peer state — the demux task pushes inbound bytes into
/// `inbound_tx`, and `send_to` pushes outbound plaintext into
/// `outbound_tx`. The peer task selects on both.
struct PeerHandle {
    inbound_tx: Sender<Bytes>,
    outbound_tx: Sender<Bytes>,
}

/// Lock-free peer table. Inbound demux looks up the handle on every
/// datagram (high-frequency hot path) and `send_to` looks it up on
/// every outbound write; at MMO scale a `Mutex<HashMap>` would
/// serialise both. `DashMap` shards per-bucket so demux + per-peer
/// SSL_write scale with cores.
type PeerMap = Arc<DashMap<SocketAddr, PeerHandle>>;

/// DTLS listener that supports many concurrent peers over one UDP
/// socket via software demux.
///
/// Constructed via [`MultiPeerListener::bind`]; consumed by the
/// caller via [`MultiPeerListener::next_event`] (lifecycle events)
/// and [`MultiPeerListener::send_to`] (outbound plaintext).
///
/// The demux task is detached on the embedder-registered spawner
/// (see [`crate::spawn::set_spawner`]). Lifecycle is gated by
/// `_shutdown_tx`: when the listener drops, the channel closes and
/// the demux loop observes the close on its select branch and
/// exits.
pub struct MultiPeerListener {
    socket: Arc<Async<UdpSocket>>,
    peers: PeerMap,
    event_rx: Receiver<MultiPeerEvent>,
    /// Held only for its Drop side-effect: closing the channel is the
    /// demux task's exit signal. See [`crate::spawn::ShutdownSignal`].
    _shutdown: crate::spawn::ShutdownSignal,
}

impl MultiPeerListener {
    /// Bind the UDP socket, build the server SSL context, and start
    /// the demux task. Returns once the listener is ready to accept
    /// peers; the actual accept work happens in spawned tasks.
    pub async fn bind(addr: &str, cert_pem: &str, key_pem: &str) -> Result<Self, DriverError> {
        let bind_addr: SocketAddr = addr
            .parse()
            .map_err(|e| DriverError::Address(format!("Invalid address: {e}")))?;
        let std_socket = UdpSocket::bind(bind_addr)?;
        // Windows-only: disable `WSAECONNRESET` propagation. Without
        // this, the OS reflects every ICMP "port unreachable" we
        // receive (because we sent a keepalive to a peer that has
        // closed its socket) into the *next* `recv_from`, which then
        // returns `ConnectionReset`. async-io surfaces that as a
        // hard error and the demux loop exits — but because the
        // error is silently consumed by the polling layer in some
        // codepaths, the symptom users see is "the listener stops
        // accepting new peers after the first one disconnects".
        disable_udp_connreset(&std_socket);
        let socket = Arc::new(Async::new(std_socket)?);
        let ssl_ctx = Arc::new(build_server_ssl_ctx(cert_pem, key_pem)?);

        let peers: PeerMap = Arc::new(DashMap::new());
        // DTLS-level lifecycle events; aggregates across all peers so
        // size for burst tolerance, not per-peer scale.
        const LISTENER_EVENT_CAPACITY: usize = 4096;
        let (event_tx, event_rx) = bounded::<MultiPeerEvent>(LISTENER_EVENT_CAPACITY);
        let (shutdown, shutdown_rx) = crate::spawn::ShutdownSignal::new();

        crate::spawn::spawn_detached(demux_loop(
            socket.clone(),
            ssl_ctx,
            peers.clone(),
            event_tx,
            shutdown_rx,
        ));

        Ok(Self {
            socket,
            peers,
            event_rx,
            _shutdown: shutdown,
        })
    }

    /// Local address the demux socket is bound to. Useful when
    /// binding to port 0 to discover the OS-chosen port.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.get_ref().local_addr()
    }

    /// Next lifecycle event. Returns `None` when the demux task has
    /// stopped (listener dropped). Cancel-safe. Internal —
    /// The server event bridge is the only consumer.
    pub(crate) async fn next_event(&self) -> Option<MultiPeerEvent> {
        self.event_rx.recv().await.ok()
    }

    /// Send plaintext to an established peer. The bytes go into the
    /// peer's task, which `SSL_write`s and flushes to the shared
    /// socket. Returns `Err` if the peer is unknown or its task
    /// has ended.
    ///
    /// Lock-free peer lookup; the outbound channel push may still
    /// `.await` if the per-peer SSL_write queue is full (backpressure).
    pub async fn send_to(
        &self,
        peer_addr: SocketAddr,
        plaintext: Bytes,
    ) -> Result<(), DriverError> {
        // Clone the sender out of the dashmap entry so we don't hold
        // the shard guard across the `.await`.
        let outbound_tx = {
            let Some(handle) = self.peers.get(&peer_addr) else {
                return Err(DriverError::Ssl(format!("unknown peer {peer_addr}")));
            };
            handle.outbound_tx.clone()
        };
        outbound_tx
            .send(plaintext)
            .await
            .map_err(|e| DriverError::Ssl(format!("peer {peer_addr} closed: {e}")))
    }

    /// Sync, non-blocking variant. Returns `Err` if the peer is
    /// unknown or its outbound queue is full. Used by the carrier
    /// driver task to push without spawning a forwarder.
    pub fn try_send_to(&self, peer_addr: SocketAddr, plaintext: Bytes) -> Result<(), DriverError> {
        let Some(handle) = self.peers.get(&peer_addr) else {
            return Err(DriverError::Ssl(format!("unknown peer {peer_addr}")));
        };
        handle.outbound_tx.try_send(plaintext).map_err(|e| match e {
            TrySendError::Full(_) => {
                DriverError::Ssl(format!("peer {peer_addr} outbound queue full"))
            }
            TrySendError::Closed(_) => {
                DriverError::Ssl(format!("peer {peer_addr} outbound queue closed"))
            }
        })
    }
}

/// Top-level demux: read every incoming datagram, route by
/// `peer_addr`. New peers get a fresh per-peer task spawned on the
/// embedder-selected executor.
///
/// `shutdown_rx` is the listener's exit signal — when the listener
/// drops, its `_shutdown_tx` closes the channel and the select
/// branch wakes, returning the demux from its blocking
/// `recv_from`.
/// Windows: clear `SIO_UDP_CONNRESET` on the bound UDP socket so that
/// an outbound datagram to a closed peer port doesn't poison the
/// next `recv_from` with `WSAECONNRESET`. The default-on behaviour
/// is a Windows-only convenience for clients (so they learn the
/// server is gone); for a *server* socket it's a footgun — a peer's
/// disappearance silently breaks reception for every other peer.
///
/// Best-effort: if the ioctl fails we log and continue. Real-world
/// failure modes are exotic (the kernel always supports this ioctl
/// on UDP sockets since Windows 2000), but the listener still
/// functions if the call returns an error — it just becomes
/// vulnerable to the original bug.
fn disable_udp_connreset(_socket: &UdpSocket) {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Networking::WinSock::{SIO_UDP_CONNRESET, SOCKET, WSAIoctl};

        let raw = _socket.as_raw_socket() as SOCKET;
        let mut disable: u32 = 0;
        let mut returned: u32 = 0;
        let rc = unsafe {
            WSAIoctl(
                raw,
                SIO_UDP_CONNRESET,
                &mut disable as *mut _ as *mut _,
                std::mem::size_of_val(&disable) as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
                None,
            )
        };
        if rc != 0 {
            warn!(
                "disable_udp_connreset: WSAIoctl(SIO_UDP_CONNRESET, FALSE) returned {rc}; \
                 demux may silently stall after first peer disconnect"
            );
        } else {
            debug!("disable_udp_connreset: SIO_UDP_CONNRESET cleared on listener socket");
        }
    }
}

async fn demux_loop(
    socket: Arc<Async<UdpSocket>>,
    ssl_ctx: Arc<SslContext>,
    peers: PeerMap,
    event_tx: Sender<MultiPeerEvent>,
    shutdown_rx: Receiver<()>,
) {
    // Refcounted recv ring: `recv_from` writes directly into the
    // ring's uninit tail and `commit` splits the just-received
    // datagram off as an `Arc`-shared `Bytes` — no per-datagram
    // allocation, no `copy_from_slice` on the inbound hot path.
    // 4 MB ring × 64 KB max datagram ≈ 64 datagrams between
    // amortised rollover allocations.
    const RECV_BUF_SIZE: usize = 65536;
    const RING_CHUNK_SIZE: usize = 64 * RECV_BUF_SIZE;
    let mut ring = super::recv_ring::RecvRing::new(RING_CHUNK_SIZE, RECV_BUF_SIZE);
    loop {
        enum DemuxStep {
            Datagram(std::io::Result<(usize, SocketAddr)>),
            Shutdown,
        }
        let (len, peer_addr) = {
            let slot = ring.recv_slot();
            let step = futures_lite::future::or(
                async { DemuxStep::Datagram(socket.recv_from(slot).await) },
                async {
                    let _ = shutdown_rx.recv().await;
                    DemuxStep::Shutdown
                },
            )
            .await;
            match step {
                DemuxStep::Datagram(Ok(v)) => v,
                DemuxStep::Datagram(Err(err)) => {
                    let _ = event_tx
                        .send(MultiPeerEvent::Error {
                            description: format!("recv_from: {err}"),
                        })
                        .await;
                    return;
                }
                DemuxStep::Shutdown => {
                    debug!("MultiPeerListener: demux shutdown signal observed; exiting");
                    return;
                }
            }
        };
        let datagram = ring.commit(len);

        // Lock-free clone of the inbound sender. Drop the dashmap
        // guard before `await` so we never hold a shard lock across
        // suspension.
        let inbound_tx = peers.get(&peer_addr).map(|h| h.inbound_tx.clone());

        if let Some(inbound_tx) = inbound_tx {
            // Existing peer: forward. If the peer task has died, the
            // send fails — drop it from the map on the next cleanup.
            if inbound_tx.send(datagram).await.is_err() {
                trace!(?peer_addr, "demux: peer_task gone, dropping entry");
                peers.remove(&peer_addr);
            }
            continue;
        }
        trace!(?peer_addr, "demux: new peer — spawning peer_task");

        // New peer: build per-peer channels, register, spawn task.
        // Bounded — slow consumer triggers `.send().await` backpressure
        // rather than unbounded memory growth.
        const PER_PEER_DTLS_QUEUE: usize = 256;
        let (inbound_tx, inbound_rx) = bounded::<Bytes>(PER_PEER_DTLS_QUEUE);
        let (outbound_tx, outbound_rx) = bounded::<Bytes>(PER_PEER_DTLS_QUEUE);
        peers.insert(
            peer_addr,
            PeerHandle {
                inbound_tx: inbound_tx.clone(),
                outbound_tx,
            },
        );
        // Seed the peer task with the ClientHello we just received.
        let _ = inbound_tx.send(datagram).await;

        crate::spawn::spawn_detached(peer_task(
            peer_addr,
            ssl_ctx.clone(),
            socket.clone(),
            inbound_rx,
            outbound_rx,
            event_tx.clone(),
            peers.clone(),
        ));
    }
}

/// Per-peer SSL session driver. Handles SSL_accept, then loops on
/// (inbound encrypted bytes → SSL_read → plaintext event) and
/// (outbound plaintext request → SSL_write → encrypted on the wire).
async fn peer_task(
    peer_addr: SocketAddr,
    ssl_ctx: Arc<SslContext>,
    socket: Arc<Async<UdpSocket>>,
    inbound_rx: Receiver<Bytes>,
    outbound_rx: Receiver<Bytes>,
    event_tx: Sender<MultiPeerEvent>,
    peers: PeerMap,
) {
    let result = run_peer(
        peer_addr,
        &ssl_ctx,
        &socket,
        &inbound_rx,
        &outbound_rx,
        &event_tx,
    )
    .await;

    // Always clean up the peer entry so a reconnecting client gets
    // a fresh SSL session.
    peers.remove(&peer_addr);

    match result {
        Ok(()) => {
            let _ = event_tx
                .send(MultiPeerEvent::Disconnected {
                    peer_addr,
                    reason: "peer task ended".into(),
                })
                .await;
        }
        Err(reason) => {
            let _ = event_tx
                .send(MultiPeerEvent::Disconnected { peer_addr, reason })
                .await;
        }
    }
}

async fn run_peer(
    peer_addr: SocketAddr,
    ssl_ctx: &SslContext,
    socket: &Async<UdpSocket>,
    inbound_rx: &Receiver<Bytes>,
    outbound_rx: &Receiver<Bytes>,
    event_tx: &Sender<MultiPeerEvent>,
) -> Result<(), String> {
    let mut ssl = Ssl::new(ssl_ctx).map_err(|e| format!("Ssl::new: {e}"))?;
    ssl.set_mtu(1200).ok();
    let (read_bio, write_bio) = bio_new_mem_pair().map_err(|e| format!("bio_new: {e}"))?;
    unsafe {
        openssl_sys::SSL_set_bio(ssl.as_ptr(), read_bio.as_ptr(), write_bio.as_ptr());
    }
    ssl.set_accept_state();

    // Drive SSL_accept until it returns 1, eating from inbound_rx
    // and flushing write_bio after each call.
    accept_handshake(&ssl, read_bio, write_bio, peer_addr, socket, inbound_rx).await?;

    debug!("MultiPeerListener: SSL_accept complete for {peer_addr}");
    let _ = event_tx
        .send(MultiPeerEvent::Established { peer_addr })
        .await;

    // Steady-state loop: select on inbound encrypted bytes and
    // outbound plaintext requests. `or` resolves whichever future
    // wakes first; both branches normalise to the same `Event`
    // enum so the combinator's `Output` type matches.
    //
    // `recv_ring` is the per-peer SSL_read sink: `SSL_read` writes
    // decrypted plaintext directly into the ring's tail and we hand
    // out an `Arc`-shared `Bytes` to the bridge via
    // `MultiPeerEvent::Data` — no `copy_from_slice` per record.
    //
    // Sized small (4 × max_record = 256 KB) so a 5–10 k-peer server
    // fits the total recv-ring footprint in 1–2.5 GB. The ring also
    // lazy-allocates on first recv, so idle peers cost zero.
    const SSL_RECV_BUF: usize = 65536;
    const SSL_RING_CHUNK: usize = 4 * SSL_RECV_BUF;
    let mut recv_ring = super::recv_ring::RecvRing::new(SSL_RING_CHUNK, SSL_RECV_BUF);
    loop {
        let event =
            futures_lite::future::or(async { Event::Inbound(inbound_rx.recv().await) }, async {
                Event::Outbound(outbound_rx.recv().await)
            })
            .await;

        match event {
            Event::Inbound(Ok(datagram)) => {
                bio_feed(read_bio, &datagram).map_err(|e| format!("bio_feed: {e}"))?;
                drain_decrypted(&ssl, &mut recv_ring, peer_addr, event_tx).await?;
            }
            Event::Inbound(Err(_)) => return Ok(()),
            Event::Outbound(Ok(plaintext)) => {
                let n = unsafe {
                    openssl_sys::SSL_write(
                        ssl.as_ptr(),
                        plaintext.as_ptr() as *const _,
                        plaintext.len() as i32,
                    )
                };
                if n <= 0 {
                    let err = unsafe { openssl_sys::SSL_get_error(ssl.as_ptr(), n) };
                    return Err(format!("SSL_write {err}"));
                }
                let outgoing = bio_drain(write_bio);
                if !outgoing.is_empty() {
                    socket
                        .send_to(outgoing.as_ref(), peer_addr)
                        .await
                        .map_err(|e| format!("send_to: {e}"))?;
                }
            }
            Event::Outbound(Err(_)) => return Ok(()),
        }
    }
}

enum Event {
    Inbound(Result<Bytes, async_channel::RecvError>),
    Outbound(Result<Bytes, async_channel::RecvError>),
}

async fn accept_handshake(
    ssl: &Ssl,
    read_bio: MemBio,
    write_bio: MemBio,
    peer_addr: SocketAddr,
    socket: &Async<UdpSocket>,
    inbound_rx: &Receiver<Bytes>,
) -> Result<(), String> {
    loop {
        let ret = unsafe { openssl_sys::SSL_accept(ssl.as_ptr()) };

        let outgoing = bio_drain(write_bio);
        if !outgoing.is_empty() {
            socket
                .send_to(outgoing.as_ref(), peer_addr)
                .await
                .map_err(|e| format!("send_to: {e}"))?;
        }

        if ret == 1 {
            return Ok(());
        }
        let err = unsafe { openssl_sys::SSL_get_error(ssl.as_ptr(), ret) };
        if err != SSL_ERROR_WANT_READ && err != SSL_ERROR_WANT_WRITE {
            // Drain the entire ERR queue (each SSL_accept failure can
            // queue several reason codes — the *first* one is usually
            // the actual handshake alert, the rest are propagation
            // wrappers from OpenSSL's call stack). The numeric pair
            // alone (`error 1 (0xa00041b)`) was opaque; passing the
            // queue through the safe `openssl::error::ErrorStack`
            // wrapper yields strings like `error:0A00041B:SSL routines:
            // :tlsv1 alert decrypt error` so we can tell a launcher
            // cert-pin rejection from a cipher-mismatch from a
            // record-layer version mismatch.
            let stack = openssl::error::ErrorStack::get();
            let joined = if stack.errors().is_empty() {
                "<empty ERR queue>".to_string()
            } else {
                stack
                    .errors()
                    .iter()
                    .map(|e| format!("{e}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            return Err(format!("SSL_accept error {err}: {joined}"));
        }

        let datagram = match inbound_rx.recv().await {
            Ok(d) => d,
            Err(_) => return Err("inbound channel closed during handshake".into()),
        };
        bio_feed(read_bio, &datagram).map_err(|e| format!("bio_feed: {e}"))?;
    }
}

async fn drain_decrypted(
    ssl: &Ssl,
    recv_ring: &mut super::recv_ring::RecvRing,
    peer_addr: SocketAddr,
    event_tx: &Sender<MultiPeerEvent>,
) -> Result<(), String> {
    loop {
        let n;
        let plaintext = {
            let slot = recv_ring.recv_slot();
            n = unsafe {
                openssl_sys::SSL_read(ssl.as_ptr(), slot.as_mut_ptr() as *mut _, slot.len() as i32)
            };
            if n <= 0 {
                // SSL_read returned 0 or an error code; do not
                // commit. Fall through to the error-classification
                // block below.
                Bytes::new()
            } else {
                recv_ring.commit(n as usize)
            }
        };
        if n > 0 {
            trace!(?peer_addr, len = n, "MultiPeerListener: decrypted record");
            if event_tx
                .send(MultiPeerEvent::Data {
                    peer_addr,
                    plaintext,
                })
                .await
                .is_err()
            {
                return Ok(()); // listener dropped
            }
            continue;
        }
        let err = unsafe { openssl_sys::SSL_get_error(ssl.as_ptr(), n) };
        if err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE {
            return Ok(());
        }
        let code = unsafe { openssl_sys::ERR_get_error() };
        warn!(?peer_addr, err, code, "MultiPeerListener: SSL_read fatal");
        return Err(format!("SSL_read {err} (0x{code:x})"));
    }
}
