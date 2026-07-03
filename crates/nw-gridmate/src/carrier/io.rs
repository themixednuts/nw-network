//! Transport abstraction for [`super::connection_state::ConnectionState`].
//!
//! Lumberyard's GridMate `Carrier` uses one `Driver` for all
//! connections — the driver is socket-shaped and the carrier just
//! `read`s and `write`s bytes. Our port collapses driver +
//! connection into [`SecureConnection`] for the initiator
//! (one DTLS socket per outbound peer) and routes through a shared
//! [`MultiPeerListener`] for the responder (multi-peer demux on one
//! shared socket).
//!
//! [`CarrierTransport`] is the seam. Both impls live alongside this
//! module; [`ConnectionState`] is generic over `T: CarrierTransport`
//! so it never matches on the transport flavor — each call to
//! `read`/`write`/`try_recv` is monomorphised against the concrete
//! transport, no `dyn` dispatch and no per-call match arms.

#[cfg(feature = "server")]
use crate::driver::MultiPeerListener;
use crate::driver::error::DriverError;
use crate::driver::{Established, SecureConnection};
#[cfg(feature = "server")]
use async_channel::{Receiver, TryRecvError};
use bytes::Bytes;
use std::future::Future;
use std::net::SocketAddr;
#[cfg(feature = "server")]
use std::sync::Arc;

/// Per-peer byte transport feeding one [`super::connection_state::ConnectionState`].
///
/// Two impls ship with gridmate: [`DtlsTransport`] for the initiator
/// (dedicated DTLS connection) and [`ChannelTransport`] for the
/// responder (per-peer queue + shared
/// [`crate::driver::MultiPeerListener`]). Embedders building on
/// custom transports implement this trait directly.
///
/// The async methods return `Send + '_` futures so the carrier
/// driver task (which is spawned on the embedder's `Spawner`)
/// satisfies `Send` end-to-end — without this bound the trait's
/// auto-trait inference can leave the driver future `!Send` and
/// `spawn_detached` rejects it.
pub trait CarrierTransport: Send + 'static {
    /// Address of the peer this transport is bound to. Used for
    /// diagnostics.
    fn peer_addr(&self) -> SocketAddr;

    /// Async read of the next plaintext datagram. Mirrors
    /// [`SecureConnection::read`].
    fn read(&mut self) -> impl Future<Output = Result<Bytes, DriverError>> + Send + '_;

    /// Non-blocking variant — `Ok(None)` if no data is queued. Used
    /// by the carrier's batch-drain loop after an async read fires,
    /// to pick up additional packets the OS already buffered.
    fn try_recv(&mut self) -> Result<Option<Bytes>, DriverError>;

    /// Push one carrier datagram out. Takes `Bytes` so callers that
    /// already have one (the carrier driver's `prepare_outgoing_datagram`
    /// returns `Bytes`) avoid a `copy_from_slice` round-trip — the
    /// inner `Arc` is incremented and the channel/socket gets the
    /// same backing buffer.
    fn write(
        &mut self,
        data: Bytes,
    ) -> impl Future<Output = Result<usize, DriverError>> + Send + '_;
}

/// Initiator-side transport: a dedicated DTLS connection on its own
/// UDP socket used by client connections.
pub struct DtlsTransport(pub SecureConnection<Established>);

impl CarrierTransport for DtlsTransport {
    fn peer_addr(&self) -> SocketAddr {
        self.0.peer_addr()
    }

    async fn read(&mut self) -> Result<Bytes, DriverError> {
        self.0.read().await
    }

    fn try_recv(&mut self) -> Result<Option<Bytes>, DriverError> {
        self.0.try_recv_decrypt()
    }

    async fn write(&mut self, data: Bytes) -> Result<usize, DriverError> {
        // `SecureConnection::write` takes `&[u8]` (it copies into the
        // OpenSSL `SSL_write` buffer anyway). The `Bytes` `Deref`s to
        // `&[u8]` without allocating.
        self.0.write(&data).await
    }
}

/// Responder-side transport: plaintext arrives via `inbound` (fed by
/// [`MultiPeerListener`]'s demux loop) and outbound carrier datagrams
/// go directly into the listener's per-peer SSL_write queue via
/// `listener.send_to(peer_addr, ...)`. One task per peer is
/// eliminated by skipping any intermediate forwarder hop.
#[cfg(feature = "server")]
pub struct ChannelTransport {
    pub peer_addr: SocketAddr,
    pub inbound: Receiver<Bytes>,
    pub listener: Arc<MultiPeerListener>,
}

#[cfg(feature = "server")]
impl CarrierTransport for ChannelTransport {
    fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    async fn read(&mut self) -> Result<Bytes, DriverError> {
        self.inbound
            .recv()
            .await
            .map_err(|_| DriverError::ConnectionClosed)
    }

    fn try_recv(&mut self) -> Result<Option<Bytes>, DriverError> {
        match self.inbound.try_recv() {
            Ok(bytes) => Ok(Some(bytes)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => Err(DriverError::ConnectionClosed),
        }
    }

    async fn write(&mut self, data: Bytes) -> Result<usize, DriverError> {
        let len = data.len();
        self.listener.send_to(self.peer_addr, data).await?;
        Ok(len)
    }
}
