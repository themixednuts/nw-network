//! Carrier configuration for the GridMate-compatible transport.

/// Carrier wire profile selected by the embedding project or runtime.
///
/// GridMate owns the carrier protocol machinery. Projects own the compatibility
/// choices for a specific peer family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierProtocolProfile {
    /// Handshake protocol version.
    pub version: u32,
    /// Whether post-handshake user datagrams should attempt LZ4 compression.
    pub send_compressed: bool,
}

impl CarrierProtocolProfile {
    /// Construct an explicit carrier protocol profile.
    #[must_use]
    pub const fn new(version: u32, send_compressed: bool) -> Self {
        Self {
            version,
            send_compressed,
        }
    }

    /// Vanilla Lumberyard/GridMate carrier profile.
    #[must_use]
    pub const fn lumberyard_default() -> Self {
        Self::new(1, false)
    }
}

impl Default for CarrierProtocolProfile {
    fn default() -> Self {
        Self::lumberyard_default()
    }
}

/// Carrier configuration descriptor (GridMate: CarrierDesc)
/// Rust idiom: Builder pattern with defaults via Default trait
#[derive(Clone)]
pub struct CarrierDesc {
    /// Server address to connect to
    pub server_address: String,

    /// SSL keylog path (for Wireshark)
    pub ssl_keylog_path: Option<std::path::PathBuf>,

    /// CA certificate (PEM string or path)
    pub ca_cert: Option<String>,

    /// Connection timeout in milliseconds (GridMate: m_connectionTimeoutMS = 5000)
    pub connection_timeout_ms: u32,

    /// Thread update interval in milliseconds (GridMate: m_threadUpdateTimeMS = 30)
    pub thread_update_time_ms: u32,

    /// Connection retry base interval (GridMate: m_connectionRetryIntervalBase = 10)
    pub connection_retry_interval_base: u64,

    /// Connection retry max interval (GridMate: m_connectionRetryIntervalMax = 1000)
    pub connection_retry_interval_max: u64,

    /// Clock sync interval in milliseconds (GridMate: disabled by default)
    pub clock_sync_interval: u32,

    /// Protocol version (GridMate: m_version = 1).
    pub version: u32,

    /// Outbound LZ4 compression gate — mirrors
    /// `Carrier::SendCompressed(bool)`. When `true`, carrier datagrams
    /// carrying user frames after connection establishment attempt LZ4
    /// compression and emit a `0x81` header on success. When `false`,
    /// outbound datagrams emit `0x80` and skip the compression attempt.
    pub send_compressed: bool,
}

impl Default for CarrierDesc {
    fn default() -> Self {
        let protocol = CarrierProtocolProfile::default();
        Self {
            server_address: String::new(),
            ssl_keylog_path: None,
            ca_cert: None,
            connection_timeout_ms: 5000,
            thread_update_time_ms: 10,
            connection_retry_interval_base: 10,
            connection_retry_interval_max: 1000,
            clock_sync_interval: 0,
            version: protocol.version,
            send_compressed: protocol.send_compressed,
        }
    }
}

impl CarrierDesc {
    /// Create new descriptor with server address
    pub fn new(server_address: impl Into<String>) -> Self {
        Self {
            server_address: server_address.into(),
            ..Default::default()
        }
    }

    /// Builder pattern: Set SSL keylog path
    pub fn with_ssl_keylog(&mut self, path: impl Into<std::path::PathBuf>) -> &mut Self {
        self.ssl_keylog_path = Some(path.into());
        self
    }

    /// Builder pattern: Set CA certificate
    pub fn with_ca_cert(&mut self, cert: impl Into<String>) -> &mut Self {
        self.ca_cert = Some(cert.into());
        self
    }

    /// Builder pattern: Set connection timeout
    pub fn with_timeout(&mut self, timeout_ms: u32) -> &mut Self {
        self.connection_timeout_ms = timeout_ms;
        self
    }

    /// Builder pattern: select the carrier wire profile.
    pub fn with_protocol_profile(&mut self, protocol: CarrierProtocolProfile) -> &mut Self {
        self.version = protocol.version;
        self.send_compressed = protocol.send_compressed;
        self
    }

    /// Return the selected carrier wire profile.
    #[must_use]
    pub const fn protocol_profile(&self) -> CarrierProtocolProfile {
        CarrierProtocolProfile::new(self.version, self.send_compressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_carrier_profile_is_vanilla_gridmate_not_project_compatibility() {
        let desc = CarrierDesc::default();

        assert_eq!(desc.version, 1);
        assert!(!desc.send_compressed);
        assert_eq!(
            desc.protocol_profile(),
            CarrierProtocolProfile::lumberyard_default()
        );
    }

    #[test]
    fn carrier_profile_can_be_selected_by_project_code() {
        let mut desc = CarrierDesc::new("127.0.0.1:33435");
        desc.with_protocol_profile(CarrierProtocolProfile::new(5, true));

        assert_eq!(desc.server_address, "127.0.0.1:33435");
        assert_eq!(desc.version, 5);
        assert!(desc.send_compressed);
    }
}
