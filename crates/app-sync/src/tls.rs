//! TLS client configuration: rustls with the `app-tls` crypto provider (ADR 0003). The trusted roots (ADR 0016) are the
//! Mozilla roots from `webpki-roots`, the OS store on the desktop (`rustls-native-certs`), the certificates the host
//! passes in ([`set_host_roots`]: Android's `AndroidCAStore`), and per connection the account's own trusted
//! certificates ([`crate::trust`]). There is no option to accept invalid certificates (`ARCHITECTURE.md §9`).
//!
//! app-tls implements TLS 1.3 and 1.2 with ChaCha20-Poly1305 and X25519 only. A server that offers none of these
//! fails the handshake; [`incompatible_server_hint`] turns that failure into a message that says why.

use std::error::Error as StdError;
use std::io;
use std::sync::{Arc, OnceLock};

use rustls::pki_types::CertificateDer;
use rustls::{AlertDescription, ClientConfig, RootCertStore};

use crate::error::SyncError;

/// What dail tells the user when a server and app-tls share no algorithms.
pub const INCOMPATIBLE_SERVER: &str = "the server supports none of the TLS ciphers dail implements (TLS 1.2/1.3 with ChaCha20-Poly1305 or AES-GCM, and X25519, P-256 or P-384)";

/// The roots the host passed in, once per process.
static HOST_ROOTS: OnceLock<Vec<CertificateDer<'static>>> = OnceLock::new();

/// Every root besides the Mozilla ones: the OS store's, then the host's. Read once per process.
static EXTRA_ROOTS: OnceLock<Vec<CertificateDer<'static>>> = OnceLock::new();

/// Makes the certificates `hex_ders` (DER, as hex) trusted roots of every connection: Android's `AndroidCAStore`,
/// which holds the user's own CAs. Call it before the first connection; later calls, and certificates that do not
/// decode, are ignored. Returns how many were taken.
pub fn set_host_roots(hex_ders: &[String]) -> usize {
    let certs: Vec<CertificateDer<'static>> = hex_ders
        .iter()
        .filter_map(|hex| decode_hex(hex))
        .map(CertificateDer::from)
        .collect();
    let count = certs.len();
    if HOST_ROOTS.set(certs).is_err() {
        tracing::debug!("host roots already set");
        return 0;
    }
    count
}

/// The OS store's roots on the desktop, then the host's.
fn extra_roots() -> &'static [CertificateDer<'static>] {
    EXTRA_ROOTS.get_or_init(|| {
        let mut roots = native_roots();
        roots.extend(HOST_ROOTS.get().into_iter().flatten().cloned());
        roots
    })
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn native_roots() -> Vec<CertificateDer<'static>> {
    let found = rustls_native_certs::load_native_certs();
    for err in &found.errors {
        tracing::warn!("OS certificate store: {err}");
    }
    tracing::debug!("OS certificate store: {} certificates", found.certs.len());
    found.certs
}

/// The phones have no store `rustls-native-certs` can read: Android's comes from the host, iOS lists none.
#[cfg(any(target_os = "android", target_os = "ios"))]
fn native_roots() -> Vec<CertificateDer<'static>> {
    Vec::new()
}

/// The certificates of `hex_ders` (an account's trusted certificates) that decode.
pub fn decode_certs(hex_ders: &[String]) -> Vec<CertificateDer<'static>> {
    hex_ders
        .iter()
        .filter_map(|hex| decode_hex(hex))
        .map(CertificateDer::from)
        .collect()
}

/// The roots of every connection: Mozilla's, then [`extra_roots`]. A root that rustls cannot parse is skipped.
pub(crate) fn root_store(account: &[CertificateDer<'static>]) -> RootCertStore {
    let mut roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let (_, ignored) =
        roots.add_parsable_certificates(extra_roots().iter().chain(account).cloned());
    if ignored > 0 {
        tracing::debug!("{ignored} roots could not be parsed");
    }
    roots
}

/// The client config used for connections that belong to no account (autoconfig, updates).
pub fn client_config() -> Result<Arc<ClientConfig>, SyncError> {
    client_config_with_roots(root_store(&[]))
}

/// The client config of an account's connections: every root, and the certificates the account trusts.
pub fn account_config(trusted: &[CertificateDer<'static>]) -> Result<Arc<ClientConfig>, SyncError> {
    client_config_with_roots(root_store(trusted))
}

/// A client config trusting exactly `roots`.
pub fn client_config_with_roots(roots: RootCertStore) -> Result<Arc<ClientConfig>, SyncError> {
    let config = ClientConfig::builder_with_provider(Arc::new(app_tls::provider()))
        .with_safe_default_protocol_versions()
        .map_err(|err| SyncError::Tls(err.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Installs `app_tls::provider()` as rustls's process-wide default, once, for code that asks rustls for the default
/// provider. dail's own connections name app-tls explicitly; lettre gets an encrypted stream (`smtp::connect`).
pub fn install_default_provider() {
    // `Err` means a provider is already installed: by an earlier call, which is the same provider.
    if app_tls::provider().install_default().is_err() {
        tracing::trace!("rustls default crypto provider already installed");
    }
}

/// Lower-case hex of `bytes`.
pub fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            // Writing to a String cannot fail.
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// The bytes of hex text (either case, no separators); `None` if it is not hex.
pub fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            text.get(i..i + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect()
}

/// [`INCOMPATIBLE_SERVER`] when `err`, or an error in its source chain, says that the server and app-tls share no
/// protocol version, cipher suite or key exchange group. tokio-rustls reports handshake failures as an `io::Error`
/// that wraps the `rustls::Error`; lettre wraps that `io::Error` again.
pub fn incompatible_server_hint(err: &(dyn StdError + 'static)) -> Option<&'static str> {
    let mut next = Some(err);
    while let Some(err) = next {
        let rustls_err = err.downcast_ref::<rustls::Error>().or_else(|| {
            err.downcast_ref::<io::Error>()
                .and_then(|io_err| io_err.get_ref())
                .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        });
        if rustls_err.is_some_and(is_incompatible) {
            return Some(INCOMPATIBLE_SERVER);
        }
        next = err.source();
    }
    None
}

/// The failures that mean "no algorithm in common", as opposed to a bad certificate or a broken connection.
fn is_incompatible(err: &rustls::Error) -> bool {
    matches!(
        err,
        rustls::Error::AlertReceived(
            AlertDescription::HandshakeFailure
                | AlertDescription::ProtocolVersion
                | AlertDescription::InsufficientSecurity
        ) | rustls::Error::PeerIncompatible(_)
    )
}

/// `message`, followed by the incompatible-server hint when `err` calls for one.
pub fn with_hint(message: String, err: &(dyn StdError + 'static)) -> String {
    match incompatible_server_hint(err) {
        Some(hint) => format!("{message} ({hint})"),
        None => message,
    }
}

#[cfg(test)]
mod tests {
    use rustls::{CertificateError, PeerIncompatible};

    use super::*;

    fn io_wrapped(err: rustls::Error) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }

    #[test]
    fn hints_at_servers_without_common_algorithms() {
        for err in [
            rustls::Error::AlertReceived(AlertDescription::HandshakeFailure),
            rustls::Error::AlertReceived(AlertDescription::ProtocolVersion),
            rustls::Error::AlertReceived(AlertDescription::InsufficientSecurity),
            rustls::Error::PeerIncompatible(PeerIncompatible::NoCipherSuitesInCommon),
            rustls::Error::PeerIncompatible(PeerIncompatible::NoKxGroupsInCommon),
        ] {
            assert_eq!(
                incompatible_server_hint(&err),
                Some(INCOMPATIBLE_SERVER),
                "{err:?}"
            );
            assert_eq!(
                incompatible_server_hint(&io_wrapped(err.clone())),
                Some(INCOMPATIBLE_SERVER),
                "{err:?}"
            );
        }
    }

    #[test]
    fn no_hint_for_other_failures() {
        for err in [
            rustls::Error::InvalidCertificate(CertificateError::UnknownIssuer),
            rustls::Error::AlertReceived(AlertDescription::BadCertificate),
            rustls::Error::DecryptError,
        ] {
            assert_eq!(incompatible_server_hint(&io_wrapped(err)), None);
        }
        let reset = io::Error::new(io::ErrorKind::ConnectionReset, "reset");
        assert_eq!(incompatible_server_hint(&reset), None);
    }

    /// An error whose source is another error, like lettre's and tokio-rustls's wrappers.
    #[derive(Debug)]
    struct Wrapper(io::Error);

    impl std::fmt::Display for Wrapper {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("connection error")
        }
    }

    impl StdError for Wrapper {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn follows_the_source_chain() {
        let err = Wrapper(io_wrapped(rustls::Error::AlertReceived(
            AlertDescription::HandshakeFailure,
        )));
        assert_eq!(
            with_hint(
                "SMTP smtp.example.com:465: connect failed: connection error".into(),
                &err
            ),
            format!(
                "SMTP smtp.example.com:465: connect failed: connection error ({INCOMPATIBLE_SERVER})"
            )
        );
        assert_eq!(
            with_hint("x".into(), &Wrapper(io::Error::other("nope"))),
            "x"
        );
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(encode_hex(&[0, 0xab, 0x10]), "00ab10");
        assert_eq!(decode_hex("00AB10"), Some(vec![0, 0xab, 0x10]));
        assert_eq!(decode_hex("0"), None);
        assert_eq!(decode_hex("zz"), None);
        assert_eq!(decode_hex("é0"), None);
    }
}
