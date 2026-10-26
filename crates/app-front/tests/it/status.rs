//! Problems: what a failure probably means, its hint, and the fix offered next to it.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AccountStatus, ProviderPreset};
use app_front::i18n::Lang;
use app_front::status::{Cause, Fix, cause, hint, problems};

use crate::support::synced_state;

#[test]
fn diagnostics_are_classified() {
    for (error, expected) in [
        (
            "IMAP LOGIN failed: [AUTHENTICATIONFAILED] Invalid credentials (Failure)",
            Cause::Password,
        ),
        (
            "SMTP smtp.gmail.com:465: permanent error (535): 5.7.8 Username and Password not accepted",
            Cause::Password,
        ),
        (
            "SMTP smtp.gmail.com:465: permanent error (534): 5.7.9 Application-specific password required",
            Cause::Password,
        ),
        (
            "no password stored in the keyring for account/x",
            Cause::Password,
        ),
        (
            "IMAP mail.example.org:993: [OVERQUOTA] Quota exceeded",
            Cause::Quota,
        ),
        (
            "SMTP mail.example.org:587: permanent error (552): 5.2.2 Mailbox full",
            Cause::Quota,
        ),
        (
            "IMAP mail.example.org:993: TLS handshake failed: received fatal alert: HandshakeFailure (the server supports none of the TLS ciphers dail implements (TLS 1.2/1.3 with ChaCha20-Poly1305 and X25519))",
            Cause::TlsCiphers,
        ),
        (
            "IMAP mail.example.org:993: TLS handshake failed: invalid peer certificate: UnknownIssuer",
            Cause::Certificate,
        ),
        (
            "IMAP mail.example.org:993: connect failed: Connection refused (os error 111)",
            Cause::Network,
        ),
        ("IMAP mail.example.org:993: timed out", Cause::Network),
        (
            "IMAP mail.example.org:993: connect failed: failed to lookup address information: Name or service not known",
            Cause::Network,
        ),
        (
            "IMAP SELECT failed: [NONEXISTENT] Unknown Mailbox",
            Cause::Other,
        ),
    ] {
        assert_eq!(cause(error), expected, "{error}");
    }
    assert!(
        hint(Cause::Password, ProviderPreset::Gmail, Lang::English)
            .unwrap()
            .contains("App Password")
    );
    assert!(hint(Cause::Other, ProviderPreset::Custom, Lang::English).is_none());
}

#[test]
fn problems_carry_hints_and_fixes() {
    let mut state = synced_state();
    let id = *state.accounts.keys().next().unwrap();
    state.accounts[&id].status = AccountStatus::Failed {
        error: "IMAP LOGIN failed: [AUTHENTICATIONFAILED] Invalid credentials".into(),
        at_ms: 0,
    };
    let [problem] = &problems(&state, Lang::English)[..] else {
        unreachable!("{:?}", problems(&state, Lang::English))
    };
    assert_eq!(problem.fix, Some(Fix::Password(id)));
    assert!(
        problem
            .hint
            .as_deref()
            .unwrap()
            .contains("refused the password")
    );
    // A login test that could not reach the server is tried again, not re-typed.
    state.accounts[&id].status = AccountStatus::Failed {
        error: "IMAP mail.example.org:993: timed out".into(),
        at_ms: 0,
    };
    assert_eq!(problems(&state, Lang::English)[0].fix, Some(Fix::Test(id)));
    assert_eq!(Fix::Test(id).label(Lang::English), "test again");
}

#[test]
fn the_pseudo_locale_reaches_the_status_texts() {
    let mut state = synced_state();
    let id = *state.accounts.keys().next().unwrap();
    state.accounts[&id].status = AccountStatus::Failed {
        error: "IMAP LOGIN failed".into(),
        at_ms: 0,
    };
    let problem = &problems(&state, Lang::Pseudo)[0];
    assert!(
        problem.text.ends_with("lögíñ fáíléd ····]"),
        "{}",
        problem.text
    );
    assert!(problem.text.contains("@example.com"), "the address is kept");
    assert!(problem.hint.as_deref().unwrap().starts_with('['));
    assert!(Fix::Password(id).label(Lang::Pseudo).starts_with("[ré-"));
}

#[test]
fn account_state_reports_failures_then_syncing_then_waiting_changes() {
    use app_front::status::{AccountState, account_state};
    let mut state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    assert_eq!(account_state(&state, account), AccountState::Idle);
    state.status.sync.get_mut(&account).unwrap().pending = 1;
    assert_eq!(account_state(&state, account), AccountState::Syncing);
    state.status.sync.get_mut(&account).unwrap().last_error = Some(app_core::SyncIssue {
        error: "IMAP imap.example.com:993: connection refused\u{7}".into(),
        mailbox: None,
        at_ms: 1,
    });
    let AccountState::Failed(error) = account_state(&state, account) else {
        panic!("not failed")
    };
    assert!(error.starts_with("IMAP imap.example.com:993: connection refused"));
    assert!(!error.contains('\u{7}'), "control characters are cleaned");
    let sync = state.status.sync.get_mut(&account).unwrap();
    sync.last_error = None;
    sync.pending = 0;
    let (_, threads) = crate::support::inbox_of(&state);
    let mut engine = app_core::Engine::new(state);
    let ctx = app_core::Ctx {
        now_ms: crate::support::NOW,
        id_seed: 9 << 64,
        origin: app_core::Origin::Cli,
    };
    let outcome = engine
        .dispatch(
            app_core::Command::Star {
                ids: vec![threads[0]],
                messages: Vec::new(),
            },
            &ctx,
        )
        .unwrap();
    let op = outcome.receipt.affected.iter().find_map(|e| match e {
        app_core::EntityRef::Op(id) => Some(*id),
        _ => None,
    });
    engine
        .ingest(
            app_core::Event::RemoteOpFailed {
                op: op.unwrap(),
                error: "network unreachable".into(),
                permanent: false,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(
        account_state(engine.state(), account),
        AccountState::Offline { waiting: 1 }
    );
}
