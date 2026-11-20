//! Per-account certificate trust (ADR 0016). Every account connection (IMAP and SMTP) verifies
//! through [`TrustVerifier`]:
//!
//! - the usual chain check against [`crate::tls`]'s roots plus the account's trusted
//!   certificates as extra anchors (a private CA);
//! - failing that, a server certificate whose SHA-256 is one the account trusts passes, if it
//!   names the host and is within its dates (a self-signed certificate, which is often marked as
//!   a CA and so cannot be an anchor for itself);
//! - failing that with `UnknownIssuer`, the top of the chain the server sent is kept as an offer,
//!   which the runtime reports as `Event::CertificateOffered` ([`take_offers`]).

use std::sync::{Arc, Mutex};

use app_core::{Event, TrustedCert};
use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::ParsedCertificate;
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::error::SyncError;
use crate::tls;

/// The most offers waiting to be reported.
const MAX_PENDING: usize = 8;

/// Offers waiting for [`take_offers`]: host, port, certificate DER.
static OFFERS: Mutex<Vec<(String, u16, Vec<u8>)>> = Mutex::new(Vec::new());

fn offers() -> std::sync::MutexGuard<'static, Vec<(String, u16, Vec<u8>)>> {
    // A panic while holding the lock leaves a plain Vec behind: still usable.
    OFFERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The offers made since the last call, as events.
pub fn take_offers() -> Vec<Event> {
    std::mem::take(&mut *offers())
        .into_iter()
        .map(|(host, port, der)| Event::CertificateOffered {
            host,
            port,
            cert: describe(&der),
        })
        .collect()
}

/// The SHA-256 of `der`, lower-case hex.
pub fn fingerprint(der: &[u8]) -> String {
    tls::encode_hex(&Sha256::digest(der))
}

/// The client config of a connection to `host:port` for an account that trusts `trusted` (DER
/// as hex).
pub fn account_config(port: u16, trusted: &[String]) -> Result<Arc<ClientConfig>, SyncError> {
    let provider = Arc::new(app_tls::provider());
    let verifier = verifier(port, trusted, &provider)?;
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| SyncError::Tls(err.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

fn verifier(
    port: u16,
    trusted: &[String],
    provider: &Arc<rustls::crypto::CryptoProvider>,
) -> Result<TrustVerifier, SyncError> {
    let anchors = tls::decode_certs(trusted);
    let pinned = anchors.iter().map(|der| fingerprint(der)).collect();
    let inner = WebPkiServerVerifier::builder_with_provider(
        Arc::new(tls::root_store(&anchors)),
        Arc::clone(provider),
    )
    .build()
    .map_err(|err| SyncError::Tls(err.to_string()))?;
    Ok(TrustVerifier {
        inner,
        pinned,
        port,
    })
}

/// See the module doc.
#[derive(Debug)]
struct TrustVerifier {
    inner: Arc<WebPkiServerVerifier>,
    /// SHA-256 of the account's trusted certificates.
    pinned: Vec<String>,
    port: u16,
}

impl TrustVerifier {
    /// Whether `end_entity` is a pinned certificate that names `server_name` and is valid `now`.
    fn pinned_ok(
        &self,
        end_entity: &CertificateDer<'_>,
        server_name: &ServerName<'_>,
        now: UnixTime,
    ) -> bool {
        if !self.pinned.contains(&fingerprint(end_entity)) {
            return false;
        }
        let named = ParsedCertificate::try_from(end_entity)
            .and_then(|cert| rustls::client::verify_server_name(&cert, server_name))
            .is_ok();
        let info = describe(end_entity);
        let now_ms = i64::try_from(now.as_secs())
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        named && info.not_before_ms <= now_ms && now_ms <= info.not_after_ms
    }
}

impl ServerCertVerifier for TrustVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let result = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        );
        let Err(err) = result else {
            return result;
        };
        if self.pinned_ok(end_entity, server_name, now) {
            return Ok(ServerCertVerified::assertion());
        }
        // A private chain fails as `UnknownIssuer`; a self-signed certificate marked as a CA as
        // webpki's `CaUsedAsEndEntity`, which rustls passes on as `Other`. Either is an offer; a
        // wrong name or date is not.
        if matches!(
            err,
            rustls::Error::InvalidCertificate(
                CertificateError::UnknownIssuer | CertificateError::Other(_)
            )
        ) {
            let top = intermediates.last().unwrap_or(end_entity);
            let host = match server_name {
                ServerName::DnsName(name) => name.as_ref().to_owned(),
                other => other.to_str().into_owned(),
            };
            let mut pending = offers();
            if pending.len() < MAX_PENDING {
                pending.push((host, self.port, top.to_vec()));
            }
        }
        Err(err)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// What the user sees of a certificate: its fingerprint, names and dates, read from the DER.
/// Fields that do not parse are left empty (names) or zero (dates).
pub fn describe(der: &[u8]) -> TrustedCert {
    let mut cert = TrustedCert {
        der: tls::encode_hex(der),
        sha256: fingerprint(der),
        subject: String::new(),
        issuer: String::new(),
        not_before_ms: 0,
        not_after_ms: 0,
    };
    if let Some(tbs) = tbs_fields(der) {
        cert.issuer = tbs.issuer;
        cert.subject = tbs.subject;
        cert.not_before_ms = tbs.not_before_ms;
        cert.not_after_ms = tbs.not_after_ms;
    }
    cert
}

struct Tbs {
    issuer: String,
    subject: String,
    not_before_ms: i64,
    not_after_ms: i64,
}

/// One DER element: its tag and contents, and what follows it.
fn element(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 4 || rest.len() < count {
            return None;
        }
        let (bytes, rest) = rest.split_at(count);
        (
            bytes.iter().fold(0usize, |n, b| (n << 8) | usize::from(*b)),
            rest,
        )
    };
    if rest.len() < len {
        return None;
    }
    let (body, rest) = rest.split_at(len);
    Some((tag, body, rest))
}

fn tbs_fields(der: &[u8]) -> Option<Tbs> {
    let (_, cert, _) = element(der)?;
    let (_, tbs, _) = element(cert)?;
    let (tag, _, mut rest) = element(tbs)?;
    // [0] version comes first if present; then the serial number.
    if tag == 0xa0 {
        rest = element(rest)?.2;
    }
    let (_, _, rest) = element(rest)?; // signature algorithm
    let (_, issuer, rest) = element(rest)?;
    let (_, validity, rest) = element(rest)?;
    let (_, subject, _) = element(rest)?;
    let (t1, before, after) = element(validity)?;
    let (t2, after, _) = element(after)?;
    Some(Tbs {
        issuer: name(issuer),
        subject: name(subject),
        not_before_ms: time(t1, before).unwrap_or(0),
        not_after_ms: time(t2, after).unwrap_or(0),
    })
}

/// A distinguished name as `CN=…, O=…`, in the certificate's order.
fn name(mut rdns: &[u8]) -> String {
    let mut parts = Vec::new();
    while let Some((_, set, rest)) = element(rdns) {
        rdns = rest;
        let mut set = set;
        while let Some((_, pair, rest)) = element(set) {
            set = rest;
            let Some((_, oid, value)) = element(pair) else {
                continue;
            };
            let Some((tag, value, _)) = element(value) else {
                continue;
            };
            let label = match oid {
                [0x55, 0x04, 0x03] => "CN",
                [0x55, 0x04, 0x0a] => "O",
                [0x55, 0x04, 0x0b] => "OU",
                [0x55, 0x04, 0x06] => "C",
                [0x55, 0x04, 0x07] => "L",
                [0x55, 0x04, 0x08] => "ST",
                [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x01] => "E",
                _ => continue,
            };
            let text = if tag == 0x1e {
                let units: Vec<u16> = value
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| u16::from_be_bytes(*p))
                    .collect();
                String::from_utf16_lossy(&units)
            } else {
                String::from_utf8_lossy(value).into_owned()
            };
            parts.push(format!("{label}={text}"));
        }
    }
    parts.join(", ")
}

/// A UTCTime (`0x17`, `YYMMDDHHMMSSZ`) or GeneralizedTime (`0x18`, `YYYYMMDDHHMMSSZ`) in Unix
/// milliseconds.
fn time(tag: u8, text: &[u8]) -> Option<i64> {
    let text = std::str::from_utf8(text).ok()?.strip_suffix('Z')?;
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, rest) = match tag {
        0x17 if text.len() == 12 => {
            let yy = num(0..2)?;
            (if yy < 50 { 2000 + yy } else { 1900 + yy }, 2)
        }
        0x18 if text.len() == 14 => (num(0..4)?, 4),
        _ => return None,
    };
    let field = |i: usize| num(rest + 2 * i..rest + 2 * i + 2);
    let (month, day, hour, min, sec) = (field(0)?, field(1)?, field(2)?, field(3)?, field(4)?);
    // Days from 1970-01-01 to the date (Howard Hinnant's days_from_civil).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((((days * 24 + hour) * 60 + min) * 60 + sec) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_time_forms() {
        assert_eq!(time(0x17, b"700101000000Z"), Some(0));
        assert_eq!(time(0x17, b"240229123000Z"), Some(1_709_209_800_000));
        assert_eq!(time(0x18, b"20500101000000Z"), Some(2_524_608_000_000));
        assert_eq!(time(0x17, b"7001010000Z"), None);
        assert_eq!(time(0x18, b"20500101000000"), None);
    }

    const SELF_SIGNED: &[u8] = include_bytes!("../testdata/self-signed.der");
    const SHA: &str = "d2fa14ce02868cb743994bffdf0649eaf50a4774b6fad5ea0d3008e55112eba8";

    fn at(ms: i64) -> UnixTime {
        UnixTime::since_unix_epoch(std::time::Duration::from_millis(u64::try_from(ms).unwrap()))
    }

    fn check(port: u16, trusted: &[String], host: &str, now: UnixTime) -> bool {
        let provider = Arc::new(app_tls::provider());
        let verifier = verifier(port, trusted, &provider).unwrap();
        let name = ServerName::try_from(host.to_owned()).unwrap();
        verifier
            .verify_server_cert(&CertificateDer::from(SELF_SIGNED), &[], &name, &[], now)
            .is_ok()
    }

    #[test]
    fn describes_a_certificate() {
        let cert = describe(SELF_SIGNED);
        assert_eq!(cert.sha256, SHA);
        assert_eq!(cert.subject, "CN=mail.example.org, O=Example Self-Host");
        assert_eq!(cert.issuer, cert.subject);
        assert!(cert.not_before_ms > 1_700_000_000_000);
        assert!(cert.not_after_ms > cert.not_before_ms + 99 * 365 * 86_400_000);
        assert_eq!(decode_hex_der(&cert.der), SELF_SIGNED);
    }

    fn decode_hex_der(hex: &str) -> Vec<u8> {
        tls::decode_hex(hex).unwrap()
    }

    #[test]
    fn an_unknown_self_signed_certificate_is_refused_and_offered() {
        let valid = describe(SELF_SIGNED).not_before_ms + 86_400_000;
        assert!(!check(10_993, &[], "mail.example.org", at(valid)));
        let offered: Vec<_> = take_offers()
            .into_iter()
            .filter_map(|event| match event {
                Event::CertificateOffered {
                    host,
                    port: 10_993,
                    cert,
                } => Some((host, cert)),
                _ => None,
            })
            .collect();
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].0, "mail.example.org");
        assert_eq!(offered[0].1.sha256, SHA);
    }

    #[test]
    fn a_trusted_certificate_passes_only_for_its_name_and_dates() {
        let cert = describe(SELF_SIGNED);
        let trusted = [cert.der.clone()];
        let valid = at(cert.not_before_ms + 86_400_000);
        assert!(check(20_993, &trusted, "mail.example.org", valid));
        assert!(!check(20_994, &trusted, "evil.example.org", valid));
        assert!(!check(
            20_995,
            &trusted,
            "mail.example.org",
            at(cert.not_after_ms + 1000)
        ));
        assert!(!check(
            20_996,
            &trusted,
            "mail.example.org",
            at(cert.not_before_ms - 1000)
        ));
        // Another certificate's trust does not help.
        let other = [tls::encode_hex(&[0x30, 0x00])];
        assert!(!check(20_997, &other, "mail.example.org", valid));
    }

    #[test]
    fn junk_describes_as_empty() {
        let cert = describe(&[0x30, 0x03, 0x02, 0x01]);
        assert_eq!(cert.subject, "");
        assert_eq!(cert.not_after_ms, 0);
        assert_eq!(cert.sha256.len(), 64);
    }
}
