//! TLS client configuration: rustls with the `app-tls` crypto provider (ADR 0003) and the Mozilla roots from
//! `webpki-roots`. There is no option to accept invalid certificates (`ARCHITECTURE.md §9`).
//!
//! app-tls implements TLS 1.3 and 1.2 with ChaCha20-Poly1305 and X25519 only. A server that offers none of these
//! fails the handshake; [`incompatible_server_hint`] turns that failure into a message that says why.

use std::error::Error as StdError;
use std::io;
use std::sync::Arc;

use lettre::transport::smtp::client::{CertificateStore, TlsParameters};
use rustls::{AlertDescription, ClientConfig, RootCertStore};

use crate::error::SyncError;

/// What dail tells the user when a server and app-tls share no algorithms.
pub const INCOMPATIBLE_SERVER: &str = "the server supports none of the TLS ciphers dail implements (TLS 1.2/1.3 with ChaCha20-Poly1305 and X25519)";

/// The client config used for IMAP connections.
pub fn client_config() -> Result<Arc<ClientConfig>, SyncError> {
    client_config_with_roots(RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    })
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

/// Installs `app_tls::provider()` as rustls's process-wide default, once. lettre (with `rustls-no-provider`) takes
/// its provider from there, and would panic without one.
pub fn install_default_provider() {
    // `Err` means a provider is already installed: by an earlier call, which is the same provider.
    if app_tls::provider().install_default().is_err() {
        tracing::trace!("rustls default crypto provider already installed");
    }
}

/// lettre's TLS parameters for `host`: app-tls and the webpki roots, like the IMAP config.
pub fn smtp_parameters(host: &str) -> Result<TlsParameters, lettre::transport::smtp::Error> {
    install_default_provider();
    TlsParameters::builder(host.to_owned())
        .certificate_store(CertificateStore::WebpkiRoots)
        .build_rustls()
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
    fn smtp_parameters_install_the_default_provider() {
        smtp_parameters("smtp.example.com").unwrap();
        let installed = rustls::crypto::CryptoProvider::get_default().unwrap();
        let suites: Vec<_> = installed
            .cipher_suites
            .iter()
            .map(|suite| suite.suite())
            .collect();
        let ours: Vec<_> = app_tls::provider()
            .cipher_suites
            .iter()
            .map(|suite| suite.suite())
            .collect();
        assert_eq!(suites, ours);
        // A second call finds the provider installed and still works.
        smtp_parameters("smtp.example.com").unwrap();
    }
}
