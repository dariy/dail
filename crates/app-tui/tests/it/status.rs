//! The status line: sync state, problems, pending changes, the latest undoable action and
//! notices, fitted to the width; `?` lists the keys.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{AccountStatus, Command, Ctx, Engine, OpState, Origin};
use ratatui::crossterm::event::KeyCode;

use crate::support::{Term, fixed_now, inbox_of, synced_state};

fn last_line(screen: &str) -> &str {
    screen.lines().last().unwrap_or("")
}

#[test]
fn problems_and_failed_changes_come_before_the_sync_state() {
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
    let mut t = Term::new(state, 120, 24);
    let line = last_line(&t.screen()).to_owned();
    assert!(
        line.starts_with("keyring failed (+1 more, ? for details)"),
        "{line}"
    );
    assert!(
        line.contains("1 pending · 1 failed (retry: palette)"),
        "{line}"
    );
    assert!(line.ends_with("? help"), "{line}");
    t.key(KeyCode::Char('?'));
    let help = t.screen();
    assert!(
        help.contains("alice@example.com: login failed: IMAP LOGIN failed"),
        "{help}"
    );
    // On a phone the most important part stays.
    let mut t = Term::new(t.engine.state().clone(), 40, 20);
    insta::assert_snapshot!(last_line(&t.screen()));
}

#[test]
fn the_latest_undoable_action_and_rejections_show_up() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 80, 24);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[0] });
    t.key(KeyCode::Char('e'));
    t.pump();
    let line = last_line(&t.screen()).to_owned();
    assert!(line.contains("z undo"), "{line}");
    t.key(KeyCode::Char('z'));
    t.pump();
    assert!(!last_line(&t.screen()).contains("z undo"), "undone");
    t.key(KeyCode::Char('z'));
    t.key(KeyCode::Char('z'));
    t.pump();
    let line = last_line(&t.screen()).to_owned();
    assert!(line.starts_with("Undo: "), "the rejection: {line}");
}

#[test]
fn question_mark_lists_the_registry_keys_and_the_tuis_own() {
    // Tall enough for every keyed command and the TUI's own keys.
    let mut t = Term::new(synced_state(), 80, 50);
    t.key(KeyCode::Char('?'));
    let help = t.screen();
    for line in ["Ctrl+S", "Shift+Z", "g i", "In the TUI", "q Backspace"] {
        assert!(help.contains(line), "{line}:\n{help}");
    }
    assert!(
        !help.contains("Ctrl+Enter"),
        "only keys a terminal delivers"
    );
    t.key(KeyCode::Char('?'));
    assert!(!t.screen().contains("In the TUI"));
    assert!(t.take().is_empty());
}
