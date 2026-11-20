//! Per-account certificate trust (ADR 0016): offers from the runtime, `trust_certificate` and
//! `forget_certificate`, undo and redo, and agents refused.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, ctx};
use app_core::{Command, CoreError, Effect, Event, Origin, TrustedCert};

const SHA: &str = "3f8a1c0e5b2d4f6a7c9e1b3d5f7a9c0e2b4d6f8a1c3e5b7d9f0a2c4e6b8d0f1a";

fn cert() -> TrustedCert {
    TrustedCert {
        der: "3082".into(),
        sha256: SHA.to_ascii_uppercase(),
        subject: "CN=mail.example.org".into(),
        issuer: "CN=mail.example.org".into(),
        not_before_ms: 1,
        not_after_ms: 2,
    }
}

/// The certificate offered by the Hostinger account's IMAP server.
fn offered(m: &mut Mail) {
    let imap = m.engine.state().accounts[&m.account].imap.clone();
    m.feed(Event::CertificateOffered {
        host: imap.host.to_ascii_uppercase(),
        port: imap.port,
        cert: cert(),
    });
}

fn trust(m: &mut Mail, sha256: &str) -> Result<app_core::Outcome, CoreError> {
    m.run(Command::TrustCertificate {
        account: m.account,
        sha256: sha256.into(),
    })
}

#[test]
fn an_offer_is_trusted_then_undone_and_redone() {
    let mut m = Mail::hostinger();
    offered(&mut m);
    let offers = &m.engine.state().status.cert_offers;
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].account, m.account);
    assert_eq!(offers[0].cert.sha256, SHA, "fingerprints are normalised");

    // Colons and case are ignored.
    let colons: String = SHA
        .as_bytes()
        .chunks(2)
        .map(|p| std::str::from_utf8(p).unwrap().to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(":");
    let outcome = trust(&mut m, &colons).unwrap();
    let account = &m.engine.state().accounts[&m.account];
    assert_eq!(account.trusted_certs.len(), 1);
    assert_eq!(account.imap_login().trusted, ["3082"]);
    assert_eq!(account.smtp_login().trusted, ["3082"]);
    assert!(m.engine.state().status.cert_offers.is_empty());
    assert!(outcome.effects.iter().any(
        |e| matches!(e, Effect::VerifyAccount { trusted, .. } if trusted == &["3082".to_owned()])
    ));
    assert!(
        outcome
            .effects
            .iter()
            .any(|e| matches!(e, Effect::PersistState { .. }))
    );

    m.run(Command::Undo {}).unwrap();
    assert!(
        m.engine.state().accounts[&m.account]
            .trusted_certs
            .is_empty()
    );
    assert_eq!(m.engine.state().status.cert_offers.len(), 1);
    m.run(Command::Redo {}).unwrap();
    assert_eq!(m.engine.state().accounts[&m.account].trusted_certs.len(), 1);

    m.run(Command::ForgetCertificate {
        account: m.account,
        sha256: SHA.into(),
    })
    .unwrap();
    assert!(
        m.engine.state().accounts[&m.account]
            .trusted_certs
            .is_empty()
    );
    // Forgotten, it can be trusted again.
    trust(&mut m, SHA).unwrap();
}

#[test]
fn offers_go_to_the_accounts_on_that_server_only() {
    let mut m = Mail::hostinger();
    m.feed(Event::CertificateOffered {
        host: "other.example.org".into(),
        port: 993,
        cert: cert(),
    });
    assert!(m.engine.state().status.cert_offers.is_empty());
    let imap = m.engine.state().accounts[&m.account].imap.clone();
    m.feed(Event::CertificateOffered {
        host: imap.host,
        port: imap.port + 1,
        cert: cert(),
    });
    assert!(m.engine.state().status.cert_offers.is_empty());
    // A malformed fingerprint from the runtime is dropped.
    let imap = m.engine.state().accounts[&m.account].imap.clone();
    m.feed(Event::CertificateOffered {
        host: imap.host,
        port: imap.port,
        cert: TrustedCert {
            sha256: "nope".into(),
            ..cert()
        },
    });
    assert!(m.engine.state().status.cert_offers.is_empty());
    // The same offer twice is kept once.
    offered(&mut m);
    offered(&mut m);
    assert_eq!(m.engine.state().status.cert_offers.len(), 1);
}

#[test]
fn errors() {
    let mut m = Mail::hostinger();
    assert!(matches!(
        trust(&mut m, "abc"),
        Err(CoreError::InvalidArgument { .. })
    ));
    assert!(matches!(
        trust(&mut m, SHA),
        Err(CoreError::NotFound { .. })
    ));
    offered(&mut m);
    assert!(matches!(
        m.run(Command::TrustCertificate {
            account: app_core::AccountId::from_uuid(uuid::Uuid::from_u128(77)),
            sha256: SHA.into(),
        }),
        Err(CoreError::NotFound { .. })
    ));
    trust(&mut m, SHA).unwrap();
    // Offered again while trusted: no offer, and trusting twice is refused.
    offered(&mut m);
    assert!(m.engine.state().status.cert_offers.is_empty());
    assert!(matches!(
        trust(&mut m, SHA),
        Err(CoreError::InvalidArgument { .. })
    ));
    let other = "0".repeat(64);
    assert!(matches!(
        m.run(Command::ForgetCertificate {
            account: m.account,
            sha256: other,
        }),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn agents_cannot_trust_but_can_forget() {
    let mut m = Mail::hostinger();
    offered(&mut m);
    let agent = app_core::Ctx {
        origin: Origin::Mcp,
        ..ctx(90)
    };
    let err = m
        .engine
        .dispatch(
            Command::TrustCertificate {
                account: m.account,
                sha256: SHA.into(),
            },
            &agent,
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }));
    trust(&mut m, SHA).unwrap();
    m.engine
        .dispatch(
            Command::ForgetCertificate {
                account: m.account,
                sha256: SHA.into(),
            },
            &agent,
        )
        .unwrap();
    assert!(
        m.engine.state().accounts[&m.account]
            .trusted_certs
            .is_empty()
    );
}
