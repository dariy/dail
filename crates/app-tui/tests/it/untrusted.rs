//! Email content can't send control sequences to the terminal: a subject, names, addresses and a
//! body carrying `ESC ]52;…` (set the clipboard), `ESC [2J` (clear the screen), `\r` and U+202E
//! (right-to-left override) leave none of them in the buffer of the list, the reader, the
//! palette, a query's result or the approvals — only U+FFFD where they were.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{Addr, AppState, Command, Ctx, Origin};
use app_tui::text::is_unsafe;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Term, fixed_now, inbox_of, synced_state};

const NASTY: &str = "\u{1b}]52;c;ZXZpbA==\u{7}Pay\u{1b}[2Jnow\r\u{202e}gpj.exe\u{9b}31m";

/// The inbox's threads, with every attacker-controlled field of Bob's message set to `NASTY`.
fn poisoned() -> (AppState, app_core::ThreadId) {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let thread = threads[1];
    let message = state.threads[&thread].messages[0];
    state.threads.get_mut(&thread).unwrap().subject = NASTY.to_owned();
    let envelope = state.messages.get_mut(&message).unwrap();
    envelope.subject = NASTY.to_owned();
    let nasty = Addr {
        name: Some(NASTY.to_owned()),
        email: format!("bob{NASTY}@example.org"),
    };
    envelope.from = vec![nasty.clone()];
    envelope.to.push(nasty.clone());
    envelope.cc = vec![nasty];
    state.view.mailbox = Some(inbox);
    (state, thread)
}

/// Draws and checks that nothing unsafe reached the buffer (and that something was replaced).
fn assert_clean(t: &mut Term, what: &str) {
    assert_safe(t, what);
    let screen = t.screen();
    assert!(
        screen.contains('\u{fffd}'),
        "{what} shows the poisoned text:\n{screen}"
    );
}

/// Draws and checks that nothing unsafe reached the buffer.
fn assert_safe(t: &mut Term, what: &str) {
    let screen = t.screen();
    let buffer = t.terminal.backend().buffer();
    for cell in &buffer.content {
        assert!(
            !cell.symbol().chars().any(is_unsafe),
            "{what}: {:?} reached the terminal\n{screen}",
            cell.symbol()
        );
    }
}

#[test]
fn no_widget_passes_control_sequences_through() {
    let (state, thread) = poisoned();
    for (width, height) in [(40, 20), (80, 24), (120, 40)] {
        let mut t = Term::new(state.clone(), width, height);
        assert_clean(&mut t, "the list");

        t.apply(Command::OpenThread { id: thread });
        t.deliver_bodies(&[NASTY]);
        assert_clean(&mut t, "the reader");

        t.apply(Command::CloseThread {});
        t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
        t.pump();
        // The poisoned thread among the recent threads.
        t.typed("Pay");
        assert_clean(&mut t, "the palette");
        t.input(app_tui::Input::Key(
            ratatui::crossterm::event::KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        ));
        t.typed("show thread");
        t.key(KeyCode::Enter);
        t.key(KeyCode::Enter);
        t.pump();
        // The JSON escapes C0 controls itself; whatever it leaves is cleaned.
        assert_safe(&mut t, "a query's result");
    }
}

#[test]
fn an_agents_poisoned_draft_is_shown_cleaned() {
    let (state, _) = poisoned();
    let account = *state.accounts.keys().next().unwrap();
    let mut t = Term::new(state, 80, 30);
    let agent = |seed: u128| Ctx {
        now_ms: fixed_now(),
        id_seed: seed << 64,
        origin: Origin::Mcp,
    };
    let created = t
        .engine
        .dispatch(
            Command::CreateDraft {
                account,
                // The core refuses control characters in names; a bidi override gets through.
                to: vec![Addr {
                    name: Some("Carol \u{202e}moc.evil".to_owned()),
                    email: "carol@example.org".into(),
                }],
                cc: Vec::new(),
                bcc: Vec::new(),
                // The core refuses control characters in the subject too.
                subject: "Invoice \u{202e}fdp.exe".to_owned(),
                body: format!("{NASTY}\nsecond line"),
            },
            &agent(901),
        )
        .unwrap()
        .receipt;
    let draft = serde_json::from_value(created.data["id"].clone()).unwrap();
    t.engine
        .dispatch(Command::SendDraft { id: draft }, &agent(902))
        .unwrap();
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("show send requests");
    t.key(KeyCode::Enter);
    t.pump();
    assert!(t.screen().contains("Send requests"));
    assert_clean(&mut t, "the approvals");
}
