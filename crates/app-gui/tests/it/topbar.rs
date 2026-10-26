//! The top bar (sync state, the status chip, the search pill), the sidebar's frame rows and the
//! list footer's undo offer: what the status bar showed before UI-5.

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
    // In the search pill (and in the reader's empty state).
    assert!(h.get_all_by_label("Ctrl+K").next().is_some());
    h.get(By::new().role(Role::Button).label_contains("synced 2m ago"))
        .click();
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
    // The chip sums it up; its popover lists everything with the fixes.
    h.get(
        By::new()
            .role(Role::Button)
            .label_contains("1 pending · 1 failed"),
    )
    .click();
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

#[test]
fn the_search_pill_opens_the_palette() {
    let mut h = gui(synced_state());
    h.run();
    h.get(
        By::new()
            .role(Role::Button)
            .label("Search or run a command"),
    )
    .click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::OpenPalette {}]);
}

#[test]
fn the_settings_row_opens_the_settings_page() {
    let mut h = gui(synced_state());
    h.run();
    h.get(By::new().role(Role::Button).label("Settings"))
        .click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::OpenSettings { section: None }]
    );
}

#[test]
fn the_rail_names_its_icons() {
    let mut h = gui(synced_state());
    apply(&mut h, Command::ToggleSidebar {});
    h.run();
    for name in [
        "Expand sidebar (Ctrl+B)",
        "Compose (C)",
        "Inbox",
        "Sent",
        "Settings",
    ] {
        h.get(By::new().role(Role::Button).label(name));
    }
}

#[test]
fn the_list_footer_counts_the_conversations() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.run();
    h.get_by_label("2 CONVERSATIONS");
}

#[test]
fn cozy_rows_are_48_high() {
    use egui_kittest::kittest::NodeT;
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.run();
    let row = h.get(
        By::new()
            .role(Role::Button)
            .predicate(|n| n.label().is_some_and(|l| l.contains("Lunch?"))),
    );
    let rect = row.accesskit_node().bounding_box().unwrap();
    assert!((rect.height() - 48.0).abs() < 0.5, "{rect:?}");
}

#[test]
fn compact_rows_are_40_high() {
    use egui_kittest::kittest::NodeT;
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(
        &mut h,
        Command::SetSetting {
            key: app_core::SettingKey::ListDensity,
            value: 1.into(),
        },
    );
    h.run();
    let row = h.get(
        By::new()
            .role(Role::Button)
            .predicate(|n| n.label().is_some_and(|l| l.contains("Lunch?"))),
    );
    let rect = row.accesskit_node().bounding_box().unwrap();
    assert!((rect.height() - 40.0).abs() < 0.5, "{rect:?}");
}

#[test]
fn preview_rows_are_72_high_and_show_the_snippet() {
    use egui_kittest::kittest::NodeT;
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let newest = *state.threads[&threads[0]].messages.last().unwrap();
    state.messages.get_mut(&newest).unwrap().snippet = Some("Noon at the usual place?".into());
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.run();
    // Cozy rows put the snippet after the subject.
    h.get_by_label_contains("Noon at the usual place?");
    apply(
        &mut h,
        Command::SetSetting {
            key: app_core::SettingKey::ListDensity,
            value: 2.into(),
        },
    );
    h.run();
    let row = h.get(
        By::new()
            .role(Role::Button)
            .predicate(|n| n.label().is_some_and(|l| l.contains("Lunch?"))),
    );
    let rect = row.accesskit_node().bounding_box().unwrap();
    assert!((rect.height() - 72.0).abs() < 0.5, "{rect:?}");
    h.get_by_label("Noon at the usual place?");
}
