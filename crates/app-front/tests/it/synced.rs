//! The settings account and the sync state shown on the settings screens (ADR 0015, SET-3).

// reason: helpers and `let … else` arms may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::panic)]

use app_core::{AppState, Command, CommandName, SyncIssue};
use app_front::i18n::{Key, s};
use app_front::palette::{self, Choice, Flow};
use app_front::synced::panel;

use crate::support::synced_state;

#[test]
fn no_accounts_no_panel() {
    assert_eq!(panel(&AppState::default()), None);
}

#[test]
fn the_first_account_is_the_default_and_the_choice_shows() {
    let mut state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let email = state.accounts[&account].email.clone();
    let p = panel(&state).unwrap();
    assert_eq!(p.account, account);
    assert!(p.account_label.starts_with(&email));
    assert_ne!(p.account_label, email, "marked as the first account");

    state.settings.settings_account = Some(account);
    state.synced.pulled_ms = Some(1);
    let p = panel(&state).unwrap();
    assert_eq!(p.account_label, email);
    assert_eq!(p.status, s(Key::SettingsUpToDate));
    assert!(!p.failed);
}

#[test]
fn the_error_shows_with_its_code() {
    let mut state = synced_state();
    state.synced.error = Some(SyncIssue {
        error: "ERR020: the settings document has 300000 bytes, more than the 262144 allowed"
            .into(),
        mailbox: None,
        at_ms: 0,
    });
    let p = panel(&state).unwrap();
    assert!(p.failed);
    assert!(p.status.starts_with("ERR020: "));
}

#[test]
fn the_palette_asks_for_the_account() {
    let state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let Flow::Ask(step) = palette::flow(CommandName::SetSettingsAccount, &state) else {
        panic!("asks for the account");
    };
    assert_eq!(step.prompt, s(Key::SettingsAccountWhich));
    let Flow::Run(cmd) = step.answer(&state, Choice::Account(account)) else {
        panic!("runs the command");
    };
    assert_eq!(cmd, Command::SetSettingsAccount { account });
}
