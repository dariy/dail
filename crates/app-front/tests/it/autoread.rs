//! Mark as read after a delay (UX-5), with a fake clock.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, ThreadId};
use app_front::autoread::{AutoRead, Poll};

use crate::support::{NOW, inbox_of, synced_state};

/// The state with an unread thread open.
fn open_unread() -> (AppState, ThreadId) {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    let thread = *threads
        .iter()
        .find(|t| {
            state.threads[*t]
                .messages
                .iter()
                .any(|m| !state.messages[m].flags.seen)
        })
        .unwrap();
    state.view.thread = Some(thread);
    (state, thread)
}

fn fired(poll: Poll) -> Command {
    match poll {
        Poll::Fire(cmd) => cmd,
        other => panic!("{other:?}"),
    }
}

#[test]
fn fires_once_after_the_delay() {
    let (state, thread) = open_unread();
    let mut timer = AutoRead::default();
    assert_eq!(timer.poll(&state, NOW, true), Poll::Wait(2_000));
    assert_eq!(timer.poll(&state, NOW + 1_500, true), Poll::Wait(500));
    let Command::MarkRead { ids, messages } = fired(timer.poll(&state, NOW + 2_000, true)) else {
        panic!()
    };
    assert!(ids.is_empty());
    assert!(!messages.is_empty());
    assert!(messages.iter().all(|m| state.messages[m].thread == thread));
    assert!(messages.iter().all(|m| !state.messages[m].flags.seen));
    assert_eq!(timer.poll(&state, NOW + 9_000, true), Poll::Idle, "once");
}

#[test]
fn leaving_the_thread_cancels_and_reopening_restarts() {
    let (mut state, thread) = open_unread();
    let mut timer = AutoRead::default();
    timer.poll(&state, NOW, true);
    state.view.thread = None;
    assert_eq!(timer.poll(&state, NOW + 1_000, true), Poll::Idle);
    state.view.thread = Some(thread);
    assert_eq!(timer.poll(&state, NOW + 1_500, true), Poll::Wait(2_000));
}

#[test]
fn mark_unread_disarms_it_until_another_thread_opens() {
    let (state, _) = open_unread();
    let mut timer = AutoRead::default();
    timer.poll(&state, NOW, true);
    timer.note(&Command::MarkUnread {
        ids: Vec::new(),
        messages: Vec::new(),
    });
    assert_eq!(timer.poll(&state, NOW + 5_000, true), Poll::Idle);
}

#[test]
fn nothing_when_off_read_or_in_the_background() {
    let (mut state, thread) = open_unread();
    let mut timer = AutoRead::default();
    // The background pauses the count.
    timer.poll(&state, NOW, true);
    assert_eq!(timer.poll(&state, NOW + 1_000, true), Poll::Wait(1_000));
    assert_eq!(timer.poll(&state, NOW + 60_000, false), Poll::Idle);
    assert_eq!(timer.poll(&state, NOW + 60_500, true), Poll::Wait(500));
    // Off.
    state.settings.mark_read_after_ms = 0;
    assert_eq!(timer.poll(&state, NOW + 90_000, true), Poll::Idle);
    // Nothing unread.
    state.settings.mark_read_after_ms = 250;
    let messages = state.threads[&thread].messages.clone();
    for m in messages {
        state.messages.get_mut(&m).unwrap().flags.seen = true;
    }
    assert_eq!(timer.poll(&state, NOW + 99_000, true), Poll::Idle);
}
