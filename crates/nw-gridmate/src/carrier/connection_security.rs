// ConnectionSecurity namespace from GridMate SecureSocketDriver.cpp
// DTLS record parsing and helper functions

// OpenSSL constants - these match OpenSSL header values but are defined here since
// openssl-sys doesn't expose these particular constants (they're #define macros)
// Values from openssl/ssl3.h and openssl/dtls1.h

/// DTLS Record Types (from GridMate SecureSocketDriver.cpp)
/// These match OpenSSL's SSL3_RT_* constants from openssl/ssl3.h
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    ChangeCipherSpec = 20, // SSL3_RT_CHANGE_CIPHER_SPEC
    Alert = 21,            // SSL3_RT_ALERT
    Handshake = 22,        // SSL3_RT_HANDSHAKE
    ApplicationData = 23,  // SSL3_RT_APPLICATION_DATA
    Heartbeat = 24,        // DTLS1_RT_HEARTBEAT
}

/// DTLS Handshake Types (from GridMate)
/// These match OpenSSL's SSL3_MT_* constants from openssl/ssl3.h
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeType {
    HelloRequest = 0,        // SSL3_MT_HELLO_REQUEST
    ClientHello = 1,         // SSL3_MT_CLIENT_HELLO
    ServerHello = 2,         // SSL3_MT_SERVER_HELLO
    HelloVerifyRequest = 3,  // DTLS1_MT_HELLO_VERIFY_REQUEST
    Certificate = 11,        // SSL3_MT_CERTIFICATE
    ServerKeyExchange = 12,  // SSL3_MT_SERVER_KEY_EXCHANGE
    CertificateRequest = 13, // SSL3_MT_CERTIFICATE_REQUEST
    ServerDone = 14,         // SSL3_MT_SERVER_DONE
    CertificateVerify = 15,  // SSL3_MT_CERTIFICATE_VERIFY
    ClientKeyExchange = 16,  // SSL3_MT_CLIENT_KEY_EXCHANGE
    Finished = 20,           // SSL3_MT_FINISHED
}

// DTLS constants from openssl/dtls1.h
const DTLS1_RT_HEADER_LENGTH: usize = 13;
const DTLS1_HM_HEADER_LENGTH: usize = 12;

/// DTLS Record Header (13 bytes - DTLS1_RT_HEADER_LENGTH)
/// Following GridMate's RecordHeader structure exactly
#[derive(Debug, Clone)]
pub struct RecordHeader {
    pub record_type: u8,      // offset [0], size 1
    pub version: u16,         // offset [1], size 2
    pub epoch: u16,           // offset [3], size 2
    pub sequence_number: u64, // offset [5], size 6 (48-bit)
    pub length: u16,          // offset [11], size 2
}

impl RecordHeader {
    /// Parse DTLS record header from bytes (big-endian/network order)
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < DTLS1_RT_HEADER_LENGTH {
            return None;
        }

        let sequence_number =
            u64::from_be_bytes([0, 0, data[5], data[6], data[7], data[8], data[9], data[10]]);

        Some(Self {
            record_type: data[0],
            version: u16::from_be_bytes([data[1], data[2]]),
            epoch: u16::from_be_bytes([data[3], data[4]]),
            sequence_number,
            length: u16::from_be_bytes([data[11], data[12]]),
        })
    }

    /// Check if this is a handshake record
    pub fn is_handshake(&self) -> bool {
        self.record_type == RecordType::Handshake as u8
    }

    /// Check if this is application data
    pub fn is_application_data(&self) -> bool {
        self.record_type == RecordType::ApplicationData as u8
    }
}

/// Handshake Header (12 bytes - DTLS1_HM_HEADER_LENGTH)
/// Following GridMate's HandshakeHeader structure
#[derive(Debug, Clone)]
pub struct HandshakeHeader {
    pub hs_type: u8,             // offset [13], size 1
    pub hs_length: u32,          // offset [14], size 3 (24-bit)
    pub hs_sequence: u16,        // offset [17], size 2
    pub hs_fragment_offset: u32, // offset [19], size 3 (24-bit)
    pub hs_fragment_length: u32, // offset [22], size 3 (24-bit)
}

impl HandshakeHeader {
    /// Parse handshake header (expects data after record header)
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < DTLS1_HM_HEADER_LENGTH {
            return None;
        }

        // Read 3-byte (24-bit) values as big-endian
        let hs_length = u32::from_be_bytes([0, data[1], data[2], data[3]]);
        let hs_fragment_offset = u32::from_be_bytes([0, data[6], data[7], data[8]]);
        let hs_fragment_length = u32::from_be_bytes([0, data[9], data[10], data[11]]);

        Some(Self {
            hs_type: data[0],
            hs_length,
            hs_sequence: u16::from_be_bytes([data[4], data[5]]),
            hs_fragment_offset,
            hs_fragment_length,
        })
    }
}

/// Check if packet is a handshake (GridMate: IsHandshake)
pub fn is_handshake(data: &[u8]) -> bool {
    if let Some(header) = RecordHeader::parse(data) {
        header.is_handshake()
    } else {
        false
    }
}

/// Check if packet is a ClientHello (GridMate: IsClientHello)
pub fn is_client_hello(data: &[u8]) -> bool {
    if data.len() < 14 || !is_handshake(data) {
        return false;
    }
    data[13] == HandshakeType::ClientHello as u8
}

/// Extract Random from ClientHello (for debugging/logging only)
/// Returns the 32-byte random from the ClientHello
///
/// Note: OpenSSL automatically handles cookie generation using this random.
/// This function is only for inspection/debugging purposes.
pub fn extract_client_hello_random(data: &[u8]) -> Option<Vec<u8>> {
    // RecordHeader = DTLS1_RT_HEADER_LENGTH bytes, HandshakeHeader = DTLS1_HM_HEADER_LENGTH bytes
    // After HandshakeHeader: version(2) + random(32) = 25 bytes
    // So offset is DTLS1_RT_HEADER_LENGTH + DTLS1_HM_HEADER_LENGTH + 2
    let record_hdr_len = DTLS1_RT_HEADER_LENGTH;
    let handshake_hdr_len = DTLS1_HM_HEADER_LENGTH;
    let version_len = 2;
    let random_offset = record_hdr_len + handshake_hdr_len + version_len;

    if data.len() < random_offset + 32 {
        return None;
    }

    Some(data[random_offset..random_offset + 32].to_vec())
}

/// Check if packet is a HelloVerifyRequest (GridMate: IsHelloVerifyRequest)
pub fn is_hello_verify_request(data: &[u8]) -> bool {
    if data.len() < 14 || !is_handshake(data) {
        return false;
    }
    data[13] == HandshakeType::HelloVerifyRequest as u8
}

/// Extract cookie from HelloVerifyRequest packet (for debugging/logging only)
/// Returns the cookie bytes if valid HVR packet
///
/// Note: OpenSSL automatically extracts and uses the cookie from HelloVerifyRequest.
/// This function is only for inspection/debugging purposes.
pub fn extract_cookie_from_hvr(data: &[u8]) -> Option<Vec<u8>> {
    if !is_hello_verify_request(data) {
        return None;
    }

    // HelloVerifyRequest format:
    // Record (13) + Handshake (12) + ServerVersion (2) + CookieLength (1) + Cookie (variable)
    const RECORD_LEN: usize = DTLS1_RT_HEADER_LENGTH;
    const HANDSHAKE_LEN: usize = DTLS1_HM_HEADER_LENGTH;
    const SERVER_VERSION_LEN: usize = 2;
    const OFFSET: usize = RECORD_LEN + HANDSHAKE_LEN + SERVER_VERSION_LEN;

    if data.len() < OFFSET + 1 {
        return None;
    }

    let cookie_len = data[OFFSET] as usize;
    if data.len() < OFFSET + 1 + cookie_len {
        return None;
    }

    Some(data[OFFSET + 1..OFFSET + 1 + cookie_len].to_vec())
}

/// Check if packet is a HelloRequest (GridMate: IsHelloRequestHandshake)
pub fn is_hello_request_handshake(data: &[u8]) -> bool {
    data.len() == 25
        && data[0] == RecordType::Handshake as u8
        && data[13] == HandshakeType::HelloRequest as u8
        && data[14] == 0
        && data[15] == 0
        && data[16] == 0
}

/// Get human-readable packet type string (GridMate: TypeToString)
pub fn type_to_string(data: &[u8]) -> &'static str {
    if data.is_empty() {
        return "Empty";
    }

    match data[0] {
        x if x == RecordType::ChangeCipherSpec as u8 => "ChangeCipherSpec",
        x if x == RecordType::Alert as u8 => "Alert",
        x if x == RecordType::ApplicationData as u8 => "ApplicationData",
        x if x == RecordType::Heartbeat as u8 => "Heartbeat",
        x if x == RecordType::Handshake as u8 && data.len() >= 14 => match data[13] {
            x if x == HandshakeType::HelloRequest as u8 => "HelloRequest",
            x if x == HandshakeType::ClientHello as u8 => "ClientHello",
            x if x == HandshakeType::ServerHello as u8 => "ServerHello",
            x if x == HandshakeType::HelloVerifyRequest as u8 => "HelloVerifyRequest",
            x if x == HandshakeType::Certificate as u8 => "Certificate",
            x if x == HandshakeType::ServerKeyExchange as u8 => "ServerKeyExchange",
            x if x == HandshakeType::CertificateRequest as u8 => "CertificateRequest",
            x if x == HandshakeType::ServerDone as u8 => "ServerDone",
            x if x == HandshakeType::CertificateVerify as u8 => "CertificateVerify",
            x if x == HandshakeType::ClientKeyExchange as u8 => "ClientKeyExchange",
            x if x == HandshakeType::Finished as u8 => "Finished",
            _ => "UnknownHandshake",
        },
        _ => "Unknown",
    }
}

// Note: OpenSSL handles all DTLS handshake packet generation automatically:
// - ClientHello is generated by SSL_connect()
// - When HelloVerifyRequest is received and fed into the read BIO, OpenSSL processes it
// - The next SSL_connect() call automatically sends ClientHello with cookie
// No manual packet construction is needed - OpenSSL manages all buffers internally.

/// Print detailed ClientHello info like Wireshark
pub fn print_client_hello_details(data: &[u8]) -> String {
    let mut details = vec![];

    if let Some(record_hdr) = RecordHeader::parse(data) {
        details.push(format!(
            "Record: type={}, version=0x{:04x}, epoch={}, seq={}, length={}",
            record_hdr.record_type,
            record_hdr.version,
            record_hdr.epoch,
            record_hdr.sequence_number,
            record_hdr.length
        ));

        if data.len() >= 25 {
            if let Some(hs_hdr) = HandshakeHeader::parse(&data[13..]) {
                details.push(format!(
                    "Handshake: type={}, length={}, seq={}, frag_off={}, frag_len={}",
                    hs_hdr.hs_type,
                    hs_hdr.hs_length,
                    hs_hdr.hs_sequence,
                    hs_hdr.hs_fragment_offset,
                    hs_hdr.hs_fragment_length
                ));
            }

            // Version + Random (34 bytes)
            let version = u16::from_be_bytes([data[25], data[26]]);
            let random = &data[27..59];
            details.push(format!("Version: 0x{:04x}", version));
            details.push(format!("Random (hex): {}", hex::encode(random)));

            // Session ID
            if data.len() >= 60 {
                let session_len = data[59];
                details.push(format!("Session ID Length: {}", session_len));
            }

            // Cookie
            if data.len() >= 61 {
                let cookie_len = data[60];
                details.push(format!("Cookie Length: {}", cookie_len));
                if cookie_len > 0 && data.len() >= 61 + cookie_len as usize {
                    let cookie = &data[61..61 + cookie_len as usize];
                    details.push(format!("Cookie (hex): {}", hex::encode(cookie)));
                }
            }
        }
    }

    details.join(" | ")
}
