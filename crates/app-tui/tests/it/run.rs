//! The runner's event loop over a real session (memory backend), on a `TestBackend`: quitting,

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]
//! hang-ups and signals end it, and closing the bridge shuts an owned session down.

use std::sync::{Arc, mpsc};
use std::time::Duration;

use app_core::{Command, Origin};
use app_sync::{Bridge, SyncError};
use app_testkit::Fixture;
use app_tui::{EditOutcome, Event, Exit, Host, Input, Purpose, Tui, event_loop};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::support::fixed_now;

/// A terminal without an editor.
struct NoEditor;

impl Host for NoEditor {
    fn edit(&mut self, _text: &str) -> EditOutcome {
        EditOutcome::NoEditor
    }

    fn mouse(&mut self, _on: bool) {}
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

fn key(c: char) -> Event {
    Event::Input(Input::Key(KeyEvent::new(
        KeyCode::Char(c),
        KeyModifiers::NONE,
    )))
}

/// Runs the loop over an owned, synced session with `events` queued, then closes the bridge.
/// Returns how it ended and whether the session was shut down afterwards.
fn run_with(events: Vec<Event>) -> (Exit, bool) {
    let rt = runtime();
    let synced = rt
        .block_on(app_testkit::synced_session(
            Fixture::builtin("basic").unwrap(),
        ))
        .unwrap();
    let client = synced.session.client();
    let (tx, rx) = mpsc::channel();
    let wake_tx = tx.clone();
    let (mut bridge, task) = Bridge::<Purpose>::own(
        rt.handle(),
        synced.session,
        Origin::Tui,
        Arc::new(move || {
            let _ = wake_tx.send(Event::Wake);
        }),
    );
    for event in events {
        tx.send(event).unwrap();
    }
    let mut terminal = Terminal::new(TestBackend::new(50, 20)).unwrap();
    let mut tui = Tui::with_clock(fixed_now);
    let exit = event_loop(&mut terminal, &mut tui, &bridge, &rx, &mut NoEditor).unwrap();
    bridge.close();
    drop(bridge);
    rt.block_on(async { tokio::time::timeout(Duration::from_secs(5), task).await })
        .unwrap()
        .unwrap();
    let closed = matches!(
        rt.block_on(client.submit(Command::GetView {}, Origin::Cli)),
        Err(SyncError::SessionClosed)
    );
    (exit, closed)
}

#[test]
fn quitting_ends_the_loop_and_shuts_the_owned_session_down() {
    let (exit, closed) = run_with(vec![Event::Wake, Event::Wake, key('q')]);
    assert_eq!(exit, Exit::Quit);
    assert!(closed, "the owned session was shut down");
}

#[test]
fn a_hangup_or_a_signal_ends_the_loop_like_quitting() {
    let (exit, closed) = run_with(vec![Event::TerminalGone]);
    assert_eq!(exit, Exit::Hangup("the terminal went away".into()));
    assert!(closed);
    let (exit, closed) = run_with(vec![key('j'), Event::Signal("SIGHUP")]);
    assert_eq!(exit, Exit::Hangup("received SIGHUP".into()));
    assert!(closed);
}
