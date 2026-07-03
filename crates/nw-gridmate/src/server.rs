//! Server-side multi-peer listener with per-peer carrier drivers.
//!
//! Wraps [`crate::driver::MultiPeerListener`] (DTLS demux) and owns the
//! GridMate carrier protocol driver for every connected peer. The
//! application sees one event stream and addresses peers by
//! [`SessionId`] — the (peer_addr ↔ SessionId) mapping and per-peer
//! task wiring live entirely inside this module.
//!
//! Architecture:
//!
//! ```text
//!  client UDP datagram
//!         │
//!         ▼   shared socket
//!  MultiPeerListener            ── DTLS demux, per-peer SSL session
//!         │
//!         ▼   MultiPeerEvent::{Established, Data, …}
//!  event_bridge_loop  (this module)
//!         │
//!         ▼   per-peer (Sender<Bytes>, Receiver<OutboundTyped>) pair
//!  CarrierImpl<Connecting>::accept(ChannelTransport{…})
//!         │   internally: SM_CONNECT_REQUEST → SM_CONNECT_ACK
//!         ▼   .ready() → CarrierImpl<Connected>
//!  run_peer_session  (this module)
//!         │
//!         ▼   CarrierEvent::MessageReceived → envelope parsed
//!  Event::TypedReceived  → application
//! ```

#![cfg(feature = "server")]

use crate::carrier::io::ChannelTransport;
use crate::carrier::thread_message::CarrierEvent;
use crate::carrier::{
    CarrierImpl, CarrierProtocolProfile, DataReliability, MAX_CHANNELS, SYSTEM_CHANNEL,
};
use crate::driver::{DriverError, MultiPeerEvent, MultiPeerListener};
use crate::message::{Sendable, ServerToClient, read_correlated_message, read_sized_message};
use crate::serialize::buffer::{CARRIER_ENDIAN, ReadBuffer};
use crate::serialize::error::MarshalerError;
use crate::session::CarrierChannel;
use crate::session_service::{Event, OutboundSink, Outgoing, SessionId};
use async_channel::{Receiver, Sender, bounded};
use bytes::Bytes;
use dashmap::DashMap;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, trace, warn};

// (No separate per-peer event type: the server emits the
// unified [`Event`] enum, the same shape the client-side
// `SessionServiceHandle` emits.)

/// Outbound command queued by application code for a specific peer.
/// The peer's carrier driver task pulls these and turns them into
/// `CarrierImpl<Connected>::send` calls.
///
/// `envelope` carries the **full** ServerToClient wire bytes; prefer
/// the typed [`ServerListenerHandle::send`] builder over hand-building
/// one of these.
#[derive(Debug, Clone)]
pub struct OutboundTyped {
    pub channel: u8,
    pub reliability: DataReliability,
    pub priority: usize,
    pub envelope: Bytes,
}

/// Server listener handle: bound socket + per-peer carrier drivers,
/// presented to the application as one event stream keyed by
/// [`SessionId`].
#[derive(Clone)]
pub struct ServerListenerHandle {
    inner: Arc<ServerListenerInner>,
}

struct ServerListenerInner {
    local_addr: SocketAddr,
    event_rx: Receiver<Event>,
    peers: Arc<PeerRegistry>,
    /// Bridge-task exit signal. The bridge captures
    /// `Arc<MultiPeerListener>` plus per-peer carrier-driver tasks
    /// each capture a clone, forming an Arc cycle that keeps the
    /// listener alive on its own. Dropping this `ShutdownSignal`
    /// closes the bridge's exit channel, short-circuiting the
    /// `next_event().await` select branch so the chain unwinds.
    _bridge_shutdown: crate::spawn::ShutdownSignal,
}

/// Lock-free peer registry. Every inbound datagram looks up the
/// peer by `SocketAddr` (demux), and every outbound `send_typed`
/// looks up by `SessionId` (application routing). At MMO scale
/// (1000+ peers × 100s of packets/sec/peer) a single mutex around
/// a HashMap would serialise the hottest paths in the system —
/// `DashMap` shards the table per-bucket so demux scales linearly
/// with cores. Session allocation goes through an atomic counter,
/// not a lock.
struct PeerRegistry {
    by_addr: DashMap<SocketAddr, PeerEntry>,
    by_session: DashMap<SessionId, SocketAddr>,
    next_session: AtomicUsize,
}

impl PeerRegistry {
    fn new() -> Self {
        Self {
            by_addr: DashMap::new(),
            by_session: DashMap::new(),
            next_session: AtomicUsize::new(0),
        }
    }

    fn register(&self, peer_addr: SocketAddr, outbound: Sender<OutboundTyped>) -> SessionId {
        let session = SessionId::new(self.next_session.fetch_add(1, Ordering::Relaxed));
        self.by_addr
            .insert(peer_addr, PeerEntry { session, outbound });
        self.by_session.insert(session, peer_addr);
        session
    }

    fn remove(&self, peer_addr: SocketAddr) -> Option<SessionId> {
        let (_, entry) = self.by_addr.remove(&peer_addr)?;
        self.by_session.remove(&entry.session);
        Some(entry.session)
    }

    fn outbound_for(&self, session: SessionId) -> Option<Sender<OutboundTyped>> {
        let peer_addr = *self.by_session.get(&session)?;
        self.by_addr.get(&peer_addr).map(|e| e.outbound.clone())
    }

    fn session_for(&self, peer_addr: SocketAddr) -> Option<SessionId> {
        self.by_addr.get(&peer_addr).map(|e| e.session)
    }

    fn peer_addr_for(&self, session: SessionId) -> Option<SocketAddr> {
        self.by_session.get(&session).map(|v| *v)
    }

    fn len(&self) -> usize {
        self.by_addr.len()
    }
}

struct PeerEntry {
    session: SessionId,
    outbound: Sender<OutboundTyped>,
}

impl ServerListenerHandle {
    /// Bind the UDP socket, start the DTLS demux, and start spawning
    /// per-peer carrier drivers as peers complete the DTLS handshake.
    pub async fn bind(addr: &str, cert_pem: &str, key_pem: &str) -> Result<Self, DriverError> {
        Self::bind_with_protocol_profile(addr, cert_pem, key_pem, CarrierProtocolProfile::default())
            .await
    }

    /// Bind with an explicit carrier wire profile.
    pub async fn bind_with_protocol_profile(
        addr: &str,
        cert_pem: &str,
        key_pem: &str,
        protocol: CarrierProtocolProfile,
    ) -> Result<Self, DriverError> {
        let listener = Arc::new(MultiPeerListener::bind(addr, cert_pem, key_pem).await?);
        let local_addr = listener.local_addr()?;
        // Aggregate event channel across all peers — sized so a stalled
        // consumer doesn't immediately backpressure the demux loop.
        const SERVER_EVENT_CAPACITY: usize = 4096;
        let (event_tx, event_rx) = bounded::<Event>(SERVER_EVENT_CAPACITY);
        let peers = Arc::new(PeerRegistry::new());

        let (bridge_shutdown, bridge_shutdown_rx) = crate::spawn::ShutdownSignal::new();
        crate::spawn::spawn_detached(event_bridge_loop(
            listener.clone(),
            event_tx,
            peers.clone(),
            protocol,
            bridge_shutdown_rx,
        ));

        Ok(Self {
            inner: Arc::new(ServerListenerInner {
                local_addr,
                event_rx,
                peers,
                _bridge_shutdown: bridge_shutdown,
            }),
        })
    }

    /// Local address the listener is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    /// Drain the next event, awaiting if none is available. Returns
    /// `None` when the bridge task ends.
    pub async fn next_event(&self) -> Option<Event> {
        self.inner.event_rx.recv().await.ok()
    }

    /// Non-blocking drain. Returns `Ok(None)` if the queue is empty.
    pub fn try_next_event(&self) -> Option<Event> {
        self.inner.event_rx.try_recv().ok()
    }

    /// Enqueue a raw outbound command for a peer.
    pub async fn send_typed(&self, session: SessionId, cmd: OutboundTyped) -> Result<(), String> {
        let Some(outbound) = self.inner.peers.outbound_for(session) else {
            return Err(format!("unknown session {session:?}"));
        };
        outbound
            .send(cmd)
            .await
            .map_err(|e| format!("outbound queue: {e}"))
    }

    /// Look up the [`SessionId`] for a peer address. Useful for
    /// applications that retain `SocketAddr`-keyed bookkeeping.
    /// Lock-free; safe to call from any thread or schedule slot.
    pub fn session_for(&self, peer_addr: SocketAddr) -> Option<SessionId> {
        self.inner.peers.session_for(peer_addr)
    }

    /// Look up the peer address bound to a [`SessionId`]. Inverse of
    /// [`Self::session_for`]. Lock-free.
    pub fn peer_addr_for(&self, session: SessionId) -> Option<SocketAddr> {
        self.inner.peers.peer_addr_for(session)
    }

    /// Number of currently-connected peers. Lock-free.
    pub fn peer_count(&self) -> usize {
        self.inner.peers.len()
    }

    /// Typed server send: encode `msg` as a ServerToClient Hub envelope
    /// and enqueue it on the peer's outbound queue.
    pub fn send<M>(
        &self,
        session: SessionId,
        msg: M,
    ) -> Outgoing<'_, M, ServerListenerHandle, ServerToClient>
    where
        M: Sendable<ServerToClient>,
    {
        Outgoing::new(self, session, msg)
    }
}

impl OutboundSink for ServerListenerHandle {
    async fn enqueue(
        &self,
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: DataReliability,
        priority: usize,
    ) -> crate::Result<()> {
        let Some(outbound) = self.inner.peers.outbound_for(session) else {
            return Err(crate::GridMateError::Channel(format!(
                "unknown session {session:?}"
            )));
        };
        outbound
            .send(OutboundTyped {
                channel,
                reliability,
                priority,
                envelope: bytes,
            })
            .await
            .map_err(|e| crate::GridMateError::Channel(format!("outbound queue: {e}")))
    }

    fn try_enqueue(
        &self,
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: DataReliability,
        priority: usize,
    ) -> crate::Result<()> {
        let Some(outbound) = self.inner.peers.outbound_for(session) else {
            return Err(crate::GridMateError::Channel(format!(
                "unknown session {session:?}"
            )));
        };
        outbound
            .try_send(OutboundTyped {
                channel,
                reliability,
                priority,
                envelope: bytes,
            })
            .map_err(|e| {
                if e.is_full() {
                    crate::GridMateError::Channel(format!("outbound queue for {session:?} is full"))
                } else {
                    crate::GridMateError::ConnectionClosed
                }
            })
    }
}

/// Bridge loop: pulls `MultiPeerEvent`s and either (a) spawns a
/// per-peer carrier driver on `Established`, or (b) forwards Data /
/// Disconnected into the carrier driver's inbound queue + emits a
/// [`Event`] for the application.
///
/// `shutdown_rx` lets the parent [`ServerListenerHandle`] break the
/// Arc cycle on drop — without it the bridge would hold the listener
/// alive forever.
async fn event_bridge_loop(
    listener: Arc<MultiPeerListener>,
    event_tx: Sender<Event>,
    peers: Arc<PeerRegistry>,
    protocol: CarrierProtocolProfile,
    shutdown_rx: Receiver<()>,
) {
    // Per-peer plaintext sender (demux → carrier).
    let mut peer_inbound: HashMap<SocketAddr, Sender<Bytes>> = HashMap::new();

    loop {
        enum BridgeStep {
            Event(Option<crate::driver::MultiPeerEvent>),
            Shutdown,
        }
        let step = futures_lite::future::or(
            async { BridgeStep::Event(listener.next_event().await) },
            async {
                let _ = shutdown_rx.recv().await;
                BridgeStep::Shutdown
            },
        )
        .await;
        let event = match step {
            BridgeStep::Event(Some(ev)) => ev,
            BridgeStep::Event(None) => return,
            BridgeStep::Shutdown => {
                debug!("ServerListener: bridge shutdown signal observed; exiting");
                return;
            }
        };
        match event {
            MultiPeerEvent::Established { peer_addr } => {
                let (inbound_tx, outbound_tx) = spawn_peer_carrier(
                    peer_addr,
                    listener.clone(),
                    event_tx.clone(),
                    peers.clone(),
                    protocol,
                );
                peer_inbound.insert(peer_addr, inbound_tx);
                let session = peers.register(peer_addr, outbound_tx);
                tracing::info!(?peer_addr, ?session, "[GRIDMATE-IN] peer connected");
                if event_tx.send(Event::Ready { session }).await.is_err() {
                    return;
                }
            }
            MultiPeerEvent::Data {
                peer_addr,
                plaintext,
            } => {
                if let Some(tx) = peer_inbound.get(&peer_addr)
                    && tx.send(plaintext).await.is_err()
                {
                    peer_inbound.remove(&peer_addr);
                }
            }
            MultiPeerEvent::Disconnected { peer_addr, reason } => {
                peer_inbound.remove(&peer_addr);
                let session = peers.remove(peer_addr);
                if let Some(session) = session {
                    tracing::info!(?peer_addr, ?session, %reason, "[GRIDMATE-IN] peer disconnected");
                    if event_tx
                        .send(Event::Disconnected { session, reason })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
            MultiPeerEvent::Error { description } => {
                if event_tx.send(Event::Error { description }).await.is_err() {
                    return;
                }
            }
        }
    }
}

/// Spawn the per-peer carrier driver for a peer that just finished
/// the DTLS handshake. Returns the inbound queue (for
/// `event_bridge_loop` to forward decrypted bytes into) and the
/// outbound queue (for the application to push commands into).
///
/// The carrier's outbound plaintext goes directly into the listener's
/// per-peer SSL_write queue via the `ChannelTransport`'s listener
/// reference — no per-peer forwarder task.
fn spawn_peer_carrier(
    peer_addr: SocketAddr,
    listener: Arc<MultiPeerListener>,
    event_tx: Sender<Event>,
    peers: Arc<PeerRegistry>,
    protocol: CarrierProtocolProfile,
) -> (Sender<Bytes>, Sender<OutboundTyped>) {
    // Per-peer queues. Bounded so a stalled consumer surfaces
    // backpressure via `.send().await` rather than growing memory
    // unbounded. 256 datagrams × MTU ≈ 300 KB per peer is the
    // worst-case in-flight buffer.
    const PER_PEER_QUEUE_CAPACITY: usize = 256;
    let (inbound_tx, inbound_rx) = bounded::<Bytes>(PER_PEER_QUEUE_CAPACITY);
    let (cmd_tx, cmd_rx) = bounded::<OutboundTyped>(PER_PEER_QUEUE_CAPACITY);

    // Carrier driver: handshake then steady-state loop.
    let event_tx_for_driver = event_tx.clone();
    let peers_for_driver = peers.clone();
    let listener_for_driver = listener.clone();
    crate::spawn::spawn_detached(async move {
        let io = ChannelTransport {
            peer_addr,
            inbound: inbound_rx,
            listener: listener_for_driver,
        };
        let carrier = match CarrierImpl::accept_with_protocol_profile(io, protocol) {
            Ok(c) => c,
            Err(err) => {
                warn!(?peer_addr, ?err, "CarrierImpl::accept failed");
                return;
            }
        };
        let mut connected = match carrier.ready().await {
            Ok(c) => c,
            Err(err) => {
                warn!(?peer_addr, ?err, "carrier ready failed");
                let session = peers_for_driver.remove(peer_addr);
                if let Some(session) = session {
                    let _ = event_tx_for_driver
                        .send(Event::Disconnected {
                            session,
                            reason: format!("carrier ready: {err}"),
                        })
                        .await;
                }
                return;
            }
        };

        run_peer_session(
            peer_addr,
            &mut connected,
            &cmd_rx,
            &event_tx_for_driver,
            &peers_for_driver,
        )
        .await;
    });

    (inbound_tx, cmd_tx)
}

/// Per-peer steady-state loop. Selects on the carrier's inbound
/// `recv_message` and the application's outbound `OutboundTyped`
/// commands. Translates `CarrierEvent` into [`Event`] for
/// the application.
async fn run_peer_session(
    peer_addr: SocketAddr,
    connected: &mut CarrierImpl<crate::carrier::Connected>,
    cmd_rx: &Receiver<OutboundTyped>,
    event_tx: &Sender<Event>,
    peers: &Arc<PeerRegistry>,
) {
    loop {
        let action = futures_lite::future::or(
            async { Action::Inbound(connected.recv_message().await) },
            async { Action::Outbound(cmd_rx.recv().await.ok()) },
        )
        .await;

        match action {
            Action::Inbound(Some(msg)) => {
                if !handle_inbound(peer_addr, msg, event_tx, peers).await {
                    return;
                }
            }
            Action::Inbound(None) => {
                let session = peers.remove(peer_addr);
                if let Some(session) = session {
                    let _ = event_tx
                        .send(Event::Disconnected {
                            session,
                            reason: "carrier receiver closed".into(),
                        })
                        .await;
                }
                return;
            }
            Action::Outbound(Some(cmd)) => {
                // `envelope` is a `Bytes` (Arc-shaped). Clone for the
                // post-send `Event::Sent` echo; both halves share the
                // same backing buffer — no copy on the hot path.
                let envelope = cmd.envelope.clone();
                let channel = cmd.channel;
                let reliability = cmd.reliability;
                let priority = cmd.priority;
                if let Err(err) = connected
                    .send(cmd.envelope, reliability, priority, channel)
                    .await
                {
                    warn!(?peer_addr, ?err, "carrier send failed; dropping peer");
                    let session = peers.remove(peer_addr);
                    if let Some(session) = session {
                        let _ = event_tx
                            .send(Event::Disconnected {
                                session,
                                reason: format!("send: {err}"),
                            })
                            .await;
                    }
                    return;
                }
                // Emit Sent symmetry with the client side. Look up the
                // session_id; if the peer disappeared while we held the
                // outbound cmd, skip the event (the disconnect path
                // will have already fired).
                if let Some(session) = peers.session_for(peer_addr) {
                    let type_index = parse_outbound_type_index(&envelope);
                    let _ = event_tx
                        .send(Event::Sent {
                            session,
                            data: envelope,
                            channel,
                            reliability,
                            priority,
                            type_index,
                        })
                        .await;
                }
            }
            Action::Outbound(None) => return,
        }
    }
}

enum Action {
    Inbound(Option<CarrierEvent>),
    Outbound(Option<OutboundTyped>),
}

/// Returns `false` to signal the run loop should exit.
async fn handle_inbound(
    peer_addr: SocketAddr,
    msg: CarrierEvent,
    event_tx: &Sender<Event>,
    peers: &Arc<PeerRegistry>,
) -> bool {
    let Some(session) = peers.session_for(peer_addr) else {
        return true;
    };
    match msg {
        CarrierEvent::MessageReceived { channel, data } => {
            // System / out-of-range channels are carrier control —
            // drop. Channel-0 carries replica `Cmd_*` traffic AND
            // (sometimes) direct message envelopes; route channel 0
            // separately so application code can content-demux.
            if channel == SYSTEM_CHANNEL || channel as usize >= MAX_CHANNELS {
                trace!(
                    ?peer_addr,
                    channel,
                    bytes = data.len(),
                    "carrier system/oob channel"
                );
                return true;
            }
            // Surface raw bytes alongside the parsed envelope so
            // diagnostic systems can sniff the wire.
            if event_tx
                .send(Event::Received {
                    session,
                    channel,
                    data: data.clone(),
                })
                .await
                .is_err()
            {
                return false;
            }

            let Some(channel_kind) = CarrierChannel::from_u8(channel) else {
                trace!(?peer_addr, channel, "unknown application channel");
                return true;
            };

            if !channel_kind.carries_hub_messages() {
                return true;
            }

            // Parse the ClientToServer correlated frame.
            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, data.as_ref());
            let (correlated, envelope_view) = match read_correlated_message(&mut rb) {
                Ok(parts) => parts,
                Err(MarshalerError::EmptyEnvelope) => {
                    // Keepalive / ack-only envelope, no typed body.
                    return true;
                }
                Err(err) => {
                    warn!(?peer_addr, channel, bytes = data.len(), error = ?err,
                        "failed to parse correlated IMessage envelope");
                    return true;
                }
            };
            let type_index = match envelope_view.type_id.resolved_type_index() {
                Some(type_index) => type_index,
                None => {
                    warn!(?peer_addr, channel, "unknown UUID in IMessage envelope");
                    return true;
                }
            };
            let body = Bytes::copy_from_slice(envelope_view.body);
            let _ = correlated;
            event_tx
                .send(Event::TypedReceived {
                    session,
                    channel,
                    type_index,
                    data: body,
                })
                .await
                .is_ok()
        }
        CarrierEvent::Disconnected { reason } => {
            let _ = event_tx
                .send(Event::Disconnected {
                    session,
                    reason: format!("{reason:?}"),
                })
                .await;
            false
        }
        CarrierEvent::Error { description } => {
            warn!(?peer_addr, %description, "carrier error");
            true
        }
        CarrierEvent::Connected { .. } => true,
    }
}

fn parse_outbound_type_index(data: &Bytes) -> Option<u32> {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, data.as_ref());
    let (_metadata, envelope) = read_sized_message(&mut rb).ok()?;
    envelope.type_id.resolved_type_index()
}
