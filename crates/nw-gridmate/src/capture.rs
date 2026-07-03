//! Wire capture sidecar: when `NW_NETWORK_CAPTURE` is set, every
//! outbound and inbound envelope is persisted to disk alongside a
//! JSONL metadata trail.
//!
//! Compiled out of release builds entirely so the transport hot path
//! has no env-var lookups, no `format!` calls, and no filesystem I/O.
//! The session-service actor calls these from its send / receive
//! paths via thin `cfg(debug_assertions)` gates — release builds
//! skip the call sites and link out the capture module.
//!
//! The capture path is intentionally a sidecar: the session-service
//! actor owns wire orchestration, not diagnostics, so we keep the
//! file-format details out of the actor module entirely. The
//! sequence-number counters live on a [`CaptureCounters`] value the
//! actor threads through every call.
//!
//! # Wire format
//!
//! Per-message binary files:
//!
//! ```text
//! send_{seq:06}_T0x{type_index:x}_S{session}.bin  // outbound body
//! recv_{seq:06}_T0x{type_index:x}_S{session}.bin  // inbound envelope raw bytes
//! type_{type_index}.bin                            // first-of-type inbound body fixture
//! ```
//!
//! Plus `messages.jsonl` — one record per direction-tagged event.

#![cfg(debug_assertions)]

use crate::carrier::DataReliability;
use crate::session_service::SessionId;
use bytes::Bytes;
use std::sync::OnceLock;
use tracing::trace;

/// Cached `NW_NETWORK_CAPTURE` env var. Resolved at first access and
/// never re-read; saves a syscall + string allocation per message.
pub fn capture_dir() -> Option<&'static str> {
    static DIR: OnceLock<Option<String>> = OnceLock::new();
    DIR.get_or_init(|| std::env::var("NW_NETWORK_CAPTURE").ok())
        .as_deref()
}

/// Monotonic per-direction counter the actor advances on each call.
/// Capture filenames embed the counter so on-disk order matches wire
/// order even when filesystem mtime resolution is too coarse.
#[derive(Default)]
pub struct CaptureCounters {
    send_seq: u64,
    recv_seq: u64,
}

impl CaptureCounters {
    /// Capture one outbound message. No-op when `NW_NETWORK_CAPTURE`
    /// is unset (the common case).
    #[allow(clippy::too_many_arguments)]
    pub fn capture_send(
        &mut self,
        session_id: SessionId,
        data: &Bytes,
        channel: u8,
        reliability: DataReliability,
        priority: usize,
        type_index: Option<u32>,
    ) {
        let Some(cap_dir) = capture_dir() else {
            return;
        };

        use std::io::Write;
        self.send_seq += 1;
        let seq = self.send_seq;

        let _ = std::fs::create_dir_all(cap_dir);
        let type_part = type_index
            .map(|type_index| format!("T0x{type_index:x}"))
            .unwrap_or_else(|| "Tunknown".to_string());
        let bin_path = format!("{cap_dir}/send_{seq:06}_{type_part}_S{session_id}.bin");
        let _ = std::fs::write(&bin_path, data.as_ref());

        let log_path = format!("{cap_dir}/messages.jsonl");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let hex_preview = hex::encode(&data[..std::cmp::min(256, data.len())]);
            let type_json = type_index
                .map(|type_index| type_index.to_string())
                .unwrap_or_else(|| "null".to_string());
            let _ = writeln!(
                f,
                r#"{{"ts":{},"dir":"W","seq":{},"session":{},"type":{},"size":{},"channel":{},"reliability":"{:?}","priority":{},"file":"{}","hex":"{}"}}"#,
                ts,
                seq,
                *session_id,
                type_json,
                data.len(),
                channel,
                reliability,
                priority,
                bin_path.replace('\\', "/"),
                hex_preview
            );
        }

        if let Some(type_index) = type_index {
            trace!(
                "[CAPTURE] Saved send seq={} type={} to {} ({} bytes)",
                seq,
                type_index,
                bin_path,
                data.len()
            );
        } else {
            trace!(
                "[CAPTURE] Saved send seq={} type=unknown to {} ({} bytes)",
                seq,
                bin_path,
                data.len()
            );
        }
    }

    /// Capture one inbound envelope: full envelope bytes + a
    /// first-of-type body fixture + a JSONL line. No-op when
    /// `NW_NETWORK_CAPTURE` is unset.
    pub fn capture_recv(
        &mut self,
        session_id: SessionId,
        type_index: u32,
        envelope_raw: &[u8],
        body: &Bytes,
    ) {
        let Some(cap_dir) = capture_dir() else {
            return;
        };

        use std::io::Write;
        let _ = std::fs::create_dir_all(cap_dir);

        self.recv_seq += 1;
        let seq = self.recv_seq;
        let recv_path = format!("{cap_dir}/recv_{seq:06}_T0x{type_index:x}_S{session_id}.bin");
        let _ = std::fs::write(&recv_path, envelope_raw);

        // First-of-type body fixture — useful for round-trip tests.
        let bin_path = format!("{cap_dir}/type_{type_index}.bin");
        if !std::path::Path::new(&bin_path).exists() {
            let _ = std::fs::write(&bin_path, body.as_ref());
            trace!(
                "[CAPTURE] Saved first type={} to {} ({} bytes)",
                type_index,
                bin_path,
                body.len()
            );
        }

        let log_path = format!("{cap_dir}/messages.jsonl");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let hex_preview = hex::encode(&body[..std::cmp::min(64, body.len())]);
            let _ = writeln!(
                f,
                r#"{{"ts":{},"dir":"R","seq":{},"session":{},"type":{},"size":{},"file":"{}","hex":"{}"}}"#,
                ts,
                seq,
                *session_id,
                type_index,
                body.len(),
                recv_path.replace('\\', "/"),
                hex_preview
            );
        }
    }
}
