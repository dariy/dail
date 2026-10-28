//! The account wizard: preset → address → password → test → sync.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Gui, gui, ingest, synced_state};
use app_core::{AccountId, AppState, Command, Endpoint, Event, ProviderPreset};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

fn button(h: &Harness<'static, Gui>, label: &str) {
    h.get(By::new().role(Role::Button).label(label)).click();
}

fn type_into(h: &mut Harness<'static, Gui>, label: &str, text: &str) {
    // Text fields are labelled by their caption (the password field has its own role).
    h.get_by_label(label).click();
    h.run();
    h.event(egui::Event::Text(text.to_owned()));
    h.run();
}

/// Runs the wizard up to the test step for a Hostinger account; returns the new account.
fn add_hostinger(h: &mut Harness<'static, Gui>) -> AccountId {
    button(h, "Hostinger Mail");
    h.run();
    button(h, "Next");
    h.run();
    type_into(h, "Email address", "alice@example.com");
    button(h, "Next");
    h.run();
    type_into(h, "Password", "hunter2");
    button(h, "Add and test");
    h.run();
    // add_account goes out as the wizard's background request; its reply triggers the test.
    assert!(h.state_mut().pump().is_empty());
    h.run();
    let test = h.state_mut().pump();
    let [Command::TestAccount { id }] = test[..] else {
        panic!("expected test_account, got {test:?}");
    };
    h.run();
    id
}

#[test]
fn it_opens_on_first_run_and_adds_tests_and_syncs_an_account() {
    let mut h = gui(AppState::default());
    h.run();
    h.get_by_label("Add an account");
    let id = add_hostinger(&mut h);
    let account = &h.state().engine.state().accounts[&id];
    assert_eq!(account.preset, ProviderPreset::Hostinger);
    assert_eq!(account.email, "alice@example.com");
    let saved = serde_json::to_string(h.state().engine.state()).unwrap();
    assert!(
        !saved.contains("hunter2"),
        "the password never reaches the state"
    );
    h.get_by_label("Signing in to alice@example.com…");
    ingest(&mut h, Event::AccountVerified { account: id });
    h.get_by_label_contains("Connected");
    button(&h, "Sync now");
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::SyncNow { account: Some(id) }]
    );
    h.run();
    assert!(h.query_by_label("Add an account").is_none(), "closed");
}

#[test]
fn a_failed_login_can_be_retried_with_another_password() {
    let mut h = gui(AppState::default());
    h.run();
    let id = add_hostinger(&mut h);
    ingest(
        &mut h,
        Event::AccountVerifyFailed {
            account: id,
            error: "IMAP LOGIN failed: authentication failed".into(),
        },
    );
    // The wizard and the test's result window (MISC-22) both say why.
    assert!(
        h.query_all_by_label_contains("authentication failed")
            .next()
            .is_some()
    );
    button(&h, "Try another password");
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::RemoveAccount { id, confirm: true }]
    );
    h.run();
    h.get_by_label("3 / 4  PASSWORD");
    assert!(h.state().engine.state().accounts.is_empty());
}

#[test]
fn custom_servers_must_parse() {
    let mut h = gui(AppState::default());
    h.run();
    button(&h, "Other IMAP / SMTP");
    h.run();
    button(&h, "Next");
    h.run();
    type_into(&mut h, "Email address", "me@example.org");
    type_into(&mut h, "IMAP server", "imap.example.org");
    type_into(&mut h, "SMTP server", "smtp.example.org:587");
    h.get_by_label_contains("expected host:port");
    button(&h, "Next");
    h.run();
    h.get_by_label("2 / 4  ADDRESS");
    type_into(&mut h, "IMAP server", ":993");
    button(&h, "Next");
    h.run();
    h.get_by_label("3 / 4  PASSWORD");
}

#[test]
fn custom_servers_can_be_found_from_the_address() {
    let mut h = gui(AppState::default());
    h.run();
    button(&h, "Other IMAP / SMTP");
    h.run();
    button(&h, "Next");
    h.run();
    h.get_by_label_contains("then Mozilla's list");
    type_into(&mut h, "Email address", "me@example.org");
    button(&h, "Find servers");
    h.run();
    assert_eq!(
        h.state_mut()
            .out
            .iter()
            .map(|r| r.cmd.clone())
            .collect::<Vec<_>>(),
        [Command::DiscoverServers {
            email: "me@example.org".into()
        }]
    );
    assert!(h.state_mut().pump().is_empty(), "the wizard's own request");
    h.run();
    h.get_by_label("Looking…");
    ingest(
        &mut h,
        Event::ServersDiscovered {
            email: "me@example.org".into(),
            imap: Endpoint::parse("imap.example.org:993").unwrap(),
            smtp: Endpoint::parse("smtp.example.org:587").unwrap(),
            source: "https://autoconfig.example.org/mail/config-v1.1.xml".into(),
        },
    );
    h.run();
    h.get_by_label_contains("Found at https://autoconfig.example.org");
    assert!(
        h.query_by_label_contains("\"state\"").is_none(),
        "no result window"
    );
    button(&h, "Next");
    h.run();
    h.get_by_label("3 / 4  PASSWORD");
    type_into(&mut h, "Password", "hunter2");
    button(&h, "Add and test");
    h.run();
    h.state_mut().pump();
    let account = h.state().engine.state().accounts.values().next().unwrap();
    assert_eq!(
        account.imap,
        Endpoint::parse("imap.example.org:993").unwrap()
    );
    assert_eq!(
        account.smtp,
        Endpoint::parse("smtp.example.org:587:starttls").unwrap()
    );
}

#[test]
fn it_opens_from_the_sidebar_and_cancels() {
    let mut h = gui(synced_state());
    h.run();
    assert!(
        h.query_by_label("Add an account").is_none(),
        "not on first run with accounts"
    );
    h.get_by_label("+ Add account").click();
    h.run();
    h.get_by_label("Add an account");
    h.get(By::new().role(Role::Button).label_contains("Cancel"))
        .click();
    h.run();
    assert!(h.query_by_label("Add an account").is_none());
    assert!(h.state_mut().pump().is_empty());
}

#[test]
fn a_failed_login_offers_to_re_enter_the_password() {
    let mut state = synced_state();
    let id = *state.accounts.keys().next().unwrap();
    state.accounts[&id].status = app_core::AccountStatus::Failed {
        error: "IMAP LOGIN failed: [AUTHENTICATIONFAILED] Invalid credentials".into(),
        at_ms: 0,
    };
    let mut h = gui(state);
    h.run();
    // The top bar's status chip opens the list of problems with their fixes.
    h.get(By::new().role(Role::Button).label_contains("login failed"))
        .click();
    h.run();
    button(&h, "re-enter password");
    h.run();
    h.run();
    h.get_by_label("Change the password");
    type_into(&mut h, "Password", "new secret");
    button(&h, "Save and test");
    h.run();
    let sent: Vec<Command> = h.state().out.iter().map(|r| r.cmd.clone()).collect();
    let [
        Command::SetPassword {
            id: changed,
            password,
        },
    ] = &sent[..]
    else {
        panic!("{sent:?}")
    };
    assert_eq!((*changed, password.expose()), (id, "new secret"));
    assert!(h.state_mut().pump().is_empty(), "the form's own request");
    h.run();
    h.get_by_label_contains("Signing in");
    ingest(
        &mut h,
        Event::AccountVerifyFailed {
            account: id,
            error: "IMAP LOGIN failed: still wrong".into(),
        },
    );
    button(&h, "Try another password");
    h.run();
    assert!(h.state_mut().pump().is_empty(), "the account is kept");
    h.run();
    h.get_by_label("Change the password");
    assert!(h.state().engine.state().accounts.contains_key(&id));
}
