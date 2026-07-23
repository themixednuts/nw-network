//! SessionService - Top-level session manager
//!
//! Implements an actor pattern where the actor owns session state and communicates
//! via message passing. Based on GridMate/Session/Session.h SessionService class.

use super::carrier::CarrierConnecting;
use super::session::{Connecting, GridSession};
use crate::Result;
use crate::message::{ClientToServer, EgressPath, Sendable};
use crate::serialize::{WriteBuffer, buffer::CARRIER_ENDIAN};
use async_channel::{Receiver, Sender, bounded};
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::Arc;
use tracing::trace;

const REPLICATED_STATE_BUNDLE_TYPE_INDEX: u32 = 8;
const REPLICATED_STATE_BUNDLE_TYPE_NAME: &str = "Amazon::Hub::ReplicatedStateBundle";

/// Session index type. Newtype for type safety — prevents mixing
/// session indices with other usize values.
///
/// `Deref<Target = usize>` so callsites that need raw indexing
/// (`Vec` slot, log formatting) write `*session_id` without an
/// explicit field access. Equality is `SessionId == SessionId` — the
/// `Deref` does not introduce cross-type comparison, so callers must
/// not write `tag.0 == *session` expecting `usize == SessionId`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::Deref,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
)]
pub struct SessionId(pub usize);

impl SessionId {
    pub fn new(index: usize) -> Self {
        Self(index)
    }
}

/// Unified network event surface. Both the client-side
/// [`SessionServiceHandle`] and the server-side
/// [`crate::ServerListenerHandle`] emit this. The Bevy plugin wraps
/// it in a `NetEvent` newtype for Bevy's messaging system.
///
/// `session: SessionId` identifies the peer/session uniformly across
/// both sides — the server allocates a SessionId per accepted peer;
/// the client gets one per outbound connection. `channel` is the
/// carrier channel id so applications can demux ordinary Hub messages from
/// channel-1 replicated-state Hub messages and carrier system traffic.
#[derive(Debug, Clone)]
pub enum Event {
    /// Data received on a session (raw decrypted carrier bytes, no
    /// envelope parsing). The application demuxes on `channel`.
    Received {
        session: SessionId,
        channel: u8,
        data: Bytes,
    },

    /// Typed IMessage envelope received, parsed past the wire
    /// framing. `type_index` identifies the message type.
    TypedReceived {
        session: SessionId,
        channel: u8,
        type_index: u32,
        data: Bytes,
    },

    /// Data accepted for send on a session.
    Sent {
        session: SessionId,
        data: Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
        type_index: Option<u32>,
    },

    /// Session connected and carrier ready for typed traffic.
    Ready { session: SessionId },

    /// Session disconnected.
    Disconnected { session: SessionId, reason: String },

    /// Transport-level error not bound to a specific session.
    Error { description: String },
}

impl Event {
    /// Decode a typed receive event body as a concrete generated/manual message.
    ///
    /// Returns `Ok(None)` when the event is not typed traffic or when the type
    /// index does not match `T`. Generated direct-message structs work here
    /// through the blanket [`crate::message::Message`] implementation.
    pub fn typed_message<T>(&self) -> std::result::Result<Option<T>, crate::MarshalerError>
    where
        T: crate::message::Message + crate::Unmarshal,
    {
        let Event::TypedReceived {
            type_index, data, ..
        } = self
        else {
            return Ok(None);
        };
        if *type_index != <T as crate::message::Message>::TYPE_INDEX {
            return Ok(None);
        }

        let mut rb = crate::ReadBuffer::new(crate::serialize::CARRIER_ENDIAN, data.as_ref());
        <T as crate::Unmarshal>::unmarshal(&mut rb).map(Some)
    }

    /// Borrow-decode a replicated state bundle event.
    ///
    /// Returns `Ok(None)` when the event is not state-bundle lane traffic.
    pub fn state_bundle(
        &self,
    ) -> std::result::Result<Option<nw_network::ReplicatedStateBundleView<'_>>, crate::MarshalerError>
    {
        let Event::TypedReceived {
            type_index, data, ..
        } = self
        else {
            return Ok(None);
        };
        if !is_replicated_state_bundle_type_index(*type_index) {
            return Ok(None);
        };

        let mut rb = crate::ReadBuffer::new(crate::serialize::CARRIER_ENDIAN, data.as_ref());
        nw_network::ReplicatedStateBundleView::read_from(&mut rb).map(Some)
    }
}

/// Legacy alias for backward compatibility
#[deprecated(note = "use Event instead")]
pub type SessionEvent = Event;

/// Messages sent to the SessionService actor
pub enum SessionServiceMessage {
    /// Create a new session
    CreateSession {
        desc: Box<super::carrier::CarrierDesc>,
        respond_to: Sender<Result<SessionId>>,
    },
    /// Send a message on a session
    SendMessage {
        session_id: SessionId,
        data: bytes::Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
        respond_to: Sender<Result<()>>,
    },
    /// Get number of sessions
    GetNumSessions { respond_to: Sender<usize> },
    /// Disconnect all sessions
    DisconnectAll { respond_to: Sender<Result<()>> },
    /// Shutdown the actor
    Shutdown { respond_to: Sender<()> },
}

/// Per-session inbox item — pairs a carrier-level event with the
/// session it belongs to. Used internally by the per-session task to
/// tag every `CarrierEvent` with its `SessionId` before the
/// actor's multiplexed select loop picks it up.
type SessionCarrierEvent = (SessionId, super::carrier::thread_message::CarrierEvent);

/// Internal actor that owns session state and processes messages.
///
/// UUID to `type_index` resolution for inbound message envelopes goes through
/// the generated network schema. The actor does not carry per-instance
/// reflection state.
struct SessionServiceActor {
    receiver: Receiver<SessionServiceMessage>,
    sessions: Vec<Option<GridSession<Connecting, CarrierConnecting>>>,
    active_sessions: usize,
    event_tx: Sender<Event>,
    session_event_tx: Sender<SessionCarrierEvent>,
    session_event_rx: Receiver<SessionCarrierEvent>,
    /// Per-session translator tasks are detached; they self-terminate
    /// when the carrier event receiver closes. The set is kept only
    /// to mirror live sessions for diagnostics — `()` is a zero-cost
    /// presence marker.
    session_tasks: HashMap<SessionId, ()>,
    /// Wire-capture sidecar. No-op in release builds (the type is
    /// `()` then) and gated on `NW_NETWORK_CAPTURE` env var even in
    /// debug builds. Lives outside the actor's core
    /// responsibilities — capture is a tap, not orchestration.
    #[cfg(debug_assertions)]
    capture: crate::capture::CaptureCounters,
    /// Track which sessions have sent ready events using HashSet for efficiency
    /// More efficient than Vec<bool> for sparse indices and many sessions
    ready_events_sent: HashSet<SessionId>,
}

impl SessionServiceActor {
    async fn run(mut self) {
        use futures_util::future::{Either, select};

        loop {
            let recv_cmd = self.receiver.recv();
            let recv_evt = self.session_event_rx.recv();
            futures_util::pin_mut!(recv_cmd);
            futures_util::pin_mut!(recv_evt);

            match select(recv_cmd, recv_evt).await {
                Either::Left((msg_result, _pending_event)) => {
                    let msg = match msg_result {
                        Ok(msg) => msg,
                        Err(_) => break,
                    };
                    if !self.handle_message(msg).await {
                        break;
                    }
                }
                Either::Right((event_result, _pending_msg)) => {
                    let event = match event_result {
                        Ok(event) => event,
                        Err(_) => break,
                    };
                    self.handle_carrier_event(event).await;
                }
            }
        }
    }

    async fn handle_message(&mut self, msg: SessionServiceMessage) -> bool {
        match msg {
            SessionServiceMessage::CreateSession { desc, respond_to } => {
                let result = self.handle_create_session(*desc).await;
                let _ = respond_to.send(result).await;
                true
            }
            SessionServiceMessage::SendMessage {
                session_id,
                data,
                channel,
                reliability,
                priority,
                respond_to,
            } => {
                let result = self
                    .handle_send_message(session_id, data, channel, reliability, priority)
                    .await;
                let _ = respond_to.send(result).await;
                true
            }
            SessionServiceMessage::GetNumSessions { respond_to } => {
                let _ = respond_to.send(self.active_sessions).await;
                true
            }
            SessionServiceMessage::DisconnectAll { respond_to } => {
                let result = self.handle_disconnect_all().await;
                let _ = respond_to.send(result).await;
                true
            }
            SessionServiceMessage::Shutdown { respond_to } => {
                let _ = self.handle_disconnect_all().await;
                let _ = respond_to.send(()).await;
                false
            }
        }
    }

    async fn handle_carrier_event(&mut self, event: SessionCarrierEvent) {
        use super::carrier::thread_message::CarrierEvent;
        use super::session::CarrierChannel;

        let (session_id, msg) = event;
        match msg {
            CarrierEvent::Connected { version } => {
                trace!("Session {:?} connected (v{})", session_id, version);
                self.send_ready_if_needed(session_id).await;
            }
            CarrierEvent::Disconnected { reason } => {
                trace!("Session {:?} disconnected: {:?}", session_id, reason);
                self.close_session(session_id, Some(format!("{reason:?}")))
                    .await;
            }
            CarrierEvent::Error { description } => {
                trace!("Session {:?} carrier error: {}", session_id, description);
                let _ = self.event_tx.send(Event::Error { description }).await;
            }
            CarrierEvent::MessageReceived { channel, data } => {
                if !self.is_live_session(session_id) {
                    trace!("Dropped message for closed session {:?}", session_id);
                    return;
                }

                self.send_ready_if_needed(session_id).await;

                if self
                    .event_tx
                    .send(Event::Received {
                        session: session_id,
                        channel,
                        data: data.clone(),
                    })
                    .await
                    .is_err()
                {
                    return;
                }

                let Some(channel_kind) = CarrierChannel::from_u8(channel) else {
                    trace!(channel, "received carrier data on unknown channel");
                    return;
                };

                if channel_kind.carries_hub_messages() {
                    for event in self.dispatch_messages(session_id, channel, &data) {
                        let _ = self.event_tx.send(event).await;
                    }
                }
            }
        }
    }

    async fn send_ready_if_needed(&mut self, session_id: SessionId) {
        if self.ready_events_sent.contains(&session_id) {
            return;
        }

        trace!("Session {:?} ready", session_id);
        if let Err(e) = self
            .event_tx
            .send(Event::Ready {
                session: session_id,
            })
            .await
        {
            trace!("Failed to send SessionReady event: {}", e);
        }
        self.ready_events_sent.insert(session_id);
    }

    fn is_live_session(&self, session_id: SessionId) -> bool {
        matches!(self.sessions.get(*session_id), Some(Some(_)))
    }

    fn live_session_mut(
        &mut self,
        session_id: SessionId,
    ) -> Result<&mut GridSession<Connecting, CarrierConnecting>> {
        self.sessions
            .get_mut(*session_id)
            .and_then(Option::as_mut)
            .ok_or_else(|| {
                crate::GridMateError::InvalidState(format!("Session {:?} not found", session_id))
            })
    }

    async fn close_session(&mut self, session_id: SessionId, reason: Option<String>) {
        let closed = match self.sessions.get_mut(*session_id) {
            Some(slot) => slot.take().is_some(),
            None => false,
        };

        if !closed {
            return;
        }

        self.active_sessions = self.active_sessions.saturating_sub(1);
        self.session_tasks.remove(&session_id);
        self.ready_events_sent.remove(&session_id);

        if let Some(reason) = reason {
            let _ = self
                .event_tx
                .send(Event::Disconnected {
                    session: session_id,
                    reason,
                })
                .await;
        }
    }

    async fn handle_create_session(
        &mut self,
        desc: super::carrier::CarrierDesc,
    ) -> Result<SessionId> {
        let mut session = GridSession::join(desc).await?;
        let index = self.sessions.len();
        let session_id = SessionId::new(index);

        let carrier = session
            .carrier_mut()
            .ok_or_else(|| crate::GridMateError::InvalidState("Carrier missing".into()))?;
        let Some(rx) = carrier.take_event_receiver() else {
            return Err(crate::GridMateError::InvalidState(
                "Carrier receiver unavailable".into(),
            ));
        };

        let event_tx = self.session_event_tx.clone();
        crate::spawn::spawn_detached(async move {
            use super::carrier::thread_message::CarrierEvent;
            // Forward every carrier event tagged with this task's
            // session id. The actor's multiplexed select loop picks
            // them up alongside command-channel traffic.
            while let Ok(msg) = rx.recv().await {
                if event_tx.send((session_id, msg)).await.is_err() {
                    break;
                }
            }
            // Channel closed unexpectedly — synthesise a disconnect
            // so the actor cleans up bookkeeping.
            let _ = event_tx
                .send((
                    session_id,
                    CarrierEvent::Disconnected {
                        reason: super::carrier::DisconnectReason::ShuttingDown,
                    },
                ))
                .await;
        });

        self.session_tasks.insert(session_id, ());
        self.sessions.push(Some(session));
        self.active_sessions += 1;
        trace!("Created GridMate session with id {:?}", session_id);
        Ok(session_id)
    }

    async fn handle_send_message(
        &mut self,
        session_id: SessionId,
        data: bytes::Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> Result<()> {
        let type_index = Self::parse_send_type_index(data.as_ref());
        let session = self.live_session_mut(session_id)?;
        session
            .send(channel, data.clone(), reliability, priority)
            .await?;

        #[cfg(debug_assertions)]
        self.capture.capture_send(
            session_id,
            &data,
            channel,
            reliability,
            priority,
            type_index,
        );

        let _ = self
            .event_tx
            .send(Event::Sent {
                session: session_id,
                data: data.clone(),
                channel,
                reliability,
                priority,
                type_index,
            })
            .await;

        Ok(())
    }

    fn parse_send_type_index(data: &[u8]) -> Option<u32> {
        use crate::serialize::buffer::{CARRIER_ENDIAN, ReadBuffer};
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, data);
        let (_metadata, envelope) = crate::message::read_correlated_message(&mut rb).ok()?;
        match envelope.type_id {
            crate::message::MessageTypeId::TypeIndex(type_index) => Some(type_index),
            crate::message::MessageTypeId::Uuid(uuid) => crate::message::resolve_type_index(uuid),
        }
    }

    async fn handle_disconnect_all(&mut self) -> Result<()> {
        let live_sessions: Vec<SessionId> = self
            .sessions
            .iter()
            .enumerate()
            .filter_map(|(index, session)| session.as_ref().map(|_| SessionId::new(index)))
            .collect();

        for session_id in live_sessions {
            self.close_session(session_id, Some("disconnect_all".to_owned()))
                .await;
        }
        Ok(())
    }

    /// Dispatch messages from a buffer containing multiple concatenated messages.
    ///
    /// Wire format:
    /// ```text
    /// [vlq_body_size_1][message_1][vlq_body_size_2][message_2]...
    /// ```
    ///
    /// Each message envelope:
    /// ```text
    /// [outer_flags: 1 byte]     - bit 0: has field1, bit 1: has field2
    /// [field1: 8 bytes]         - optional, if outer_flags & 1
    /// [field2: 8 bytes]         - optional, if outer_flags & 2
    /// [envelope_flags: 1 byte]  - 0=empty, 1=has message, >=2 error
    /// [vlq_type_index: var]     - 0=UUID follows, non-zero=direct type index
    /// [uuid: 16 bytes]          - only if vlq_type_index == 0
    /// [body: remaining]         - message payload
    /// ```
    fn dispatch_messages(
        &mut self,
        session_id: SessionId,
        channel: u8,
        data: &Bytes,
    ) -> Vec<Event> {
        use super::serialize::buffer::{CARRIER_ENDIAN, ReadBuffer};

        let mut events = Vec::new();

        if data.is_empty() {
            return events;
        }

        trace!(
            "CARRIER_RECV: total_size={} hex={}",
            data.len(),
            hex::encode(data.as_ref())
        );

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, data.as_ref());

        // Loop while the stream still contains sized message envelopes.
        while !rb.is_empty() {
            match crate::message::read_sized_message(&mut rb) {
                Ok((metadata, view)) => {
                    trace!("Message body_size: {}", metadata.envelope_size);
                    if let Some(event) = self.event_from_message_envelope(session_id, channel, view)
                    {
                        events.push(event);
                    }
                }
                Err(crate::serialize::error::MarshalerError::EmptyEnvelope) => {
                    trace!("Empty envelope (flags=0)");
                }
                Err(err) => {
                    trace!("Invalid message stream item: {}", err);
                    break;
                }
            }
        }

        trace!("Dispatched {} messages from buffer", events.len());
        events
    }

    /// Convert a parsed message envelope into a typed event.
    fn event_from_message_envelope(
        &mut self,
        session_id: SessionId,
        channel: u8,
        envelope: crate::message::MessageEnvelopeView<'_>,
    ) -> Option<Event> {
        trace!("Outer flags: 0x{:02x}", envelope.outer_flags);

        let type_index = match envelope.type_id {
            crate::message::MessageTypeId::TypeIndex(type_index) => type_index,
            crate::message::MessageTypeId::Uuid(uuid) => {
                trace!("Full UUID bytes: {:02x?}", uuid.as_bytes());
                match crate::message::resolve_type_index(uuid) {
                    Some(ti) => ti,
                    None => {
                        trace!("Unknown UUID: {:02x?}", uuid.as_bytes());
                        return None;
                    }
                }
            }
        };

        let message_data = if envelope.body.is_empty() {
            Bytes::new()
        } else {
            Bytes::copy_from_slice(envelope.body)
        };

        trace!(
            "Parsed message: type_index={}, payload_size={}",
            type_index,
            message_data.len()
        );

        #[cfg(debug_assertions)]
        self.capture
            .capture_recv(session_id, type_index, envelope.raw, &message_data);

        Some(Event::TypedReceived {
            session: session_id,
            channel,
            type_index,
            data: message_data,
        })
    }
}

fn is_replicated_state_bundle_type_index(type_index: u32) -> bool {
    type_index == REPLICATED_STATE_BUNDLE_TYPE_INDEX
        || nw_network::name_for_type_index(type_index) == Some(REPLICATED_STATE_BUNDLE_TYPE_NAME)
}

/// Handle for communicating with SessionService actor
///
/// The handle is Clone and communicates with the actor via message channels.
#[derive(Clone)]
pub struct SessionServiceHandle {
    sender: Sender<SessionServiceMessage>,
    event_rx: Receiver<Event>,
    /// Zero-sized ref counter used solely to detect the last handle
    /// drop — at that point the Drop impl signals graceful shutdown.
    /// The actor task itself runs detached on the embedder-registered
    /// spawner.
    handle_ref: Arc<()>,
    shutdown_complete: Receiver<()>,
}

impl SessionServiceHandle {
    /// Create new SessionService actor and handle.
    ///
    /// UUID to `type_index` resolution flows through the generated network
    /// schema. No per-handle registry state is configured.
    pub fn new() -> Self {
        // Sized for client-side: typically O(1) sessions, but events
        // can burst (e.g., dispatching many TypedReceived from a
        // single inbound buffer). Bounded so a slow drain
        // backpressures rather than growing memory.
        let (sender, receiver) = bounded(256);
        let (event_tx, event_rx) = bounded(4096);
        let (shutdown_tx, shutdown_rx) = bounded(1);
        let (session_event_tx, session_event_rx) = bounded(2048);

        let actor = SessionServiceActor {
            receiver,
            sessions: Vec::new(),
            active_sessions: 0,
            event_tx,
            session_event_tx,
            session_event_rx,
            session_tasks: HashMap::new(),
            #[cfg(debug_assertions)]
            capture: crate::capture::CaptureCounters::default(),
            ready_events_sent: HashSet::new(),
        };

        crate::spawn::spawn_detached(async move {
            actor.run().await;
            let _ = shutdown_tx.send(()).await;
        });

        Self {
            sender,
            event_rx,
            handle_ref: Arc::new(()),
            shutdown_complete: shutdown_rx,
        }
    }

    /// Create a new session
    pub async fn create_session(&self, desc: super::carrier::CarrierDesc) -> Result<SessionId> {
        let (tx, rx) = bounded(1);
        self.sender
            .send(SessionServiceMessage::CreateSession {
                desc: Box::new(desc),
                respond_to: tx,
            })
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?;
        rx.recv()
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?
    }

    /// Update all sessions (legacy no-op for async actor)
    pub async fn update(&self) -> Result<()> {
        Ok(())
    }

    /// Typed client send: encode `msg` as a ClientToServer Hub envelope
    /// and enqueue it on `session_id`.
    ///
    /// Returns a builder so callers can attach path-specific
    /// framing context (e.g. `.for_actor(actor)` on ClientToServer) or
    /// override defaults before `.await`-ing it (or calling
    /// `.try_send_now()` for a sync, fire-and-forget enqueue).
    pub fn send<M>(&self, session_id: SessionId, msg: M) -> Outgoing<'_, M, Self, ClientToServer>
    where
        M: Sendable<ClientToServer>,
    {
        Outgoing::new(self, session_id, msg)
    }

    /// Send raw data on a session
    pub async fn send_message(
        &self,
        session_id: SessionId,
        data: bytes::Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> Result<()> {
        let (tx, rx) = bounded(1);
        self.sender
            .send(SessionServiceMessage::SendMessage {
                session_id,
                data,
                channel,
                reliability,
                priority,
                respond_to: tx,
            })
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?;
        rx.recv()
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?
    }

    /// Enqueue raw data on a session without waiting for the actor to flush it.
    ///
    /// The session-service actor processes this command channel FIFO, so callers
    /// that invoke this from ordered Bevy systems preserve wire enqueue order
    /// without spawning per-message tasks that can race each other.
    pub fn try_send_message_detached(
        &self,
        session_id: SessionId,
        data: bytes::Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> Result<()> {
        let (tx, _rx) = bounded(1);
        self.sender
            .try_send(SessionServiceMessage::SendMessage {
                session_id,
                data,
                channel,
                reliability,
                priority,
                respond_to: tx,
            })
            .map_err(|e| {
                if e.is_full() {
                    crate::GridMateError::Channel("session service command queue full".into())
                } else {
                    crate::GridMateError::ConnectionClosed
                }
            })
    }

    /// Get number of sessions
    pub async fn num_sessions(&self) -> usize {
        let (tx, rx) = bounded(1);
        if self
            .sender
            .send(SessionServiceMessage::GetNumSessions { respond_to: tx })
            .await
            .is_err()
        {
            return 0;
        }
        rx.recv().await.unwrap_or(0)
    }

    /// Disconnect all sessions
    pub async fn disconnect_all(&self) -> Result<()> {
        let (tx, rx) = bounded(1);
        self.sender
            .send(SessionServiceMessage::DisconnectAll { respond_to: tx })
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?;
        rx.recv()
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?
    }

    /// Shutdown the actor (graceful)
    pub async fn shutdown(&self) -> Result<()> {
        let (tx, rx) = bounded(1);
        self.sender
            .send(SessionServiceMessage::Shutdown { respond_to: tx })
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?;
        rx.recv()
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?;
        self.shutdown_complete
            .recv()
            .await
            .map_err(|_| crate::GridMateError::ConnectionClosed)?;
        Ok(())
    }

    /// Receive next event (awaits data)
    pub async fn recv_event(&self) -> Option<Event> {
        self.event_rx.recv().await.ok()
    }
}

impl Default for SessionServiceHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SessionServiceHandle {
    fn drop(&mut self) {
        let is_last = Arc::strong_count(&self.handle_ref) == 1;

        if is_last {
            trace!("SessionService: initiating graceful shutdown");

            let (tx, rx) = bounded(1);
            let shutdown_sent = self
                .sender
                .try_send(SessionServiceMessage::Shutdown { respond_to: tx })
                .is_ok();

            if shutdown_sent {
                drop(rx);
            } else {
                self.sender.close();
            }
        }
        // Do NOT close sender when dropping a cloned handle - other handles still need it!
    }
}

/// Behavior an outbound destination must provide to be a typed-send
/// target. Both [`SessionServiceHandle`] (client) and
/// [`crate::ServerListenerHandle`] (server) implement this; the
/// [`Outgoing`] builder is generic over the trait so the two sides
/// share one builder type instead of mirrored near-clones.
///
/// `enqueue` uses return-position `impl Future` in a trait (RPITIT),
/// so the returned future is monomorphised against the concrete sink
/// and no boxed future crosses the trait boundary.
pub trait OutboundSink: Send + Sync {
    /// Async enqueue. Backpressures on full bounded channels.
    fn enqueue(
        &self,
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> impl Future<Output = Result<()>> + Send + '_;

    /// Sync fire-and-forget enqueue. Returns `Err` if the queue is
    /// full or closed.
    fn try_enqueue(
        &self,
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> Result<()>;
}

impl OutboundSink for SessionServiceHandle {
    async fn enqueue(
        &self,
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> Result<()> {
        self.send_message(session, bytes, channel, reliability, priority)
            .await
    }

    fn try_enqueue(
        &self,
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: super::carrier::DataReliability,
        priority: usize,
    ) -> Result<()> {
        self.try_send_message_detached(session, bytes, channel, reliability, priority)
    }
}

/// Typed-send builder produced by `handle.send(session, msg)` on
/// either [`SessionServiceHandle`] or [`crate::ServerListenerHandle`].
///
/// Awaiting it (or calling [`Outgoing::try_send_now`]) encodes the
/// message through the [`EgressPath`] `P` and enqueues it on the
/// target session. The path is a generic parameter (not an
/// associated type on the message) so dual-direction messages like
/// `PingMsg` can pick a direction at the call site.
///
/// Path-specific affordances are gated by `where` clauses:
/// [`Outgoing::for_actor`] only exists when `P = ClientToServer`
/// (the CRC plus correlation outer frame). Path-agnostic knobs
/// ([`Outgoing::on_channel`], [`Outgoing::with_reliability`],
/// [`Outgoing::with_priority`]) work uniformly.
///
/// Prefer [`Outgoing::enqueue`] when writing new async code: it returns
/// the compiler-native future from an `async fn`. Awaiting `Outgoing`
/// directly is kept as a compatibility convenience and boxes only at
/// the `IntoFuture` associated type boundary, where stable Rust cannot
/// name an opaque `impl Future` type.
pub struct Outgoing<'a, M, S: OutboundSink = SessionServiceHandle, P: EgressPath = ClientToServer> {
    sink: &'a S,
    session_id: SessionId,
    msg: M,
    context: P::Context,
    channel: u8,
    reliability: super::carrier::DataReliability,
    priority: usize,
}

impl<'a, M, S, P> Outgoing<'a, M, S, P>
where
    M: Sendable<P>,
    S: OutboundSink,
    P: EgressPath,
{
    /// Construct an `Outgoing` from a sink, session, and message.
    /// Defaults: channel 0, Reliable, priority 0, default context.
    pub fn new(sink: &'a S, session_id: SessionId, msg: M) -> Self {
        Self {
            sink,
            session_id,
            msg,
            context: <P::Context as Default>::default(),
            channel: 0,
            reliability: super::carrier::DataReliability::Reliable,
            priority: 0,
        }
    }

    /// Override the carrier channel (default 0).
    #[inline]
    pub fn on_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }

    /// Override the reliability flag (default Reliable).
    #[inline]
    pub fn with_reliability(mut self, reliability: super::carrier::DataReliability) -> Self {
        self.reliability = reliability;
        self
    }

    /// Override the priority slot (default 0).
    #[inline]
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }

    /// Encode via `P::marshal_framed`. Shared between the async
    /// (`.await`) and sync (`try_send_now`) paths so the framing
    /// logic lives in one place — and is owned entirely by the path
    /// trait, not by this builder.
    fn encode(self) -> Result<(SessionId, Bytes, u8, super::carrier::DataReliability, usize)> {
        P::validate_message::<M>()?;
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        P::marshal_framed::<M>(self.msg, self.context, &mut wb);
        let data = Bytes::from(wb.into_vec());
        Ok((
            self.session_id,
            data,
            self.channel,
            self.reliability,
            self.priority,
        ))
    }

    /// Sync, fire-and-forget enqueue. Use from Bevy systems that
    /// cannot await but need FIFO ordering.
    pub fn try_send_now(self) -> Result<()> {
        let sink = self.sink;
        let (session_id, data, channel, reliability, priority) = self.encode()?;
        sink.try_enqueue(session_id, data, channel, reliability, priority)
    }

    /// Async enqueue using the native future returned by this `async fn`.
    ///
    /// This is allocation-free at the API boundary. Awaiting `Outgoing`
    /// directly remains supported, but that path must box to satisfy
    /// [`IntoFuture`]'s stable associated-type shape.
    pub async fn enqueue(self) -> Result<()> {
        let sink = self.sink;
        let (session_id, data, channel, reliability, priority) = self.encode()?;
        sink.enqueue(session_id, data, channel, reliability, priority)
            .await
    }
}

/// `for_actor` only exists on paths whose framing context is `Uuid`
/// - today that's [`ClientToServer`] (the client-to-server `CorrelatedMetadata`
///   frame). Calling it on sized-only paths is a compile error, not a
///   silent no-op, because their `Context` type is `()`.
impl<'a, M, S> Outgoing<'a, M, S, ClientToServer>
where
    M: Sendable<ClientToServer>,
    S: OutboundSink,
{
    /// Route this send through the wire correlation slot for `actor`.
    /// Put the target actor UUID in the correlation slot. The framing layer
    /// covers it with the outer CRC32 alongside the envelope body.
    #[inline]
    pub fn for_actor(mut self, actor: uuid::Uuid) -> Self {
        self.context = actor;
        self
    }
}

impl<'a, M, S, P> IntoFuture for Outgoing<'a, M, S, P>
where
    M: Sendable<P>,
    S: OutboundSink,
    P: EgressPath,
{
    type Output = Result<()>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.enqueue())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Marshal;
    use crate::carrier::DataReliability;
    use crate::generated::messages::ClientAddEntryMsg;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct SentMessage {
        session: SessionId,
        bytes: Bytes,
        channel: u8,
        reliability: DataReliability,
        priority: usize,
    }

    #[derive(Default)]
    struct RecordingSink {
        sent: Mutex<Vec<SentMessage>>,
    }

    impl OutboundSink for RecordingSink {
        async fn enqueue(
            &self,
            session: SessionId,
            bytes: Bytes,
            channel: u8,
            reliability: DataReliability,
            priority: usize,
        ) -> Result<()> {
            self.sent
                .lock()
                .expect("recording sink mutex")
                .push(SentMessage {
                    session,
                    bytes,
                    channel,
                    reliability,
                    priority,
                });
            Ok(())
        }

        fn try_enqueue(
            &self,
            session: SessionId,
            bytes: Bytes,
            channel: u8,
            reliability: DataReliability,
            priority: usize,
        ) -> Result<()> {
            self.sent
                .lock()
                .expect("recording sink mutex")
                .push(SentMessage {
                    session,
                    bytes,
                    channel,
                    reliability,
                    priority,
                });
            Ok(())
        }
    }

    #[test]
    fn typed_state_bundle_event_borrow_decodes_payload() {
        let bundle = nw_network::ReplicatedStateBundle::default();
        let mut wb = crate::WriteBuffer::new(crate::serialize::CARRIER_ENDIAN);
        bundle.marshal(&mut wb);

        let event = Event::TypedReceived {
            session: SessionId::new(0),
            channel: crate::CarrierChannel::ReplicatedStateBundle.id(),
            type_index: REPLICATED_STATE_BUNDLE_TYPE_INDEX,
            data: Bytes::from(wb.into_vec()),
        };

        let view = event
            .state_bundle()
            .expect("state bundle view")
            .expect("state bundle typed event");

        assert_eq!(view.bundle_buffer, bundle.bundle_buffer.as_slice());
        assert_eq!(view.total_bundle_size(), bundle.total_bundle_size());
    }

    #[test]
    fn non_state_bundle_event_does_not_decode_payload() {
        let event = Event::Received {
            session: SessionId::new(0),
            channel: crate::CarrierChannel::GameData.id(),
            data: Bytes::new(),
        };

        assert!(event.state_bundle().expect("non-state event").is_none());
    }

    #[test]
    fn outgoing_enqueue_uses_native_future_path() {
        let sink = RecordingSink::default();
        let session = SessionId::new(7);
        let message = ClientAddEntryMsg {
            field_0: [0x42; 16],
        };

        futures_lite::future::block_on(
            Outgoing::<_, _, ClientToServer>::new(&sink, session, message)
                .with_priority(3)
                .enqueue(),
        )
        .expect("enqueue outgoing message");

        let sent = sink.sent.lock().expect("recording sink mutex");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].session, session);
        assert_eq!(sent[0].channel, crate::CarrierChannel::GameData.id());
        assert_eq!(sent[0].reliability, DataReliability::Reliable);
        assert_eq!(sent[0].priority, 3);
        assert!(!sent[0].bytes.is_empty());
    }
}
