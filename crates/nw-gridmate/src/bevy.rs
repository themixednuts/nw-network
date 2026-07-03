//! Bevy integration for the GridMate-compatible transport.
//!
//! The transport, framing, and session service are framework-agnostic: they
//! spawn through a [`Spawner`] abstraction and emit a plain [`Event`] stream.
//! This module is the Bevy-shaped wrapper:
//!
//! - [`GridMatePlugin`] bridges [`Event`] into Bevy's
//!   messaging system as [`NetEvent`].
//! - [`GridMateSessionService`] makes the session-service handle a
//!   Bevy `Resource`.
//! - [`SessionConnectionRequest`] is a `Component` that, when
//!   spawned, kicks off a `create_session` call.
//!
//! Embedders using a non-Bevy runtime (tokio, smol, async-std) should
//! install their own [`Spawner`] via [`crate::set_spawner`].

use crate::spawn::{BoxedFuture, Spawner};
use crate::{CarrierDesc, CarrierProtocolProfile, Event, SessionServiceHandle};
use bevy_app::{App, AppExit, Last, Plugin, PostStartup, Startup, Update};
use bevy_ecs::prelude::*;
use bevy_tasks::{IoTaskPool, Task};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::trace;

/// [`Spawner`] adapter for Bevy's `IoTaskPool`. Installed by
/// [`GridMatePlugin::build`] before any network handle is
/// constructed — non-Bevy embedders pick their own spawner (tokio,
/// smol, async-std) and call [`crate::set_spawner`] directly.
pub struct BevyIoTaskSpawner;

impl Spawner for BevyIoTaskSpawner {
    fn spawn(&self, future: BoxedFuture) {
        IoTaskPool::get().spawn(future).detach();
    }
}

/// GridMate networking plugin for Bevy.
///
/// Manages GridMate sessions as Bevy resources and bridges events
/// from the session service actor to Bevy's messaging system.
/// Supports unlimited concurrent connections through the actor-based
/// session service.
pub struct GridMatePlugin;

impl Plugin for GridMatePlugin {
    fn build(&self, app: &mut App) {
        // Install the Bevy `IoTaskPool` spawner exactly once.
        // `set_spawner` returns `Err` if some other plugin /
        // embedder already registered one — we treat that as
        // intentional and skip silently.
        let _ = crate::set_spawner(Arc::new(BevyIoTaskSpawner));

        app.init_resource::<GridMateSessionService>()
            .init_resource::<GridMateEventQueue>()
            .init_resource::<GridMateEventTask>()
            .add_message::<NetEvent>()
            .add_systems(Startup, init_session_service)
            .add_systems(PostStartup, start_gridmate_event_task)
            .add_systems(Update, (create_session_on_request, drain_event_queue))
            .add_systems(Last, gridmate_shutdown_system);
    }
}

/// Component marking an entity that should create a GridMate session.
#[derive(Component, Debug)]
pub struct SessionConnectionRequest {
    /// Server address (host:port).
    pub server: String,
    /// Optional path to CA certificate file.
    pub cert_path: Option<String>,
    /// Optional path to SSL keylog file (for debugging).
    pub ssl_keylog: Option<String>,
    /// Carrier wire profile selected by the project/runtime.
    pub protocol_profile: CarrierProtocolProfile,
}

impl SessionConnectionRequest {
    /// Create a connection request with the framework default carrier
    /// profile. Projects targeting a compatibility protocol should set
    /// [`Self::protocol_profile`] explicitly.
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            cert_path: None,
            ssl_keylog: None,
            protocol_profile: CarrierProtocolProfile::default(),
        }
    }
}

/// Creates a GridMate session when a `SessionConnectionRequest`
/// component is added.
fn create_session_on_request(
    session_service: Res<GridMateSessionService>,
    connection_requests: Query<&SessionConnectionRequest, Added<SessionConnectionRequest>>,
) {
    for request in connection_requests.iter() {
        let desc = build_carrier_desc(request);
        let Some(handle) = session_service.handle.clone() else {
            return;
        };

        IoTaskPool::get()
            .spawn(async move {
                match handle.create_session(desc).await {
                    Ok(index) => {
                        trace!("Created GridMate session {}", index);
                    }
                    Err(e) => {
                        trace!("Failed to create GridMate session: {}", e);
                    }
                }
            })
            .detach();
    }
}

/// Builds a `CarrierDesc` from a connection request.
fn build_carrier_desc(request: &SessionConnectionRequest) -> CarrierDesc {
    let mut desc = CarrierDesc::new(&request.server);
    desc.with_protocol_profile(request.protocol_profile);

    if let Some(cert) = &request.cert_path
        && !cert.is_empty()
    {
        desc.with_ca_cert(cert.clone());
    }

    if let Some(keylog) = &request.ssl_keylog
        && !keylog.is_empty()
    {
        desc.with_ssl_keylog(std::path::PathBuf::from(keylog));
    }

    desc
}

/// Wrapper for `SessionServiceHandle` as a Bevy resource.
///
/// The handle uses an actor pattern internally, providing thread-safe
/// access to session management operations without requiring mutex
/// locks. There is no registry resource to inject; class descriptors are
/// link-time registrations.
#[derive(Resource, Default)]
pub struct GridMateSessionService {
    pub handle: Option<SessionServiceHandle>,
}

/// Initialize `SessionServiceHandle` after Bevy task pools are ready.
///
fn init_session_service(mut session_service: ResMut<GridMateSessionService>) {
    if session_service.handle.is_none() {
        session_service.handle = Some(SessionServiceHandle::new());
    }
}

/// Bevy message wrapper for GridMate network events.
///
/// Thin wrapper around [`Event`] for Bevy's messaging system.
/// Events are bridged directly from the session service actor with
/// zero-copy semantics. The `Bytes` type uses `Arc` internally, so
/// when Bevy clones messages for broadcasting, only the reference
/// counter is incremented.
#[derive(Debug, Clone, Message)]
pub struct NetEvent(pub Event);

const EVENT_QUEUE_CAPACITY: usize = 4096;
const MAX_EVENTS_PER_FRAME: usize = 512;

#[derive(Resource)]
struct GridMateEventQueue {
    sender: async_channel::Sender<Event>,
    receiver: async_channel::Receiver<Event>,
    pending: Arc<AtomicBool>,
}

impl Default for GridMateEventQueue {
    fn default() -> Self {
        let (sender, receiver) = async_channel::bounded(EVENT_QUEUE_CAPACITY);
        Self {
            sender,
            receiver,
            pending: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Long-lived task handle for the GridMate event bridge.
#[derive(Default, Resource)]
struct GridMateEventTask {
    _task: Option<Arc<Task<()>>>,
}

/// Starts the long-lived GridMate event bridge task.
///
/// This task waits on the actor's event channel and forwards events
/// into Bevy without polling or sleeps.
fn start_gridmate_event_task(
    session_service: Res<GridMateSessionService>,
    event_queue: Res<GridMateEventQueue>,
    mut event_task: ResMut<GridMateEventTask>,
) {
    let Some(handle) = session_service.handle.clone() else {
        trace!("GridMate event task: handle not available!");
        return;
    };

    trace!("Starting GridMate event task");

    let sender = event_queue.sender.clone();
    let pending = event_queue.pending.clone();
    let task = IoTaskPool::get().spawn(async move {
        while let Some(event) = handle.recv_event().await {
            if sender.send(event).await.is_ok() {
                pending.store(true, Ordering::Release);
            }
        }
    });

    event_task._task = Some(Arc::new(task));
}

fn drain_event_queue(event_queue: Res<GridMateEventQueue>, mut writer: MessageWriter<NetEvent>) {
    if !event_queue.pending.swap(false, Ordering::Acquire) {
        return;
    }

    let mut drained = 0usize;
    while drained < MAX_EVENTS_PER_FRAME {
        match event_queue.receiver.try_recv() {
            Ok(event) => {
                writer.write(NetEvent(event));
                drained += 1;
            }
            Err(async_channel::TryRecvError::Empty) => break,
            Err(async_channel::TryRecvError::Closed) => break,
        }
    }

    if !event_queue.receiver.is_empty() {
        event_queue.pending.store(true, Ordering::Release);
    }
}

/// Handles graceful shutdown of all GridMate sessions.
///
/// Listens for `AppExit` messages and disconnects all sessions
/// cleanly before the application terminates. Ensures proper cleanup
/// of network resources.
fn gridmate_shutdown_system(
    session_service: Res<GridMateSessionService>,
    mut exit_reader: MessageReader<AppExit>,
    mut disconnected: Local<bool>,
) {
    if *disconnected {
        return;
    }

    if exit_reader.read().next().is_some() {
        *disconnected = true;
        trace!("Initiating graceful shutdown of all GridMate sessions");

        let Some(handle) = session_service.handle.clone() else {
            return;
        };

        IoTaskPool::get()
            .spawn(async move {
                if let Err(e) = handle.disconnect_all().await {
                    trace!("Error during graceful shutdown: {}", e);
                } else {
                    let count = handle.num_sessions().await;
                    trace!("Gracefully disconnected {} session(s)", count);
                }
            })
            .detach();
    }
}
