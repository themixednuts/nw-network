// CarrierDriver - Main Carrier implementation
// Following GridMate Carrier.cpp CarrierDriver class
//
// GridMate pattern: CarrierDriver is instantiated by CarrierImpl and runs ThreadPump
// Rust pattern: Instance struct with async event loop, runs on IoTaskPool
//
// Event-driven architecture:
// - Awaits directly on network I/O and command channels
// - No fixed-interval polling - responds immediately to events
// - Timers only used for keepalive/retry logic

use super::connection_state::ConnectionState;
use super::io::CarrierTransport;
use super::message::{DataReliability, MessageData, MessageFlags};
use super::thread_message::{CarrierCommand, CarrierEvent};
use super::types::{DisconnectReason, MAX_CHANNELS, SYSTEM_CHANNEL, SequenceNumber};
use crate::serialize::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};
use async_channel::{Receiver, Sender};
use async_io::Timer;
use futures_lite::StreamExt;
use std::io::Write;
use std::sync::Arc as StdArc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{debug, trace};

/// Message header field sizes (GridMate: GetMaxMessageHeaderSize constants)
mod header_sizes {
    use super::SequenceNumber;
    use std::mem::size_of;

    pub const FLAGS: usize = size_of::<u8>();
    pub const DATA_SIZE: usize = size_of::<u16>();
    pub const CHANNEL_INFO: usize = size_of::<u8>();
    pub const SPLIT_PACKET_INFO: usize = size_of::<SequenceNumber>();
    pub const SEQUENCE_NUMBER: usize = size_of::<SequenceNumber>();
    pub const SEQUENCE_RELIABLE_NUMBER: usize = size_of::<SequenceNumber>();
}

/// Which side of the GridMate handshake this carrier is on.
///
/// Lumberyard's `Carrier.cpp` is symmetric — the same `CarrierDriver`
/// runs whether the connection was established by `Connect` or
/// `OnNewConnection`. The only behavioural difference is who sends
/// `SM_CONNECT_REQUEST` first; the receive-side reply with
/// `SM_CONNECT_ACK` is identical on both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierRole {
    /// We initiated the connection — send `SM_CONNECT_REQUEST` and
    /// retry on the handshake-retry timer until we get an ACK back.
    /// Used by [`super::CarrierImpl::new`] (client / outbound).
    Initiator,
    /// We accepted an incoming connection — wait for the peer's
    /// `SM_CONNECT_REQUEST` to arrive, then reply once with
    /// `SM_CONNECT_ACK`. Used by `CarrierImpl::accept` (server /
    /// inbound).
    Responder,
}

/// Resolve the session directory used by [`CarrierDriver::capture_datagram`].
///
/// `parent` is taken from the `NW_NETWORK_CAPTURE` env var verbatim. If
/// the path looks like an existing per-session dir (e.g. it already
/// contains a `carrier/` subdir) we reuse it; otherwise a fresh
/// `${ISO_TS}-pid${PID}` session subdir is created beneath it so
/// successive launches of `just local-stack-up` don't clobber each
/// other's manifests.
///
/// Writes `manifest.json` once on creation with the schema-versioned
/// session metadata that matches `nw-hooks-dll`'s sink — `cap` reads
/// either source through one code path.
fn open_capture_session(parent: &std::path::Path) -> std::path::PathBuf {
    // If the caller already pointed us at a session-shaped dir (it has
    // a `manifest.json` or a `carrier/` child), reuse it. Otherwise
    // append a fresh `${ts}-pid${pid}` session subdir.
    let session_dir = if parent.join("manifest.json").exists() || parent.join("carrier").exists() {
        parent.to_path_buf()
    } else {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let ts = capture_iso_compact(secs);
        let pid = std::process::id();
        parent.join(format!("{ts}-pid{pid}"))
    };

    let _ = std::fs::create_dir_all(session_dir.join("carrier"));

    let manifest_path = session_dir.join("manifest.json");
    if !manifest_path.exists() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let started_at = capture_iso_full(secs);
        let pid = std::process::id();
        // Mirror nw-hooks-dll's `serde_event` shape exactly — same keys,
        // same string-only values, so a `cap` reader can parse both.
        let manifest = format!(
            r#"{{"schema":"1","kind":"vendor","pid":"{pid}","started_at":"{started_at}","source":"gridmate-server","session_dir":"{session}"}}"#,
            session = session_dir.to_string_lossy().replace('\\', "/"),
        );
        let _ = std::fs::write(&manifest_path, manifest);
    }

    session_dir
}

fn capture_iso_compact(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = secs_to_ymdhms(secs);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

fn capture_iso_full(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = secs_to_ymdhms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Howard-Hinnant `days_from_civil` inverse — kept in-crate to avoid a
/// `chrono` dependency in `gridmate`.
fn secs_to_ymdhms(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let day = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let h = (time_of_day / 3_600) as u32;
    let mi = ((time_of_day % 3_600) / 60) as u32;
    let s = (time_of_day % 60) as u32;
    let z = day + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe
        .wrapping_sub(doe / 1_460)
        .wrapping_sub(doe / 36_524)
        .wrapping_add(doe / 146_096))
        / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y as u32, mo, d, h, mi, s)
}

/// CarrierDriver - Background thread managing network I/O (GridMate: CarrierDriver class)
///
/// Event-driven architecture:
/// - Awaits directly on network data and commands (no polling)
/// - Processes events immediately as they arrive
/// - Uses keepalive timer only for connection health
pub struct CarrierDriver<T: CarrierTransport> {
    /// Per-peer connection state, generic over the byte transport.
    thread_conn: ConnectionState<T>,

    /// Channel for receiving from main
    from_main_rx: Receiver<CarrierCommand>,

    /// Channel for sending to main
    to_main_tx: Sender<CarrierEvent>,

    /// Shutdown flag
    shutdown_flag: StdArc<AtomicBool>,

    /// Cleanup completion signal
    cleanup_done: Sender<()>,

    /// Initiator vs responder. Gates `handshake_retry` (initiator-only)
    /// and triggers the inbound-`SM_CONNECT_REQUEST` reply path
    /// (responder-only).
    role: CarrierRole,

    /// Monotonic capture sequence across both directions — matches the
    /// `nw-hooks-dll` sink so a session's `carrier/index.jsonl` sorts
    /// chronologically regardless of which direction the datagram went.
    send_datagram_capture_seq: u64,

    /// Lazily opened session directory for the shared on-disk capture
    /// schema (see `capture_datagram`). `None` until the first capture
    /// fires; subsequent captures append into the same session.
    capture_session_dir: Option<std::path::PathBuf>,
}

impl<T: CarrierTransport> CarrierDriver<T> {
    /// Construct a carrier driver task. Internal: gridmate's
    /// `CarrierImpl::new` / `CarrierImpl::accept` are the entry
    /// points consumers go through.
    pub(crate) fn new(
        thread_conn: ConnectionState<T>,
        from_main_rx: Receiver<CarrierCommand>,
        to_main_tx: Sender<CarrierEvent>,
        shutdown_flag: StdArc<AtomicBool>,
        cleanup_done: Sender<()>,
        role: CarrierRole,
    ) -> Self {
        Self {
            thread_conn,
            role,
            from_main_rx,
            to_main_tx,
            shutdown_flag,
            cleanup_done,
            send_datagram_capture_seq: 0,
            capture_session_dir: None,
        }
    }

    /// Run the carrier thread event loop
    ///
    /// Timing model (matches Lumberyard's CarrierDriver::ThreadPump):
    ///
    /// - **NetworkData**: Socket becomes readable → batch-drain ALL available
    ///   UDP packets in a tight loop via `try_recv_decrypt()`, then send ONE
    ///   combined ACK covering all received datagrams. Matches Lumberyard's
    ///   `UpdateReceive` which drains the socket completely before `UpdateSend`.
    ///
    /// - **AckCheck (15ms)**: Matches Lumberyard's `k_timerResolutionMS`.
    ///   Checks `IsSendACKOnly` (received data since last send OR 100ms
    ///   elapsed), drains ACK history via `mark_acked()`, and forwards
    ///   received messages. Equivalent to `ProcessConnections`.
    ///
    /// - **Heartbeat (100ms)**: Keepalive ceiling. Matches Lumberyard's
    ///   `m_lostPacketTimeoutMS / 10 = 1000 / 10`. Ensures the connection
    ///   stays alive even without application traffic.
    ///
    /// - **HandshakeRetry**: Exponential backoff during connection setup.
    ///
    /// - **Command**: Messages from the main thread (send, disconnect, etc.).
    pub async fn run(mut self) {
        debug!("[CarrierDriver] Event-driven run() started");

        // Lumberyard: k_timerResolutionMS = 15ms — ProcessConnections cadence
        let mut ack_check = Timer::interval(Duration::from_millis(15));
        // Lumberyard: m_lostPacketTimeoutMS / 10 = 100ms — keepalive ceiling
        let mut heartbeat = Timer::interval(Duration::from_millis(100));

        loop {
            if self.shutdown_flag.load(Ordering::Relaxed) {
                debug!("[CarrierDriver] Shutdown flag set, exiting");
                break;
            }

            enum Event {
                NetworkData(Result<bytes::Bytes, crate::driver::DriverError>),
                Command(Result<CarrierCommand, async_channel::RecvError>),
                HandshakeRetry,
                AckCheck,
                Heartbeat,
            }

            // Handshake retry timer: only active during connecting phase
            let retry_duration = self
                .thread_conn
                .time_until_retry()
                .unwrap_or(Duration::from_secs(3600));
            let handshake_retry_fut = Timer::after(retry_duration);

            let network_fut = self.thread_conn.io.read();
            let cmd_fut = self.from_main_rx.recv();
            let ack_check_fut = ack_check.next();
            let heartbeat_fut = heartbeat.next();

            let event = {
                use futures_util::FutureExt as FutureFuse;
                use futures_util::select;
                select! {
                    result = network_fut.fuse() => Event::NetworkData(result),
                    cmd = cmd_fut.fuse() => Event::Command(cmd),
                    _ = FutureFuse::fuse(handshake_retry_fut) => Event::HandshakeRetry,
                    _ = ack_check_fut.fuse() => Event::AckCheck,
                    _ = heartbeat_fut.fuse() => Event::Heartbeat,
                }
            };

            match event {
                Event::NetworkData(result) => {
                    // Process the first packet that woke us
                    if let Err(should_break) = self.handle_network_data(result).await
                        && should_break
                    {
                        break;
                    }

                    // Lumberyard batch-drain: drain ALL queued UDP packets
                    // before sending a combined ACK. Matches UpdateReceive's
                    // tight loop that calls recv() until WouldBlock.
                    loop {
                        match self.thread_conn.io.try_recv() {
                            Ok(Some(plaintext)) => {
                                if let Err(should_break) =
                                    self.handle_network_data(Ok(plaintext)).await
                                    && should_break
                                {
                                    break;
                                }
                            }
                            Ok(None) => break, // No more queued packets
                            Err(e) => {
                                trace!("[CarrierDriver] Batch drain error: {:?}", e);
                                break;
                            }
                        }
                    }

                    // Forward parsed messages first so that system-message
                    // handlers (e.g. SM_CONNECT_REQUEST → queue
                    // SM_CONNECT_ACK on the responder side) can stuff
                    // replies into `to_send` before this tick's flush.
                    // Without this ordering, the reply waits for the
                    // next 15 ms AckCheck — which means the peer sees
                    // an ACK datagram first and the carrier handshake
                    // takes a tick longer than necessary.
                    self.forward_messages().await;
                    self.thread_conn.process_resends();
                    self.send_pending_data().await;
                }
                Event::Command(result) => {
                    match result {
                        Ok(msg) => {
                            if let Err(e) = self.handle_command(msg).await
                                && e == "shutdown"
                            {
                                break;
                            }
                        }
                        Err(_) => break, // Channel closed
                    }
                }
                Event::HandshakeRetry => {
                    if self.thread_conn.is_connecting {
                        self.handle_handshake_retry().await;
                    }
                }
                Event::AckCheck => {
                    // Lumberyard: ProcessConnections every 15ms
                    // ACK history drain is integrated into write_ack_data(),
                    // matching Lumberyard's WriteAckData/GetForAck path.
                    self.thread_conn.process_resends();
                    // IsSendACKOnly: send ACK-only if we received data or
                    // 100ms elapsed since last ACK (handled in prepare_outgoing_datagram)
                    self.send_pending_data().await;
                    self.forward_messages().await;
                }
                Event::Heartbeat => {
                    // 100ms keepalive ceiling — ensure connection stays alive
                    self.thread_conn.process_resends();
                    self.send_pending_data().await;
                    self.forward_messages().await;
                }
            }
        }

        // Cleanup — `io` Drop closes the underlying connection /
        // channels; nothing else to do here besides signalling.
        let _ = self.cleanup_done.send(()).await;
    }

    /// Handle handshake retry - resend SM_CONNECT_REQUEST with exponential backoff
    ///
    /// GridMate pattern: PendingHandshake retry loop
    /// - Resend SM_CONNECT_REQUEST with increasing payload size
    /// - Schedule next retry with exponential backoff (10, 20, 40... up to 1000ms)
    ///
    /// Initiator-only: the responder side of the handshake never
    /// emits `SM_CONNECT_REQUEST`, it only replies to one (handled
    /// via [`Self::reply_with_connect_ack`] on inbound).
    async fn handle_handshake_retry(&mut self) {
        if self.role != CarrierRole::Initiator {
            return;
        }
        let retry_num = self.thread_conn.handshake_num_retries;
        debug!(
            "[CarrierDriver] Handshake retry #{} (next in {}ms)",
            retry_num,
            super::handshake_retry::retry_interval(retry_num + 1).as_millis()
        );

        // Queue SM_CONNECT_REQUEST with current retry count
        self.thread_conn.queue_connect_request();

        // Queue SM_CT_ACKS keepalive: [ACK_FLAGS=0x20][SM_CT_ACKS msg_id=0x06]
        let ack_msg = MessageData {
            channel: SYSTEM_CHANNEL,
            reliability: DataReliability::Unreliable,
            is_connecting: true,
            data: bytes::Bytes::from_static(&[0x20, 0x06]),
            ..Default::default()
        };
        self.thread_conn.to_send[0].push_back(ack_msg);

        // Send the queued messages immediately
        self.send_pending_data().await;

        // Schedule next retry
        self.thread_conn.schedule_next_retry();
    }

    /// Handle received network data
    async fn handle_network_data(
        &mut self,
        result: Result<bytes::Bytes, crate::driver::DriverError>,
    ) -> Result<(), bool> {
        match result {
            Ok(plaintext) => {
                self.capture_datagram("R", &plaintext);
                debug!(
                    "[CarrierDriver] Received {} bytes: {:02x?}",
                    plaintext.len(),
                    &plaintext[..std::cmp::min(32, plaintext.len())]
                );

                // Process datagram immediately
                match self.thread_conn.process_incoming_datagram(plaintext) {
                    Ok(()) => {}
                    Err(e) => {
                        trace!("[CarrierDriver] Datagram processing error: {}", e);
                        let _ = self
                            .to_main_tx
                            .send(CarrierEvent::Error {
                                description: format!("Datagram error: {}", e),
                            })
                            .await;
                    }
                }

                Ok(())
            }
            Err(e) => {
                if e.is_retryable() {
                    // Timeout or no data - not an error
                    Ok(())
                } else {
                    let _ = self
                        .to_main_tx
                        .send(CarrierEvent::Error {
                            description: format!("DTLS error: {:?}", e),
                        })
                        .await;
                    Err(true) // Should break
                }
            }
        }
    }

    /// Forward received messages to main thread
    async fn forward_messages(&mut self) {
        // Forward all available messages from all channels
        for channel in 0..MAX_CHANNELS as u8 {
            loop {
                use futures_lite::StreamExt;
                let mut stream = self.thread_conn.receive_stream(channel);

                match stream.next().await {
                    Some(Ok(msg)) => {
                        if channel == SYSTEM_CHANNEL {
                            self.handle_system_message(msg.data).await;
                        } else {
                            let _ = self
                                .to_main_tx
                                .send(CarrierEvent::MessageReceived {
                                    channel,
                                    data: msg.data,
                                })
                                .await;
                        }
                    }
                    _ => break, // No more messages on this channel
                }
            }
        }
    }

    /// Handle system messages (connect REQUEST/ACK, disconnect, etc.)
    /// Takes ownership of data to avoid Send issues with MessageData's ack_callback
    async fn handle_system_message(&mut self, data: bytes::Bytes) {
        if data.is_empty() {
            return;
        }

        let msg_id = data[data.len() - 1];

        match msg_id {
            super::system_message::SM_CONNECT_REQUEST
                if data.len() >= 5 && self.role == CarrierRole::Responder =>
            {
                // Lumberyard's `Carrier.cpp` line 4413 path:
                // `m_handshake->OnReceiveRequest(...)` validates the
                // peer's request, then the carrier sends
                // `SM_CONNECT_ACK` back. We're the symmetric end of
                // the same handshake — initiator does the same on
                // SM_CONNECT_ACK receive, see arm below.
                let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &data[..data.len() - 1]);
                let Ok(version) = rb.read_u32() else { return };
                let expected_version = self.thread_conn.protocol_version();
                if version != expected_version {
                    let _ = self
                        .to_main_tx
                        .send(CarrierEvent::Error {
                            description: format!(
                                "Peer SM_CONNECT_REQUEST version mismatch: {version} != {}",
                                expected_version,
                            ),
                        })
                        .await;
                    return;
                }
                // Lumberyard `Carrier.cpp:~4413` guards the ACK send
                // with `if (conn->m_state != Carrier::CST_CONNECTED)` —
                // duplicate `SM_CONNECT_REQUEST`s (launcher retries the
                // request unreliably until it sees an ACK) are silently
                trace!(
                    "SM_CONNECT_REQUEST v{} (responder) → queueing SM_CONNECT_ACK",
                    version
                );
                self.thread_conn.queue_connect_ack();
                self.thread_conn.is_connecting = false;
                let _ = self
                    .to_main_tx
                    .send(CarrierEvent::Connected { version })
                    .await;
            }
            0x02 if data.len() >= 5 => {
                // SM_CONNECT_ACK
                let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &data[..data.len() - 1]);
                if let Ok(version) = rb.read_u32() {
                    let expected_version = self.thread_conn.protocol_version();
                    if version == expected_version {
                        trace!("SM_CONNECT_ACK v{}", version);
                        // Transition out of connecting phase
                        self.thread_conn.is_connecting = false;
                        let _ = self
                            .to_main_tx
                            .send(CarrierEvent::Connected { version })
                            .await;
                    } else {
                        let _ = self
                            .to_main_tx
                            .send(CarrierEvent::Error {
                                description: format!(
                                    "Version mismatch: {} != {}",
                                    version, expected_version
                                ),
                            })
                            .await;
                    }
                }
            }
            0x03 => {
                // SM_DISCONNECT
                let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &data[..data.len() - 1]);
                let raw_reason = rb.read_u8().ok();
                let reason = raw_reason
                    .and_then(|v| DisconnectReason::try_from(v).ok())
                    .unwrap_or(DisconnectReason::BadPackets);

                trace!(
                    "Server disconnected: {:?} (raw: {:?}, data: {:02x?})",
                    reason, raw_reason, &data
                );
                let _ = self
                    .to_main_tx
                    .send(CarrierEvent::Disconnected { reason })
                    .await;
                self.shutdown_flag.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Send all pending outgoing data
    async fn send_pending_data(&mut self) {
        // Send all queued messages
        loop {
            // Use GridMate MTU calculation (fragments large messages automatically)
            match self
                .thread_conn
                .prepare_outgoing_datagram(super::mtu::MAX_DATAGRAM_SIZE)
            {
                Ok(Some(datagram)) => {
                    self.capture_datagram("W", &datagram);
                    debug!(
                        "[CarrierDriver] Sending {} bytes: {:02x?}",
                        datagram.len(),
                        &datagram[..std::cmp::min(32, datagram.len())]
                    );
                    if let Err(e) = self.thread_conn.io.write(datagram.clone()).await {
                        trace!("[CarrierDriver] Send error: {:?}", e);
                        let _ = self
                            .to_main_tx
                            .send(CarrierEvent::Error {
                                description: format!("Send error: {:?}", e),
                            })
                            .await;
                        break;
                    }
                }
                Ok(None) => break, // No more data to send
                Err(e) => {
                    trace!("[CarrierDriver] Prepare error: {:?}", e);
                    break;
                }
            }
        }
    }

    /// Append one outbound or inbound carrier datagram to the active
    /// capture session.
    ///
    /// On-disk layout matches the shared schema the `nw-hooks-dll`
    /// client-side sink uses, so `cap` reads either source through the
    /// same code path:
    ///
    /// ```text
    /// ${NW_NETWORK_CAPTURE}/${ISO_TS}-pid${PID}/
    ///   manifest.json         # written once on first call
    ///   carrier/
    ///     index.jsonl         # one JSON line per datagram
    ///     000001_W.bin        # outbound payload (this PID is the server)
    ///     000002_R.bin        # inbound payload
    ///     …
    /// ```
    ///
    /// Filename sequence is monotonic across both directions (no separate
    /// `send_*` / `recv_*` counters). `dir` is `"W"` (server → client) or
    /// `"R"` (client → server). Direction labels are from the server's
    /// perspective, mirroring the DLL's "client-perspective" labels so
    /// `W` always means "this side sent it" regardless of who captured.
    ///
    /// `NW_NETWORK_CAPTURE` may name either a parent directory (in which
    /// case a `${ISO_TS}-pid${PID}` session subdir is created beneath it)
    /// or be unset, in which case the fallback is `resources/captures/vendor/`
    /// relative to the server's CWD. The session subdir keeps multiple
    /// restarts of `just local-stack-up` from clobbering each other.
    /// Diagnostic-only: filesystem-writing capture sink. The whole
    /// body is gated on `debug_assertions` so release builds compile
    /// it out and the carrier hot path pays nothing.
    #[allow(unused_variables)]
    fn capture_datagram(&mut self, dir: &str, data: &[u8]) {
        #[cfg(debug_assertions)]
        {
            let parent = match std::env::var("NW_NETWORK_CAPTURE") {
                Ok(s) if !s.is_empty() => std::path::PathBuf::from(s),
                _ => return,
            };

            let session_dir = self
                .capture_session_dir
                .get_or_insert_with(|| open_capture_session(&parent));
            let session_dir = session_dir.clone();
            let carrier_dir = session_dir.join("carrier");

            // Monotonic across both directions matches the DLL sink.
            let seq = {
                self.send_datagram_capture_seq += 1;
                self.send_datagram_capture_seq
            };

            let file_name = format!("{seq:06}_{dir}.bin");
            let bin_path = carrier_dir.join(&file_name);
            if let Err(e) = std::fs::write(&bin_path, data) {
                trace!(
                    "capture_datagram: write {} failed: {}",
                    bin_path.display(),
                    e
                );
                return;
            }

            let index_path = carrier_dir.join("index.jsonl");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&index_path)
            {
                let ts_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                // Properly-typed single-line JSON so the `cap` tool's
                // serde reader can deserialize without coercing strings
                // to numbers. Numeric fields are numbers; `dir` / `file`
                // are strings.
                let _ = writeln!(
                    f,
                    r#"{{"seq":{},"dir":"{}","ts_ms":{},"size":{},"file":"{}"}}"#,
                    seq,
                    dir,
                    ts_ms,
                    data.len(),
                    file_name,
                );
            }
        }
    }

    /// Handle command from main thread
    async fn handle_command(&mut self, msg: CarrierCommand) -> Result<(), String> {
        match msg {
            CarrierCommand::SendMessage { message, priority } => {
                debug!(
                    "[CarrierDriver] Queue message ch:{} len:{} priority:{}",
                    message.channel,
                    message.data.len(),
                    priority
                );
                self.thread_conn.queue_message(message, priority);

                // Additional messages will be handled by the main loop.

                // Send all batched messages in one or more datagrams
                self.send_pending_data().await;
                Ok(())
            }
            CarrierCommand::Disconnect(reason) => {
                let sys_msg = Self::disconnect_message(reason);
                self.thread_conn.queue_message(sys_msg, 0);
                self.send_pending_data().await;

                let _ = self
                    .to_main_tx
                    .send(CarrierEvent::Disconnected { reason })
                    .await;
                self.shutdown_flag.store(true, Ordering::Relaxed);
                Err("shutdown".to_string())
            }
        }
    }

    /// Create disconnect system message
    fn disconnect_message(reason: DisconnectReason) -> MessageData {
        use bytes::BytesMut;
        let mut payload = BytesMut::with_capacity(2);
        payload.extend_from_slice(&[reason as u8, 0x03]); // reason + SM_DISCONNECT

        MessageData {
            channel: SYSTEM_CHANNEL,
            reliability: DataReliability::Unreliable,
            data: payload.freeze(),
            ..Default::default()
        }
    }
}

// ============================================================================
// Message header helpers — free functions because they don't depend on
// the transport `T` and end up in the hot path of both the driver and
// the connection-state's datagram assembler. Free functions sidestep
// the `T` inference noise that `CarrierDriver::write_header` would
// inflict on every call site.
// ============================================================================

/// Write a message header to the outbound buffer.
pub fn write_message_header(
    buffer: &mut WriteBuffer,
    msg: &MessageData,
    is_write_seq_number: bool,
    is_write_reliable_seq_num: bool,
    is_write_channel: bool,
) {
    let mut flags: u8 = 0;

    if msg.reliability == DataReliability::Reliable {
        flags |= MessageFlags::Reliable as u8;
    }
    if msg.num_chunks.get() > 1 {
        flags |= MessageFlags::Chunks as u8;
    }
    if !is_write_seq_number {
        flags |= MessageFlags::SequentialId as u8;
    }
    if !is_write_reliable_seq_num {
        flags |= MessageFlags::SequentialRelId as u8;
    }
    if is_write_channel {
        flags |= MessageFlags::DataChannel as u8;
    }
    if msg.is_connecting {
        flags |= MessageFlags::Connecting as u8;
    }

    buffer.write_u8(flags);
    buffer.write_u16(msg.data.len() as u16);

    if is_write_channel {
        buffer.write_u8(msg.channel);
    }
    if msg.num_chunks.get() > 1 {
        buffer.write_u16(msg.num_chunks.get());
    }
    if is_write_seq_number {
        buffer.write_u16(msg.sequence_number.get());
    }
    if is_write_reliable_seq_num {
        buffer.write_u16(msg.send_reliable_seq_num.get());
    }
}

/// Calculate the message header size for `msg` with the given
/// "sequential id / reliable id / channel" elision flags.
pub fn message_header_size(
    msg: &MessageData,
    is_write_seq_number: bool,
    is_write_reliable_seq_num: bool,
    is_write_channel: bool,
) -> usize {
    let mut size = header_sizes::FLAGS + header_sizes::DATA_SIZE;

    if is_write_channel {
        size += header_sizes::CHANNEL_INFO;
    }
    if msg.num_chunks.get() > 1 {
        size += header_sizes::SPLIT_PACKET_INFO;
    }
    if is_write_seq_number {
        size += header_sizes::SEQUENCE_NUMBER;
    }
    if is_write_reliable_seq_num {
        size += header_sizes::SEQUENCE_RELIABLE_NUMBER;
    }

    size
}
