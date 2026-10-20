//! The status bar: sync state, pending changes, errors and the latest undoable action.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{apply, fixed_now, gui, inbox_of, synced_state};
use app_core::{AccountStatus, Command, Ctx, Engine, OpState, Origin};
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};

#[test]
fn sync_state_and_the_poll_hint_are_shown() {
    let mut state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    state.status.sync.get_mut(&account).unwrap().last_sync_ms = Some(fixed_now() - 120_000);
    let mut h = gui(state);
    h.run();
    h.get_by_label("synced 2m ago");
    h.get_by_label("Ctrl+K");
    h.get(By::new().role(Role::Button).label("sync")).click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::SyncNow { account: None }]);
}

#[test]
fn failures_are_listed_and_failed_changes_can_be_retried() {
    let state = synced_state();
    let (_, threads) = inbox_of(&state);
    let account = *state.accounts.keys().next().unwrap();
    let mut engine = Engine::new(state);
    let ctx = Ctx {
        now_ms: fixed_now(),
        id_seed: 7 << 64,
        origin: Origin::Cli,
    };
    engine
        .dispatch(
            Command::MarkRead {
                ids: vec![threads[0]],
                messages: Vec::new(),
            },
            &ctx,
        )
        .unwrap();
    let mut state = engine.state().clone();
    let op = state.outbox.values_mut().next().unwrap();
    op.state = OpState::Failed;
    op.last_error = Some("NO [ALERT] mailbox is read-only".into());
    state.status.keyring_error = Some("secret service unavailable".into());
    state.accounts.get_mut(&account).unwrap().status = AccountStatus::Failed {
        error: "IMAP LOGIN failed".into(),
        at_ms: fixed_now(),
    };
    let mut h = gui(state);
    h.run();
    h.get_by_label("1 pending · 1 failed");
    h.get_by_label("keyring failed");
    h.get_by_label("alice@example.com: login failed");
    h.get(By::new().role(Role::Button).label("retry")).click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::RetryPendingOps { id: None }]
    );
}

#[test]
fn the_latest_undoable_action_is_offered_until_undone() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.key_press(Key::S);
    h.run();
    h.state_mut().pump();
    h.run();
    h.get_by_label_contains("“Star");
    h.get(By::new().role(Role::Button).label_contains("Undo"))
        .click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::Undo {}]);
    h.run();
    assert!(
        h.query_by_label_contains("“").is_none(),
        "nothing left to offer"
    );
}
