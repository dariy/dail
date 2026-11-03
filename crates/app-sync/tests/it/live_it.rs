//! Live integration tests against real providers. Ignored by default; run with
//!
//! ```text
//! DAIL_IT_HOSTINGER_EMAIL=… DAIL_IT_HOSTINGER_PASSWORD=… \
//! DAIL_IT_GMAIL_EMAIL=… DAIL_IT_GMAIL_PASSWORD=… \
//!   cargo test -p app-sync --test it live_it -- --ignored
//! ```
//!
//! `DAIL_IT_KEYRING=1` additionally exercises the real OS keyring. Credentials are read from the
//! environment only and never printed.
//!
//! `DAIL_IT_TLS=1` checks, without credentials, that app-tls (ADR 0003) completes handshakes with the reference
//! providers: `cargo test -p app-sync --test it live_it::tls -- --ignored`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::io::Write as _;
use std::time::Duration;

use app_core::{AccountId, ProviderPreset, Secret, SecretRef};
use app_sync::{AccountVerifier, KeyringSecrets, NetVerifier, SecretStore, tls};
use lettre::transport::smtp::client::AsyncSmtpConnection;
use lettre::transport::smtp::extension::ClientId;
use rustls::pki_types::ServerName;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// `(email, password)` from `DAIL_IT_<PREFIX>_EMAIL` / `_PASSWORD`. Fails with the variable names
/// (not their values) if they are missing: an explicitly requested live test must not pass
/// vacuously.
fn credentials(prefix: &str) -> (String, Secret) {
    let var = |name: String| {
        let value = std::env::var(&name).unwrap_or_default();
        assert!(!value.is_empty(), "set {name} to run this live test");
        value
    };
    let email = var(format!("DAIL_IT_{prefix}_EMAIL"));
    let password = Secret::new(var(format!("DAIL_IT_{prefix}_PASSWORD")));
    (email, password)
}

async fn verify_preset(preset: ProviderPreset, prefix: &str) {
    let (email, password) = credentials(prefix);
    let profile = preset.profile();
    let imap = profile.imap.unwrap().to_endpoint();
    let smtp = profile.smtp.unwrap().to_endpoint();
    let verifier = NetVerifier::new().unwrap();

    let result = verifier.verify(&email, &password, &imap, &smtp).await;
    assert!(
        result.is_ok(),
        "{} login failed: {result:?}",
        preset.as_str()
    );

    // A wrong password must be detected by both protocols.
    let wrong = Secret::new(format!("{}-wrong", password.expose()));
    let err = verifier
        .verify(&email, &wrong, &imap, &smtp)
        .await
        .unwrap_err();
    assert!(err.contains("IMAP LOGIN failed"), "{err}");
    assert!(err.contains("SMTP AUTH failed"), "{err}");
    assert!(!err.contains(password.expose()), "error leaks the password");
}

#[tokio::test]
#[ignore = "live: needs DAIL_IT_HOSTINGER_* credentials and network"]
async fn hostinger_login_verifies() {
    verify_preset(ProviderPreset::Hostinger, "HOSTINGER").await;
}

#[tokio::test]
#[ignore = "live: needs DAIL_IT_GMAIL_* credentials (App Password) and network"]
async fn gmail_login_verifies() {
    verify_preset(ProviderPreset::Gmail, "GMAIL").await;
}

#[test]
#[ignore = "live: needs DAIL_IT_KEYRING=1 and an unlocked OS keyring"]
fn os_keyring_round_trip() {
    assert_eq!(
        std::env::var("DAIL_IT_KEYRING").as_deref(),
        Ok("1"),
        "set DAIL_IT_KEYRING=1 to run this live test"
    );
    let id = AccountId::from_uuid(uuid::Uuid::new_v4());
    let secret = SecretRef {
        service: "dail-it".to_owned(),
        ..SecretRef::for_account(id)
    };
    let keyring = KeyringSecrets;
    keyring.set(&secret, &Secret::new("it-value")).unwrap();
    let read = keyring.get(&secret).unwrap().unwrap();
    assert!(
        read.expose() == "it-value",
        "keyring returned a different value"
    );
    keyring.delete(&secret).unwrap();
    assert!(keyring.get(&secret).unwrap().is_none());
    keyring.delete(&secret).unwrap();
}

/// Connects with TLS to `host:port` and returns the first bytes the server sends, after checking that the
/// connection runs TLS 1.3. The server picks the suite and group among app-tls's (ADR 0014).
async fn tls_greeting(host: &str, port: u16) -> String {
    tls_handshake(host, port).await.0
}

/// [`tls_greeting`], with what was negotiated: `(greeting, "TLSv1_3 TLS13_AES_256_GCM_SHA384 X25519")`.
async fn tls_handshake(host: &str, port: u16) -> (String, String) {
    let connector = TlsConnector::from(tls::client_config().unwrap());
    let tcp = TcpStream::connect((host, port)).await.unwrap();
    let name = ServerName::try_from(host.to_owned()).unwrap();
    let mut stream = connector
        .connect(name, tcp)
        .await
        .map_err(|err| tls::with_hint(format!("{host}:{port}: {err}"), &err))
        .unwrap();
    let conn = stream.get_ref().1;
    let negotiated = format!(
        "{:?} {:?} {:?}",
        conn.protocol_version().unwrap(),
        conn.negotiated_cipher_suite().unwrap().suite(),
        conn.negotiated_key_exchange_group().unwrap().name()
    );
    let mut greeting = vec![0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(20), stream.read(&mut greeting))
        .await
        .unwrap()
        .unwrap();
    (
        String::from_utf8_lossy(&greeting[..n]).into_owned(),
        negotiated,
    )
}

/// STARTTLS on submission port 587 through lettre (the default provider app-tls installs), then QUIT, no AUTH.
async fn smtp_starttls(host: &str) {
    let hello = ClientId::default();
    let timeout = Some(Duration::from_secs(20));
    let mut conn = AsyncSmtpConnection::connect_tokio1((host, 587), timeout, &hello, None, None)
        .await
        .unwrap();
    conn.starttls(tls::smtp_parameters(host).unwrap(), &hello)
        .await
        .map_err(|err| tls::with_hint(format!("{host}:587: {err}"), &err))
        .unwrap();
    assert!(conn.is_encrypted(), "{host}:587");
    conn.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "live: needs DAIL_IT_TLS=1 and network (no credentials)"]
async fn tls_reaches_reference_providers() {
    assert_eq!(
        std::env::var("DAIL_IT_TLS").as_deref(),
        Ok("1"),
        "set DAIL_IT_TLS=1 to run this live test"
    );
    for host in ["imap.gmail.com", "imap.hostinger.com"] {
        let (greeting, negotiated) = tls_handshake(host, 993).await;
        assert!(negotiated.starts_with("TLSv1_3"), "{host}: {negotiated}");
        assert!(
            greeting.starts_with("* OK"),
            "{host}:993 greeted {greeting:?}"
        );
    }
    for host in ["smtp.gmail.com", "smtp.hostinger.com"] {
        let greeting = tls_greeting(host, 465).await;
        assert!(
            greeting.starts_with("220"),
            "{host}:465 greeted {greeting:?}"
        );
        smtp_starttls(host).await;
    }
}

/// The provider list of ARCHITECTURE §5.3, probed through app-tls (STORE-11): each IMAP server's TLS handshake and
/// greeting, no login. Prints what each negotiated; fails if any does not connect.
#[tokio::test]
#[ignore = "live: needs DAIL_IT_TLS=1 and network (no credentials)"]
async fn tls_reaches_the_provider_list() {
    assert_eq!(
        std::env::var("DAIL_IT_TLS").as_deref(),
        Ok("1"),
        "set DAIL_IT_TLS=1 to run this live test"
    );
    let mut failed = Vec::new();
    for host in PROVIDERS {
        let probe = tokio::time::timeout(Duration::from_secs(30), async {
            let connector = TlsConnector::from(tls::client_config().unwrap());
            let tcp = TcpStream::connect((*host, 993))
                .await
                .map_err(|e| e.to_string())?;
            let name = ServerName::try_from(host.to_string()).unwrap();
            let stream = connector
                .connect(name, tcp)
                .await
                .map_err(|e| e.to_string())?;
            let conn = stream.get_ref().1;
            Ok::<_, String>(format!(
                "{:?} {:?} {:?}",
                conn.protocol_version().unwrap(),
                conn.negotiated_cipher_suite().unwrap().suite(),
                conn.negotiated_key_exchange_group().unwrap().name()
            ))
        })
        .await
        .unwrap_or_else(|_| Err("timed out".to_owned()));
        match probe {
            Ok(negotiated) => {
                let _ = writeln!(std::io::stderr(), "ok    {host}: {negotiated}");
            }
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "FAIL  {host}: {err}");
                failed.push(*host);
            }
        }
    }
    assert!(failed.is_empty(), "no handshake with {failed:?}");
}

/// The IMAP servers of ARCHITECTURE §5.3's table.
const PROVIDERS: &[&str] = &[
    "imap.gmail.com",
    "imap.hostinger.com",
    "imap.mail.yahoo.com",
    "imap.mail.me.com",
    "imap.fastmail.com",
    "imap.zoho.com",
    "imap.gmx.net",
    "imap.aol.com",
    "imap.yandex.com",
    "imap.ionos.com",
    "imap.one.com",
    "imap.migadu.com",
    "imap.web.de",
    "imap.mailbox.org",
    "imap.purelymail.com",
    "imap.mail.ru",
    "mail.privateemail.com",
    "outlook.office365.com",
    "secureimap.t-online.de",
];
