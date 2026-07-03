//! Handshake retry timing.
//!
//! Direct port of Lumberyard's `GridMate::CarrierDesc::m_connectionRetry*`
//! fields plus the exponential-backoff helper. The carrier driver
//! consults [`retry_interval`] to decide how long to wait before
//! re-sending `SM_CONNECT_REQUEST` when the peer hasn't acknowledged.

use std::time::Duration;

/// Base retry interval (GridMate: `m_connectionRetryIntervalBase = 10`).
pub const RETRY_BASE_MS: u64 = 10;

/// Maximum retry interval (GridMate: `m_connectionRetryIntervalMax = 1000`).
pub const RETRY_MAX_MS: u64 = 1000;

/// Get the retry interval for the given retry count, with
/// exponential backoff capped at [`RETRY_MAX_MS`].
///
/// GridMate: `min(max, base * (1 << numRetries))`.
pub fn retry_interval(num_retries: u32) -> Duration {
    let interval_ms = std::cmp::min(
        RETRY_MAX_MS,
        // Cap `numRetries` at 10 so the shift can't overflow.
        RETRY_BASE_MS * (1u64 << num_retries.min(10)),
    );
    Duration::from_millis(interval_ms)
}
