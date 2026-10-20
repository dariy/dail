//! Sending with the undo window, attaching a typed path, and quitting while a send waits.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{Addr, AttachmentState, Command, Event};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Term, inbox_of, synced_state};

/// A reply to Bob in the composer, ready to send.
fn replying() -> (Term, app_core::DraftId) {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 80, 24);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[1] });
    t.out.clear();
    t.key(KeyCode::Char('r'));
    let sent = t.pump();
    let Some(Command::OpenComposer { id }) = sent.last().cloned() else {
        unreachable!("{sent:?}");
    };
    assert_eq!(
        t.engine.state().drafts[&id].to,
        [Addr {
            name: Some("Bob".into()),
            email: "bob@example.org".into()
        }]
    );
    (t, id)
}

#[test]
fn ctrl_s_sends_with_the_undo_window_and_z_cancels() {
    let (mut t, id) = replying();
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('s'));
    assert_eq!(t.pump(), [Command::SendDraft { id }]);
    let screen = t.screen();
    let status = screen.lines().last().unwrap();
    assert!(
        status.starts_with("Sending in 10 s · z cancels"),
        "{status}"
    );
    assert!(screen.contains("read-only while it is sent"), "{screen}");
    // The composer is read-only: typing changes nothing; Escape still closes it.
    t.typed("more");
    assert!(t.take().is_empty());
    t.key(KeyCode::Esc);
    assert_eq!(t.pump(), [Command::CloseComposer {}]);
    t.key(KeyCode::Char('z'));
    t.pump();
    assert_eq!(t.engine.state().drafts[&id].send_at, None, "cancelled");
    assert!(!t.screen().contains("Sending in"));
}

#[test]
fn a_typed_path_is_attached_with_home_expanded_and_its_state_shows() {
    let (mut t, id) = replying();
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("attach file");
    t.key(KeyCode::Enter);
    t.typed("~/notes.txt");
    t.key(KeyCode::Enter);
    let sent = t.pump();
    let home = std::env::var("HOME").unwrap();
    let path = format!("{home}/notes.txt");
    assert_eq!(
        sent[0],
        Command::AttachFile {
            draft: id,
            path: path.clone(),
            index: None
        }
    );
    assert!(t.screen().contains(&format!("@ {path} (checking…)")));
    t.ingest(Event::AttachmentInspected {
        draft: id,
        path: path.clone(),
        size: 2_048,
        content_type: "text/plain".into(),
    });
    assert!(t.screen().contains("(2 KB)"));
    assert!(matches!(
        t.engine.state().drafts[&id].attachments[0].state,
        AttachmentState::Ready { .. }
    ));
}

#[test]
fn quitting_an_owned_session_while_a_send_waits_asks_first() {
    let (mut t, _) = replying();
    t.tui = std::mem::take(&mut t.tui).owning_session(true);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('s'));
    t.pump();
    t.key(KeyCode::Esc);
    t.pump();
    // q closes the thread first, then it would quit.
    t.key(KeyCode::Char('q'));
    assert_eq!(t.pump(), [Command::CloseThread {}]);
    t.key(KeyCode::Char('q'));
    assert!(!t.tui.quit_requested());
    let status = t.screen().lines().last().unwrap().to_owned();
    assert!(status.starts_with("Quitting cancels the send"), "{status}");
    t.key(KeyCode::Char('j'));
    assert!(!t.tui.quit_requested(), "any other key stays");
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('c'));
    assert!(!t.tui.quit_requested(), "asks again");
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('c'));
    assert!(t.tui.quit_requested());
}

#[test]
fn an_attached_tui_quits_at_once_the_owner_keeps_the_send() {
    let (mut t, _) = replying();
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('s'));
    t.pump();
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('c'));
    assert!(t.tui.quit_requested());
}
