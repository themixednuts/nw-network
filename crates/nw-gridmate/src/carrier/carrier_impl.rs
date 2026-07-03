// Carrier - GridMate Carrier Interface and Implementation
// Following GridMate/Carrier/Carrier.h and Carrier.cpp
//
// Architecture:
// - trait Carrier: Abstract interface (GridMate: class Carrier)
// - struct CarrierImpl: Concrete implementation (GridMate: class CarrierImpl : public Carrier)
// - Uses IoTaskPool for async operations with OpenSSL

use super::carrier_desc::{CarrierDesc, CarrierProtocolProfile};
use super::carrier_thread::{CarrierDriver, CarrierRole};
use super::connection_state::ConnectionState;
use super::io::{CarrierTransport, DtlsTransport};
use super::message::{DataReliability, MessageData};
use super::thread_message::{CarrierCommand, CarrierEvent};
use super::types::DisconnectReason;
use crate::driver::secure_connection::SecureConnection;
use crate::{GridMateError, Result};
use async_channel::{Receiver, Sender, bounded};
use async_io::Timer;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{debug, trace};

// ============================================================================
// Carrier State and Trait
// ============================================================================

/// Type-level state markers for CarrierImpl
/// State is encoded in the type system, preventing invalid operations at compile time
use crate::state::{Connected, Connecting};

/// Disconnected state - not connected (carrier-specific)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

/// Disconnecting state - graceful shutdown (carrier-specific)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnecting;

/// Inner state shared across all carrier states (enables safe state
/// transitions).
///
/// `from_carrier_rx` is `Option` because there is exactly one consumer
/// of the carrier event stream and it is moved out at construction
/// time. The current consumers are:
/// - the session-service per-session translator
///   ([`crate::session_service`]), which takes ownership via
///   [`CarrierImpl::take_event_receiver`] and forwards every event
///   into the actor's multiplexed select loop;
/// - the multi-peer server driver ([`crate::server`]), which holds
///   the receiver internally and reads via
///   [`CarrierImpl::<Connected>::recv_message`].
///
/// Once moved out, the receiver does not come back — callers that
/// want to drive carrier events through their own loop are
/// responsible for the lifecycle. The `Option` is the runtime
/// witness of that single-consumer contract.
struct CarrierInner {
    /// Channel to send commands to carrier thread
    to_carrier_tx: Sender<CarrierCommand>,

    /// Inbound carrier event stream. `None` once a consumer has
    /// taken ownership via [`CarrierImpl::take_event_receiver`] — see
    /// the module docstring on `CarrierInner`.
    from_carrier_rx: Option<Receiver<CarrierEvent>>,

    /// Shutdown flag for graceful termination
    shutdown_flag: Arc<AtomicBool>,

    /// Cleanup completion signal (one-shot channel)
    _cleanup_done: async_channel::Receiver<()>,
}

/// CarrierImpl - Concrete implementation
/// Communicates with background CarrierDriver via channels
/// Type-level state machine: State is encoded in the type parameter
/// Default to Disconnected for backward compatibility
pub struct CarrierImpl<State = Disconnected>
where
    State: Send + Sync,
{
    /// Option enables moving inner out during state transitions without triggering Drop
    inner: Option<CarrierInner>,
    _state: PhantomData<State>,
}

impl<State> CarrierImpl<State>
where
    State: Send + Sync,
{
    /// Take the inner state, leaving None (for state transitions)
    fn take_inner(&mut self) -> CarrierInner {
        self.inner.take().expect("CarrierInner already taken")
    }

    /// Get a reference to the inner state
    fn inner(&self) -> &CarrierInner {
        self.inner.as_ref().expect("CarrierInner not present")
    }

    /// Get a mutable reference to the inner state
    fn inner_mut(&mut self) -> &mut CarrierInner {
        self.inner.as_mut().expect("CarrierInner not present")
    }

    /// Take ownership of the carrier's event stream. The
    /// gridmate-internal per-session / per-peer driver tasks call
    /// this once to wrap the receiver in a task that translates
    /// `CarrierEvent` into the public `Event` surface. Returns
    /// `None` if the receiver has already been taken (which would
    /// indicate a bug — only one consumer is supported).
    pub(crate) fn take_event_receiver(&mut self) -> Option<Receiver<CarrierEvent>> {
        self.inner_mut().from_carrier_rx.take()
    }

    /// Set the shutdown flag and yield long enough for the carrier
    /// thread to observe it. Works in any state — graceful cleanup
    /// doesn't depend on connection liveness.
    pub async fn shutdown(self) {
        if let Some(inner) = self.inner.as_ref() {
            inner.shutdown_flag.store(true, Ordering::Relaxed);
        }
        Timer::after(Duration::from_millis(100)).await;
    }

    /// State-erased send. Forwards through the carrier thread's
    /// command channel; the underlying carrier may not yet be
    /// `Connected` and the message will queue until it is.
    ///
    /// Takes `impl Into<Bytes>` so callers that already hold a
    /// `Bytes` (the typical outbound path — typed-encode produces
    /// `Bytes::from(Vec<u8>)`) avoid the `copy_from_slice` round
    /// trip. `&[u8]` callers still work (`impl Into<Bytes>` covers
    /// `Vec<u8>` and `&'static [u8]` zero-allocation; non-static
    /// `&[u8]` falls back to `Bytes::copy_from_slice`).
    ///
    /// Prefer the inherent [`CarrierImpl<Connected>::send`] on a
    /// `Connected` value — it's typestate-gated and clearer at
    /// call sites.
    pub async fn send(
        &mut self,
        data: impl Into<bytes::Bytes>,
        reliability: DataReliability,
        priority: usize,
        channel: u8,
    ) -> Result<()> {
        let msg = MessageData {
            channel,
            reliability,
            data: data.into(),
            ..Default::default()
        };
        self.send_message(msg, priority).await
    }
}

/// Capacity of the consumer → carrier-driver command channel.
const CMD_CAPACITY: usize = 256;
/// Capacity of the carrier-driver → consumer event channel.
const EVENT_CAPACITY: usize = 512;

impl CarrierImpl<Disconnected> {
    /// Initiator-side carrier (GridMate: `CarrierImpl` constructor on
    /// the client). Establishes the DTLS connection from
    /// [`CarrierDesc::server_address`], spawns the driver as an
    /// [`CarrierRole::Initiator`] (actively retransmits
    /// `SM_CONNECT_REQUEST` until `SM_CONNECT_ACK` arrives), and
    /// returns a [`CarrierImpl<Connecting>`].
    pub async fn new(desc: CarrierDesc) -> Result<CarrierImpl<Connecting>> {
        let dtls = SecureConnection::connect(
            &desc.server_address,
            desc.ca_cert.as_deref(),
            desc.ssl_keylog_path.as_deref(),
        )
        .await
        .map_err(GridMateError::Driver)?;
        debug!("Carrier driver established");

        let protocol = desc.protocol_profile();
        let conn = ConnectionState::with_protocol_profile(DtlsTransport(dtls), protocol);
        let mut carrier = Self::start_with(conn, CarrierRole::Initiator);
        carrier.connect_request(protocol).await?;
        Ok(carrier)
    }

    /// Responder-side carrier — the symmetric mirror of [`Self::new`].
    /// Mirrors Lumberyard's `OnNewConnection` → `MTM_NEW_CONNECTION`
    /// path: a peer's first carrier datagram has already arrived
    /// (e.g. from [`crate::driver::MultiPeerListener`]) and the
    /// application is opting in to handle it. We do not send
    /// `SM_CONNECT_REQUEST`; the client is responsible for that. We
    /// do reply with `SM_CONNECT_ACK` once their request reaches us,
    /// transitioning to `Connected`.
    ///
    /// `io` is any [`CarrierTransport`] — typically a
    /// [`super::io::ChannelTransport`] backed by per-peer queues from a
    /// multi-peer demux. The transport stays as a concrete generic
    /// type so `read`/`write`/`try_recv` monomorphise — no `dyn`
    /// dispatch and no match arms on the carrier hot path.
    pub fn accept<T: CarrierTransport>(io: T) -> Result<CarrierImpl<Connecting>> {
        Self::accept_with_protocol_profile(io, CarrierProtocolProfile::default())
    }

    /// Responder-side carrier with an explicit carrier wire profile.
    pub fn accept_with_protocol_profile<T: CarrierTransport>(
        io: T,
        protocol: CarrierProtocolProfile,
    ) -> Result<CarrierImpl<Connecting>> {
        let conn = ConnectionState::with_protocol_profile(io, protocol);
        Ok(Self::start_with(conn, CarrierRole::Responder))
    }

    /// Shared setup for both entry points: build the cmd/event
    /// channels, spawn the driver task on the embedder's executor,
    /// and wrap the resulting handle in `CarrierImpl<Connecting>`.
    /// `T` is erased from the returned handle — only the driver task
    /// carries the transport type.
    fn start_with<T: CarrierTransport>(
        conn: ConnectionState<T>,
        role: CarrierRole,
    ) -> CarrierImpl<Connecting> {
        let (to_carrier_tx, from_main_rx) = bounded(CMD_CAPACITY);
        let (to_main_tx, to_main_rx) = bounded::<CarrierEvent>(EVENT_CAPACITY);
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let (cleanup_tx, cleanup_rx) = async_channel::bounded(1);

        let driver = CarrierDriver::new(
            conn,
            from_main_rx,
            to_main_tx,
            shutdown_flag.clone(),
            cleanup_tx,
            role,
        );
        crate::spawn::spawn_detached(driver.run());

        CarrierImpl::<Connecting> {
            inner: Some(CarrierInner {
                to_carrier_tx,
                from_carrier_rx: Some(to_main_rx),
                shutdown_flag,
                _cleanup_done: cleanup_rx,
            }),
            _state: PhantomData,
        }
    }
}

impl<State> CarrierImpl<State>
where
    State: Send + Sync,
{
    /// Start connection handshake
    ///
    /// The carrier thread's retry loop handles all SM_CONNECT_REQUEST messages
    /// with exponential backoff (10, 20, 40, 80... up to 1000ms).
    ///
    /// This method just logs that handshake is starting - the actual messages
    /// are sent by the carrier thread's handle_handshake_retry().
    async fn connect_request(&mut self, protocol: CarrierProtocolProfile) -> Result<()> {
        use tracing::debug;

        debug!(
            "[CARRIER] Starting handshake (v{}), retry loop handles exponential backoff",
            protocol.version
        );

        // Note: The carrier thread's handshake_retry_time is initialized to fire immediately,
        // so the first SM_CONNECT_REQUEST will be sent right away by handle_handshake_retry().

        Ok(())
    }

    /// Send message to carrier thread (DRY helper)
    async fn send_message(&mut self, msg: MessageData, priority: usize) -> Result<()> {
        self.inner()
            .to_carrier_tx
            .send(CarrierCommand::SendMessage {
                message: msg,
                priority,
            })
            .await
            .map_err(|e| GridMateError::Channel(format!("Send failed: {}", e)))
    }
}

/// State-specific implementations for Connecting state
impl CarrierImpl<Connecting> {
    /// Wait for connection to establish, then transition to Connected state
    /// Consumes self and returns CarrierImpl<Connected>
    pub async fn ready(mut self) -> Result<CarrierImpl<Connected>> {
        // Take the receiver to read from in this loop, then put it
        // back before returning — subsequent `receive(channel)` calls
        // on `CarrierImpl<Connected>` rely on it being present.
        let Some(rx) = self.inner_mut().from_carrier_rx.take() else {
            return Err(GridMateError::ConnectionClosed);
        };
        loop {
            match rx.recv().await {
                Ok(CarrierEvent::Connected { version }) => {
                    trace!("Connected to server (v{})", version);
                    let mut inner = self.take_inner();
                    inner.from_carrier_rx = Some(rx);
                    return Ok(CarrierImpl {
                        inner: Some(inner),
                        _state: PhantomData,
                    });
                }
                Ok(CarrierEvent::Disconnected { reason }) => {
                    return Err(GridMateError::Carrier(format!(
                        "Disconnected: {:?}",
                        reason
                    )));
                }
                Ok(CarrierEvent::Error { description }) => {
                    return Err(GridMateError::Carrier(description));
                }
                Ok(CarrierEvent::MessageReceived { .. }) => {
                    // Messages received before ready are ignored in connecting state.
                }
                Err(_) => {
                    return Err(GridMateError::ConnectionClosed);
                }
            }
        }
    }
}

/// State-specific implementations for Connected state
impl CarrierImpl<Connected> {
    /// Disconnect (GridMate: Carrier::Disconnect)
    /// Consumes self and returns CarrierImpl<Disconnecting>
    pub async fn disconnect(
        mut self,
        reason: DisconnectReason,
    ) -> Result<CarrierImpl<Disconnecting>> {
        debug!("Disconnecting: {:?}", reason);

        let _ = self
            .inner()
            .to_carrier_tx
            .send(CarrierCommand::Disconnect(reason))
            .await;

        // Wait for disconnect processing
        Timer::after(Duration::from_millis(100)).await;

        // Safe state transition: take inner and move to new type
        let inner = self.take_inner();
        Ok(CarrierImpl {
            inner: Some(inner),
            _state: PhantomData,
        })
    }

    /// Async stream of carrier messages for this session. Used by
    /// the gridmate-internal per-peer driver in `crate::server` to
    /// translate `CarrierEvent` into the public `Event` surface.
    /// Not exposed to application code.
    #[cfg(feature = "server")]
    pub(crate) async fn recv_message(&mut self) -> Option<CarrierEvent> {
        let rx = self.inner_mut().from_carrier_rx.as_mut()?;
        rx.recv().await.ok()
    }
}

/// Implement Drop for graceful cleanup
impl<State> Drop for CarrierImpl<State>
where
    State: Send + Sync,
{
    fn drop(&mut self) {
        // Only clean up if inner wasn't already taken (state transition)
        if let Some(inner) = self.inner.as_ref() {
            // Best-effort: Try to send disconnect message
            let _ = inner
                .to_carrier_tx
                .try_send(CarrierCommand::Disconnect(DisconnectReason::ShuttingDown));

            // Signal shutdown
            inner.shutdown_flag.store(true, Ordering::Relaxed);

            // Best-effort: cleanup completion signal is handled in background
        }
    }
}
