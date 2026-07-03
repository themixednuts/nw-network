//! Driver error types
//!
//! Proper error handling for driver layer

use thiserror::Error;

/// Driver error type
#[derive(Debug, Error)]
pub enum DriverError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SSL/DTLS error: {0}")]
    Ssl(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Address error: {0}")]
    Address(String),

    #[error("Connection timeout")]
    Timeout,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),
}

impl DriverError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::ConnectionClosed | Self::Io(_))
    }
}
