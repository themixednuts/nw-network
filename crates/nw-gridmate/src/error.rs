use std::io;

use thiserror::Error;

/// Error type for the GridMate-compatible transport and session layers.
#[derive(Debug, Error)]
pub enum GridMateError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("driver error: {0}")]
    Driver(#[from] crate::driver::error::DriverError),

    #[error("SSL/DTLS error: {0}")]
    Ssl(String),

    #[error("operation timed out")]
    Timeout,

    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("message type {type_name} ({type_index}) cannot be sent on {path}")]
    InvalidMessagePath {
        type_name: &'static str,
        type_index: u32,
        path: &'static str,
    },

    #[error("handshake failed: {0}")]
    HandshakeFailed(String),

    #[error("connection closed")]
    ConnectionClosed,

    #[error("carrier error: {0}")]
    Carrier(String),

    #[error("channel error: {0}")]
    Channel(String),
}

pub type Result<T> = std::result::Result<T, GridMateError>;
