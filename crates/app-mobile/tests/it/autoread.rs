//! Mark as read after a delay on the phone (MISC-47): the presenter runs the desktop's timer
//! (`app_front::autoread`) on its tick and submits one `mark_read` as the user's.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, MessageId, ThreadId};
use app_mobile::UiEvent;

use crate::support::{Phone, advance, inbox_of, requests_of};

/// The inbox's threads with unread messages.
fn unread_threads(phone: &Phone) -> Vec<ThreadId> {
    let state = phone.state();
    let (_, threads) = inbox_of(state);
    threads
        .into_iter()
        .filter(|t| {
            state.threads[t]
                .messages
                .iter()
                .any(|m| !state.messages[m].flags.seen)
        })
        .collect()
}

fn unread_of(phone: &Phone, thread: ThreadId) -> Vec<MessageId> {
    let state = phone.state();
    state.threads[&thread]
        .messages
        .iter()
        .filter(|m| !state.messages[*m].flags.seen)
        .copied()
        .collect()
}

/// A phone with `thread` open, its first frame drawn and nothing submitted yet.
fn open(phone: &mut Phone, thread: ThreadId) {
    phone.dispatch(Command::OpenThread { id: thread });
    phone.frame();
    phone.take();
}

/// Ticks once `ms` later; returns the `mark_read`s submitted.
fn tick_after(phone: &mut Phone, ms: i64) -> Vec<Command> {
    advance(ms);
    phone.event(UiEvent::Tick);
    requests_of(&phone.take())
        .into_iter()
        .filter(|c| matches!(c, Command::MarkRead { .. }))
        .collect()
}

#[test]
fn an_open_message_is_marked_read_once_the_delay_has_passed() {
    let mut phone = Phone::inbox(360, 800);
    let thread = unread_threads(&phone)[0];
    let unread = unread_of(&phone, thread);
    open(&mut phone, thread);
    assert!(phone.frame().wants_tick);
    let delay = i64::try_from(phone.state().settings.mark_read_after_ms).unwrap();
    assert!(
        tick_after(&mut phone, delay - 1).is_empty(),
        "before the delay"
    );
    assert_eq!(
        tick_after(&mut phone, 1),
        vec![Command::MarkRead {
            ids: Vec::new(),
            messages: unread,
        }]
    );
    assert!(unread_of(&phone, thread).is_empty());
    // No repeat, even after the user marks it unread again.
    assert!(tick_after(&mut phone, delay).is_empty());
    phone.dispatch(Command::MarkUnread {
        ids: vec![thread],
        messages: Vec::new(),
    });
    assert!(tick_after(&mut phone, delay * 2).is_empty());
}

#[test]
fn nothing_is_marked_read_when_the_setting_is_off() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::SetSetting {
        key: app_core::SettingKey::MarkReadAfterMs,
        value: 0u64.into(),
    });
    let thread = unread_threads(&phone)[0];
    open(&mut phone, thread);
    assert!(tick_after(&mut phone, 60_000).is_empty());
}

#[test]
fn switching_message_before_the_delay_restarts_it() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    let (first, second) = (threads[0], threads[1]);
    phone.dispatch(Command::MarkUnread {
        ids: vec![first, second],
        messages: Vec::new(),
    });
    let unread = unread_of(&phone, second);
    let delay = i64::try_from(phone.state().settings.mark_read_after_ms).unwrap();
    open(&mut phone, first);
    assert!(tick_after(&mut phone, delay - 100).is_empty());
    open(&mut phone, second);
    assert!(tick_after(&mut phone, delay - 100).is_empty(), "restarted");
    assert_eq!(
        tick_after(&mut phone, 100),
        vec![Command::MarkRead {
            ids: Vec::new(),
            messages: unread,
        }]
    );
    assert!(
        !unread_of(&phone, first).is_empty(),
        "the first stays unread"
    );
}

#[test]
fn an_already_read_message_is_left_alone() {
    let mut phone = Phone::inbox(360, 800);
    let thread = unread_threads(&phone)[0];
    phone.dispatch(Command::MarkRead {
        ids: vec![thread],
        messages: Vec::new(),
    });
    open(&mut phone, thread);
    assert!(tick_after(&mut phone, 60_000).is_empty());
}

#[test]
fn time_in_the_background_does_not_count() {
    let mut phone = Phone::inbox(360, 800);
    let thread = unread_threads(&phone)[0];
    let delay = i64::try_from(phone.state().settings.mark_read_after_ms).unwrap();
    open(&mut phone, thread);
    assert!(tick_after(&mut phone, delay / 2).is_empty());
    phone.event(UiEvent::Visible { visible: false });
    // No tick while stopped, as on a real phone: the gap must not count on the way back.
    advance(60_000);
    phone.event(UiEvent::Visible { visible: true });
    phone.frame();
    let fired: Vec<Command> = requests_of(&phone.take())
        .into_iter()
        .filter(|c| matches!(c, Command::MarkRead { .. }))
        .collect();
    assert!(fired.is_empty(), "{fired:?}");
    assert!(tick_after(&mut phone, delay / 2 - 1).is_empty());
    assert_eq!(tick_after(&mut phone, 1).len(), 1);
}

fn open_composer(phone: &mut Phone) {
    let account = *phone.state().accounts.keys().next().unwrap();
    phone.dispatch(Command::CreateDraft {
        account,
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: "Plans".into(),
        body: String::new(),
    });
    let id = *phone.state().drafts.keys().next().unwrap();
    phone.dispatch(Command::OpenComposer { id });
}

#[test]
fn a_phone_composing_does_not_mark_the_hidden_message_read() {
    let mut phone = Phone::inbox(360, 800);
    let thread = unread_threads(&phone)[0];
    open(&mut phone, thread);
    open_composer(&mut phone);
    assert!(phone.frame().reader.is_none());
    assert_eq!(phone.state().view.thread, Some(thread));
    assert!(tick_after(&mut phone, 60_000).is_empty());
}

#[test]
fn a_phone_with_the_drawer_open_does_not_mark_read() {
    let mut phone = Phone::inbox(360, 800);
    let thread = unread_threads(&phone)[0];
    open(&mut phone, thread);
    phone.dispatch(Command::FocusPane {
        pane: app_core::Pane::Sidebar,
    });
    assert!(tick_after(&mut phone, 60_000).is_empty());
}

#[test]
fn a_tablet_beside_the_list_marks_read() {
    let mut phone = Phone::inbox(700, 1000);
    let thread = unread_threads(&phone)[0];
    open(&mut phone, thread);
    assert!(phone.frame().layout.panes.len() >= 2);
    let delay = i64::try_from(phone.state().settings.mark_read_after_ms).unwrap();
    assert_eq!(tick_after(&mut phone, delay).len(), 1);
}
