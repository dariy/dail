//! Account setup in the TUI: preset → address → masked password → add_account → test_account →
//! sync_now, and another password after a failed login.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, Endpoint, Event, ProviderPreset};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::Term;

/// Walks the wizard up to the password and submits it. Returns the new account.
fn added(t: &mut Term) -> app_core::AccountId {
    // No accounts: Enter starts the setup.
    t.key(KeyCode::Enter);
    assert!(t.tui.setup_open());
    t.key(KeyCode::Enter); // Gmail
    t.typed("alice@gmail.com");
    t.key(KeyCode::Enter);
    t.key(KeyCode::Enter); // no name
    t.typed("app pass word");
    let screen = t.screen();
    assert!(
        !screen.contains("app pass"),
        "the password is masked:\n{screen}"
    );
    assert!(screen.contains("App Password: •••••••••••••"), "{screen}");
    t.key(KeyCode::Enter);
    // The setup's own request (its reply carries the new account's ID).
    let Some(Command::AddAccount {
        preset,
        email,
        password,
        ..
    }) = t.out.first().map(|r| r.cmd.clone())
    else {
        panic!("{:?}", t.out);
    };
    assert_eq!(preset, ProviderPreset::Gmail);
    assert_eq!(email, "alice@gmail.com");
    assert_eq!(password.as_ref().unwrap().expose(), "app pass word");
    // add_account, then (from its reply) test_account, both the setup's own.
    assert!(t.pump().is_empty());
    let id = *t.engine.state().accounts.keys().next().unwrap();
    let screen = t.screen();
    assert!(screen.contains("Testing the login"), "{screen}");
    id
}

#[test]
fn a_new_account_is_added_tested_and_synced() {
    let mut t = Term::new(AppState::default(), 40, 20);
    insta::assert_snapshot!(t.screen());
    let id = added(&mut t);
    t.ingest(Event::AccountVerified { account: id });
    assert!(t.screen().contains("Connected."));
    t.key(KeyCode::Enter);
    assert_eq!(t.pump(), [Command::SyncNow { account: Some(id) }]);
    assert!(!t.tui.setup_open());
}

#[test]
fn a_failed_login_can_be_retried_with_another_password() {
    let mut t = Term::new(AppState::default(), 80, 24);
    let id = added(&mut t);
    t.ingest(Event::AccountVerifyFailed {
        account: id,
        error: "IMAP LOGIN failed: [AUTHENTICATIONFAILED]".into(),
    });
    let screen = t.screen();
    assert!(screen.contains("AUTHENTICATIONFAILED"), "{screen}");
    t.key(KeyCode::Char('p'));
    assert_eq!(t.pump(), [Command::RemoveAccount { id, confirm: true }]);
    let screen = t.screen();
    assert!(
        screen.contains("App Password: ▏"),
        "an empty field again:\n{screen}"
    );
}

#[test]
fn custom_servers_are_checked_before_the_password() {
    let mut t = Term::new(AppState::default(), 80, 24);
    t.key(KeyCode::Enter);
    t.key(KeyCode::Down);
    t.key(KeyCode::Down);
    t.key(KeyCode::Enter); // Other IMAP / SMTP
    t.typed("me@example.com");
    t.key(KeyCode::Tab);
    t.key(KeyCode::Tab);
    t.typed("imap.example.com:notaport");
    t.key(KeyCode::Tab);
    t.typed("smtp.example.com:465");
    t.key(KeyCode::Enter);
    let screen = t.screen();
    assert!(screen.contains("IMAP:"), "{screen}");
    assert!(!screen.contains("Password:"));
    assert!(t.take().is_empty());
}

#[test]
fn ctrl_f_finds_the_custom_servers() {
    let mut t = Term::new(AppState::default(), 120, 24);
    t.key(KeyCode::Enter);
    t.key(KeyCode::Down);
    t.key(KeyCode::Down);
    t.key(KeyCode::Enter); // Other IMAP / SMTP
    t.typed("me@example.org");
    assert!(t.screen().contains("Ctrl+F finds the servers"));
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('f'));
    assert_eq!(
        t.out.iter().map(|r| r.cmd.clone()).collect::<Vec<_>>(),
        [Command::DiscoverServers {
            email: "me@example.org".into()
        }],
        "the setup's own request: its receipt is not shown"
    );
    assert!(t.pump().is_empty());
    assert!(t.tui.setup_open());
    t.ingest(Event::ServersDiscovered {
        email: "me@example.org".into(),
        imap: Endpoint::parse("imap.example.org:993").unwrap(),
        smtp: Endpoint::parse("smtp.example.org:587").unwrap(),
        source: "https://autoconfig.example.org/mail/config-v1.1.xml".into(),
    });
    let screen = t.screen();
    assert!(
        screen.contains("Found at https://autoconfig.example.org"),
        "{screen}"
    );
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('f'));
    assert!(t.take().is_empty(), "no second lookup");
    let screen = t.screen();
    assert!(
        screen.contains("IMAP host:port: imap.example.org:993:tls"),
        "{screen}"
    );
    assert!(
        screen.contains("SMTP host:port: smtp.example.org:587:starttls"),
        "{screen}"
    );
    t.key(KeyCode::Enter);
    t.key(KeyCode::Enter);
    t.key(KeyCode::Enter);
    t.key(KeyCode::Enter);
    assert!(t.screen().contains("Password:"), "the servers parse");
}

#[test]
fn the_palette_changes_a_password_through_the_setup() {
    let mut state = crate::support::synced_state();
    let id = *state.accounts.keys().next().unwrap();
    state.accounts[&id].status = app_core::AccountStatus::Failed {
        error: "IMAP LOGIN failed: [AUTHENTICATIONFAILED]".into(),
        at_ms: 0,
    };
    let mut t = Term::new(state, 120, 24);
    let screen = t.screen();
    assert!(
        screen.contains("login failed (palette: Change account password)"),
        "{screen}"
    );
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("change account password");
    t.key(KeyCode::Enter); // the command
    t.key(KeyCode::Enter); // the only account
    assert_eq!(t.pump(), [Command::ClosePalette {}]);
    assert!(t.tui.setup_open());
    let screen = t.screen();
    assert!(screen.contains("Change the password"), "{screen}");
    t.typed("new secret");
    t.key(KeyCode::Enter);
    let [
        Command::SetPassword {
            id: changed,
            password,
        },
    ] = &t.out.iter().map(|r| r.cmd.clone()).collect::<Vec<_>>()[..]
    else {
        panic!("{:?}", t.out)
    };
    assert_eq!((*changed, password.expose()), (id, "new secret"));
    assert!(t.pump().is_empty());
    assert!(t.screen().contains("Testing the login"));
    t.ingest(Event::AccountVerified { account: id });
    assert!(t.screen().contains("Connected."));
}
