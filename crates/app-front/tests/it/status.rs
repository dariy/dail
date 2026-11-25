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
            "IMAP mail.example.org:993: TLS handshake failed: received fatal alert: HandshakeFailure (the server supports none of the TLS ciphers dail implements (TLS 1.2/1.3 with ChaCha20-Poly1305 or AES-GCM, and X25519, P-256 or P-384))",
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

#[test]
fn the_language_setting_beats_the_system() {
    assert_eq!(Lang::resolve(0, Lang::French), Lang::French);
    assert_eq!(Lang::resolve(1, Lang::French), Lang::English);
    assert_eq!(Lang::resolve(2, Lang::English), Lang::French);
    let title = app_front::i18n::title_in(Lang::French, app_core::CommandName::SyncNow);
    assert_eq!(title, "Synchroniser");
}

#[test]
fn a_certificate_failure_with_an_offer_is_fixed_by_trusting_it() {
    let mut state = synced_state();
    let id = *state.accounts.keys().next().unwrap();
    state.accounts[&id].status = AccountStatus::Failed {
        error: "IMAP mail.example.org:993: TLS handshake failed: invalid peer certificate: UnknownIssuer".into(),
        at_ms: 0,
    };
    // No offer yet: test again.
    assert_eq!(problems(&state, Lang::English)[0].fix, Some(Fix::Test(id)));
    let cert = app_core::TrustedCert {
        der: "3082".into(),
        sha256: "ab".repeat(32),
        subject: "CN=mail.example.org\u{1b}[2J".into(),
        issuer: "CN=Private CA".into(),
        not_before_ms: 0,
        not_after_ms: 86_400_000,
    };
    state.status.cert_offers.push(app_core::CertOffer {
        account: id,
        cert: cert.clone(),
    });
    assert_eq!(problems(&state, Lang::English)[0].fix, Some(Fix::Trust(id)));
    assert_eq!(Fix::Trust(id).label(Lang::French), "faire confiance…");

    // The step shows the certificate, cleaned, and choosing it trusts it.
    let actions = app_front::actions::account_actions(&state, id);
    let trust = actions
        .iter()
        .find(|e| e.label == app_front::i18n::Key::TrustCertificateDots)
        .unwrap();
    let app_front::palette::Flow::Ask(step) = trust.run.flow(&state) else {
        panic!("a step");
    };
    let app_front::palette::AskKind::Options(options) = &step.kind else {
        panic!("options");
    };
    assert!(!options[0].0.contains('\u{1b}'), "{:?}", options[0]);
    assert!(
        options[0].1.contains("Issued by CN=Private CA"),
        "{:?}",
        options[0]
    );
    assert!(options[0].1.contains("AB:AB:AB"), "{:?}", options[0]);
    let done = step.answer(&state, app_front::palette::Choice::Index(0));
    let app_front::palette::Flow::Run(cmd) = done else {
        panic!("a command");
    };
    assert_eq!(
        cmd,
        app_core::Command::TrustCertificate {
            account: id,
            sha256: "ab".repeat(32),
        }
    );
    assert!(
        !actions
            .iter()
            .any(|e| e.label == app_front::i18n::Key::ForgetCertificateDots)
    );
    state.status.cert_offers.clear();
    state.accounts[&id].trusted_certs.push(cert);
    let actions = app_front::actions::account_actions(&state, id);
    assert!(
        actions
            .iter()
            .any(|e| e.label == app_front::i18n::Key::ForgetCertificateDots)
    );
}

#[test]
fn heavy_work_is_named_and_the_history_backfill_counted() {
    use app_front::status::{activity, sync_text};
    let mut state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let en = Lang::English;
    assert_eq!(activity(&state, en), None, "nothing runs");

    // The first contact: the account syncs and has no mailbox yet.
    let mut first = state.clone();
    first.mailboxes.clear();
    first.status.sync.get_mut(&account).unwrap().pending = 1;
    assert_eq!(activity(&first, en).unwrap().text, "⟳ loading folders…");
    assert_eq!(sync_text(&first, 0, en), "⟳ loading folders…");

    // History: the backfilling inbox has `cached` of min(window, total).
    let (inbox, _) = crate::support::inbox_of(&state);
    let cached = state
        .messages
        .values()
        .filter(|m| m.mailbox == inbox && !m.is_placeholder())
        .count() as u64;
    state.mailboxes[&inbox].total = u32::try_from(cached * 4).unwrap();
    state.mailboxes[&inbox].sync_window = Some(0);
    state.mailboxes[&inbox].history_complete = false;
    state.status.backfilling.insert(inbox);
    let older = activity(&state, en).unwrap();
    assert_eq!(
        older.text,
        format!("⟳ loading older mail · {cached} of {}", cached * 4)
    );
    assert_eq!(older.progress, Some(0.25));

    // New mail comes first in words; the history's progress stays.
    state.status.fetching.insert(inbox);
    let fetching = activity(&state, en).unwrap();
    assert_eq!(fetching.text, "⟳ loading new mail · Inbox");
    assert_eq!(fetching.progress, Some(0.25));
    assert_eq!(
        activity(&state, Lang::French).unwrap().text,
        "⟳ chargement du nouveau courrier · Courrier entrant"
    );

    // One body is the message being opened; more are the prefetch.
    state.status.fetching.clear();
    state.status.backfilling.clear();
    let ids: Vec<_> = state.messages.keys().copied().take(2).collect();
    state.status.bodies_in_flight.insert(ids[0]);
    assert_eq!(activity(&state, en), None);
    state.status.bodies_in_flight.insert(ids[1]);
    assert_eq!(
        activity(&state, en).unwrap().text,
        "⟳ downloading 2 messages"
    );
}
