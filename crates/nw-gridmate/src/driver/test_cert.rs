//! Self-signed RSA certificate generator for development and tests.
//!
//! Real deployments load a CA-signed cert from disk. Our private server
//! and our tests only ever need a key the client will accept (client
//! verification is `NONE` by default), so an in-memory self-signed
//! cert is the path of least friction.
//!
//! Output is PEM strings ready to hand to
//! [`SecureSocketListener::bind`](crate::driver::SecureSocketListener::bind).

use crate::driver::error::DriverError;
use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::x509::{X509, X509NameBuilder};

/// Self-signed RSA-2048 cert + key, valid 1 day.
///
/// `common_name` becomes the cert's `CN` field and the issuer (it's
/// self-signed). Use something descriptive — `gridmate.dev`,
/// `localhost`, your hostname — so wireshark / openssl-cli inspections
/// stay legible.
///
/// Returns `(cert_pem, key_pem)` ready for
/// [`SecureSocketListener::bind`](crate::driver::SecureSocketListener::bind).
pub fn generate_self_signed_cert(common_name: &str) -> Result<(String, String), DriverError> {
    let rsa = Rsa::generate(2048).map_err(|e| DriverError::Ssl(format!("RSA generate: {e}")))?;
    let pkey = PKey::from_rsa(rsa).map_err(|e| DriverError::Ssl(format!("PKey from rsa: {e}")))?;

    let mut name =
        X509NameBuilder::new().map_err(|e| DriverError::Ssl(format!("X509NameBuilder: {e}")))?;
    name.append_entry_by_text("CN", common_name)
        .map_err(|e| DriverError::Ssl(format!("append CN: {e}")))?;
    let name = name.build();

    let mut builder =
        X509::builder().map_err(|e| DriverError::Ssl(format!("X509 builder: {e}")))?;
    builder
        .set_version(2)
        .map_err(|e| DriverError::Ssl(format!("set_version: {e}")))?;

    let mut serial = BigNum::new().map_err(|e| DriverError::Ssl(format!("BigNum::new: {e}")))?;
    serial
        .rand(159, MsbOption::MAYBE_ZERO, false)
        .map_err(|e| DriverError::Ssl(format!("BigNum::rand: {e}")))?;
    let serial = serial
        .to_asn1_integer()
        .map_err(|e| DriverError::Ssl(format!("to_asn1_integer: {e}")))?;
    builder
        .set_serial_number(&serial)
        .map_err(|e| DriverError::Ssl(format!("set_serial_number: {e}")))?;

    builder
        .set_subject_name(&name)
        .map_err(|e| DriverError::Ssl(format!("set_subject_name: {e}")))?;
    builder
        .set_issuer_name(&name)
        .map_err(|e| DriverError::Ssl(format!("set_issuer_name: {e}")))?;
    builder
        .set_pubkey(&pkey)
        .map_err(|e| DriverError::Ssl(format!("set_pubkey: {e}")))?;

    let now =
        Asn1Time::days_from_now(0).map_err(|e| DriverError::Ssl(format!("Asn1Time now: {e}")))?;
    let later =
        Asn1Time::days_from_now(1).map_err(|e| DriverError::Ssl(format!("Asn1Time +1d: {e}")))?;
    builder
        .set_not_before(&now)
        .map_err(|e| DriverError::Ssl(format!("set_not_before: {e}")))?;
    builder
        .set_not_after(&later)
        .map_err(|e| DriverError::Ssl(format!("set_not_after: {e}")))?;

    builder
        .sign(&pkey, MessageDigest::sha256())
        .map_err(|e| DriverError::Ssl(format!("sign: {e}")))?;
    let cert = builder.build();

    let cert_pem = String::from_utf8(
        cert.to_pem()
            .map_err(|e| DriverError::Ssl(format!("cert to_pem: {e}")))?,
    )
    .map_err(|e| DriverError::Ssl(format!("cert utf8: {e}")))?;
    let key_pem = String::from_utf8(
        pkey.private_key_to_pem_pkcs8()
            .map_err(|e| DriverError::Ssl(format!("key to_pem: {e}")))?,
    )
    .map_err(|e| DriverError::Ssl(format!("key utf8: {e}")))?;

    Ok((cert_pem, key_pem))
}
