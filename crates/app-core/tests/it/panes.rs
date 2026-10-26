//! GUI view commands: `open_composer`, `close_composer`, `focus_pane`, `toggle_sidebar`, and the
//! `theme` setting.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail};
use app_core::{
    Command, CommandKind, CoreError, DraftId, Effect, EntityRef, Pane, PersistDelta, SettingKey,
    ViewPrefs,
};
use serde_json::json;

fn with_thread() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Plans", true)]);
    m
}

fn new_draft(m: &mut Mail) -> DraftId {
    let outcome = m
        .run(Command::CreateDraft {
            account: m.account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Hi".into(),
            body: String::new(),
        })
        .unwrap();
    serde_json::from_value(outcome.receipt.data["id"].clone()).unwrap()
}

#[test]
fn open_composer_shows_the_draft_and_focuses_it() {
    let mut m = with_thread();
    let draft = new_draft(&mut m);
    let undo_len = m.engine.history().undo_len();
    let outcome = m.run(Command::OpenComposer { id: draft }).unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::View));
    assert!(outcome.receipt.affected.contains(&EntityRef::View));
    assert_eq!(outcome.receipt.data, json!({ "composer": draft }));
    assert!(outcome.effects.is_empty());
    let view = &m.engine.state().view;
    assert_eq!(view.composer, Some(draft));
    assert_eq!(view.focus, Pane::Composer);
    assert_eq!(m.engine.history().undo_len(), undo_len, "not recorded");
}

#[test]
fn open_composer_rejects_unknown_drafts() {
    let mut m = with_thread();
    let err = m
        .run(Command::OpenComposer {
            id: DraftId::from_uuid(uuid::Uuid::from_u128(77)),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "draft"));
    assert_eq!(m.engine.state().view.composer, None);
}

#[test]
fn close_composer_keeps_the_draft_and_moves_the_focus() {
    let mut m = with_thread();
    let thread = m.thread_of(m.message("INBOX", 1));
    m.run(Command::OpenThread { id: thread }).unwrap();
    let draft = new_draft(&mut m);
    m.run(Command::OpenComposer { id: draft }).unwrap();
    let outcome = m.run(Command::CloseComposer {}).unwrap();
    assert_eq!(outcome.receipt.data, json!({ "closed": draft }));
    let state = m.engine.state();
    assert_eq!(state.view.composer, None);
    assert_eq!(
        state.view.focus,
        Pane::Reader,
        "the open thread takes the focus"
    );
    assert!(state.drafts.contains_key(&draft));
    // Closing again is harmless.
    let again = m.run(Command::CloseComposer {}).unwrap();
    assert_eq!(again.receipt.data, json!({ "closed": null }));
}

#[test]
fn discarding_the_open_draft_closes_the_composer_and_undo_does_not_reopen_it() {
    let mut m = with_thread();
    let draft = new_draft(&mut m);
    m.run(Command::OpenComposer { id: draft }).unwrap();
    m.run(Command::DiscardDraft { id: draft }).unwrap();
    let view = &m.engine.state().view;
    assert_eq!(view.composer, None);
    assert_eq!(view.focus, Pane::List);
    m.run(Command::Undo {}).unwrap();
    assert!(m.engine.state().drafts.contains_key(&draft));
    assert_eq!(m.engine.state().view.composer, None);
}

#[test]
fn focus_pane_moves_the_focus_to_shown_panes_only() {
    let mut m = with_thread();
    let outcome = m
        .run(Command::FocusPane {
            pane: Pane::Sidebar,
        })
        .unwrap();
    assert_eq!(outcome.receipt.data, json!({ "focus": "sidebar" }));
    assert_eq!(m.engine.state().view.focus, Pane::Sidebar);
    for (pane, reason) in [
        (Pane::Reader, "no thread is open (open_thread)"),
        (Pane::Composer, "no draft is open (open_composer)"),
    ] {
        let err = m.run(Command::FocusPane { pane }).unwrap_err();
        assert_eq!(
            err,
            CoreError::InvalidArgument {
                field: "pane".into(),
                reason: reason.into()
            }
        );
        assert_eq!(m.engine.state().view.focus, Pane::Sidebar, "unchanged");
    }
    let thread = m.thread_of(m.message("INBOX", 1));
    m.run(Command::OpenThread { id: thread }).unwrap();
    m.run(Command::FocusPane { pane: Pane::Reader }).unwrap();
    assert_eq!(m.engine.state().view.focus, Pane::Reader);
    m.run(Command::CloseThread {}).unwrap();
    assert_eq!(m.engine.state().view.focus, Pane::List, "the reader closed");
}

#[test]
fn toggle_sidebar_collapses_and_focusing_the_sidebar_expands_it() {
    let mut m = with_thread();
    m.run(Command::FocusPane {
        pane: Pane::Sidebar,
    })
    .unwrap();
    let outcome = m.run(Command::ToggleSidebar {}).unwrap();
    assert_eq!(outcome.receipt.data, json!({ "sidebar_collapsed": true }));
    let view = &m.engine.state().view;
    assert!(view.sidebar_collapsed);
    assert_eq!(
        view.focus,
        Pane::List,
        "a collapsed sidebar can't keep the focus"
    );
    m.run(Command::FocusPane {
        pane: Pane::Sidebar,
    })
    .unwrap();
    let view = &m.engine.state().view;
    assert!(!view.sidebar_collapsed);
    assert_eq!(view.focus, Pane::Sidebar);
    m.run(Command::ToggleSidebar {}).unwrap();
    m.run(Command::ToggleSidebar {}).unwrap();
    assert!(!m.engine.state().view.sidebar_collapsed);
}

#[test]
fn view_commands_that_change_preferences_save_them() {
    let mut m = with_thread();
    let undo_len = m.engine.history().undo_len();
    let saved = |outcome: &app_core::Outcome| {
        outcome
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::PersistState {
                    delta: PersistDelta::View(prefs),
                } => Some(prefs.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let outcome = m.run(Command::ToggleSidebar {}).unwrap();
    assert_eq!(
        saved(&outcome),
        [ViewPrefs {
            sidebar_collapsed: true,
            ..ViewPrefs::default()
        }]
    );
    let outcome = m
        .run(Command::FocusPane {
            pane: Pane::Sidebar,
        })
        .unwrap();
    assert_eq!(
        saved(&outcome),
        [ViewPrefs::default()],
        "focusing expands it"
    );
    let outcome = m.run(Command::FocusPane { pane: Pane::List }).unwrap();
    assert!(saved(&outcome).is_empty(), "nothing saved changed");
    assert_eq!(
        m.engine.history().undo_len(),
        undo_len,
        "view changes are not undoable"
    );
}

#[test]
fn theme_setting_is_range_checked_and_undoable() {
    let mut m = with_thread();
    assert_eq!(m.engine.state().settings.theme, 0);
    m.run(Command::SetSetting {
        key: SettingKey::Theme,
        value: json!(2),
    })
    .unwrap();
    assert_eq!(m.engine.state().settings.theme, 2);
    let err = m
        .run(Command::SetSetting {
            key: SettingKey::Theme,
            value: json!(5),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { .. }));
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().settings.theme, 0);
    m.run(Command::Redo {}).unwrap();
    assert_eq!(m.engine.state().settings.theme, 2);
}

#[test]
fn get_view_reports_the_view_without_changing_it() {
    let mut m = with_thread();
    let thread = m.thread_of(m.message("INBOX", 1));
    m.run(Command::OpenThread { id: thread }).unwrap();
    let before = m.engine.state().clone();
    let outcome = m.run(Command::GetView {}).unwrap();
    assert_eq!(m.engine.state(), &before, "a query changes nothing");
    assert!(outcome.effects.is_empty());
    let data = &outcome.receipt.data;
    assert_eq!(data["thread"], json!(thread));
    assert_eq!(data["selected"], json!([thread]));
    assert_eq!(data["focus"], "list");
    assert_eq!(data["palette_open"], false);
    assert_eq!(
        serde_json::from_value::<app_core::ViewState>(data.clone()).unwrap(),
        before.view
    );
}
