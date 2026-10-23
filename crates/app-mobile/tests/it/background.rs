//! Leaving the app (Wave 4 A5): the host is told the background schedule, and a network that
//! comes back while the app is visible syncs; news becomes notifications.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, DraftId, SettingKey, ThreadId};
use app_mobile::notify::Channel;
use app_mobile::{External, NoteAct, Notification, UiEvent, notifications, push_status};

use crate::support::{Phone, inbox_of, requests_of, synced_state};

fn schedules(phone: &mut Phone) -> Vec<External> {
    phone
        .mobile
        .externals()
        .into_iter()
        .filter(|e| matches!(e, External::Schedule { .. }))
        .collect()
}

#[test]
fn the_schedule_comes_with_the_first_frame_and_each_change() {
    let mut phone = Phone::inbox(360, 800);
    let _ = phone.frame();
    assert_eq!(
        schedules(&mut phone),
        [External::Schedule {
            sync: 1,
            interval_mins: 30
        }]
    );
    let _ = phone.frame();
    assert!(schedules(&mut phone).is_empty(), "only when it changes");
    phone.dispatch(Command::SetSetting {
        key: SettingKey::BackgroundIntervalMins,
        value: 60.into(),
    });
    phone.dispatch(Command::SetSetting {
        key: SettingKey::BackgroundSync,
        value: 0.into(),
    });
    let _ = phone.frame();
    assert_eq!(
        schedules(&mut phone),
        [External::Schedule {
            sync: 0,
            interval_mins: 60
        }]
    );
}

#[test]
fn a_network_that_comes_back_syncs_only_while_visible() {
    let mut phone = Phone::inbox(360, 800);
    let _ = phone.frame();
    phone.event(UiEvent::Visible { visible: false });
    phone.event(UiEvent::Online);
    assert!(phone.take().is_empty(), "off screen: the jobs do it");
    phone.event(UiEvent::Visible { visible: true });
    phone.event(UiEvent::Online);
    assert_eq!(
        requests_of(&phone.take()),
        [Command::SyncNow { account: None }]
    );
}

/// A state where one unread message arrived in the inbox and a send failed, from `before`.
fn news() -> (app_core::AppState, app_core::AppState, ThreadId, DraftId) {
    let before = synced_state();
    let (inbox, _) = inbox_of(&before);
    let next = before.mailboxes[&inbox].uid_next.unwrap();
    let mut after = before.clone();
    let template = after
        .messages
        .values()
        .find(|m| m.mailbox == inbox)
        .unwrap()
        .clone();
    let id: app_core::MessageId =
        serde_json::from_str("\"00000000-0000-0000-0000-0000000000a1\"").unwrap();
    let mut message = app_core::Envelope {
        id,
        uid: next,
        subject: "Lunch\u{202e} today?".into(),
        ..template
    };
    message.flags.seen = false;
    let thread = message.thread;
    after.messages.insert(id, message);
    let draft = after.drafts.keys().next().copied();
    let draft = draft.unwrap_or_else(|| {
        let mut phone = Phone::new(before.clone(), 360, 800);
        let account = *before.accounts.keys().next().unwrap();
        phone.dispatch(Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Plans".into(),
            body: String::new(),
        });
        let made = phone.state().drafts.values().next().unwrap().clone();
        after.drafts.insert(made.id, made.clone());
        made.id
    });
    let mut failed = after.drafts[&draft].clone();
    failed.send_error = Some("550 mailbox full".into());
    after.drafts.insert(draft, failed);
    (before, after, thread, draft)
}

/// One notification of each channel the alert watcher posts, for the contract fixtures.
pub fn samples() -> Vec<(&'static str, Notification)> {
    let (before, after, _, _) = news();
    notifications(&before, &after)
        .into_iter()
        .map(|n| match n.channel {
            Channel::NewMail => ("new_mail", n),
            Channel::NotSent | Channel::Background => ("not_sent", n),
        })
        .chain(push_status(&before, &pushing(&before, 1)).map(|n| ("push", n)))
        .collect()
}

/// `state` with push on and `watching` accounts watching their INBOX.
fn pushing(state: &app_core::AppState, watching: usize) -> app_core::AppState {
    let mut state = state.clone();
    state.settings.background_sync = 2;
    for status in state.status.sync.values_mut() {
        status.watch = app_core::WatchState::Off;
    }
    for status in state.status.sync.values_mut().take(watching) {
        status.watch = app_core::WatchState::Watching;
    }
    state
}

#[test]
fn push_shows_what_it_watches_and_stops_as_the_user() {
    let state = synced_state();
    assert!(!state.status.sync.is_empty());
    assert!(push_status(&state, &state).is_none(), "push is off");
    let connecting = pushing(&state, 0);
    let note = push_status(&state, &connecting).unwrap();
    assert_eq!(note.channel, Channel::Background);
    assert_eq!(note.tag, "push");
    assert_eq!(note.title, "Connecting to your inboxes");
    let up = pushing(&state, 1);
    assert_eq!(
        push_status(&connecting, &up).unwrap().title,
        "Watching 1 inbox"
    );
    assert!(
        push_status(&up, &up).is_none(),
        "only when the text changes"
    );
    let stop = &note.actions[0];
    assert_eq!(stop.label, "Stop");
    assert_eq!(
        stop.act.commands(&up),
        [Command::SetSetting {
            key: SettingKey::BackgroundSync,
            value: 1.into()
        }]
    );
    assert_eq!(note.actions[1].act, NoteAct::BatterySettings);
    assert!(
        NoteAct::BatterySettings.commands(&up).is_empty(),
        "a host action"
    );
}

#[test]
fn news_becomes_notifications_without_names_on_the_lock_screen() {
    let (before, after, thread, draft) = news();
    let notes = notifications(&before, &after);
    let [mail, failed] = &notes[..] else {
        panic!("{notes:?}")
    };
    assert_eq!(mail.channel, Channel::NewMail);
    assert_eq!(mail.body, "Lunch today?", "cleaned");
    assert_eq!(mail.public_title, "New mail");
    assert!(!mail.public_title.contains("Lunch"));
    assert_eq!(mail.open, Some(NoteAct::OpenThread { thread }));
    let labels: Vec<_> = mail.actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, ["Archive", "Mark read"]);
    assert_eq!(failed.channel, Channel::NotSent);
    assert_eq!(failed.open, Some(NoteAct::OpenDraft { draft }));
    assert!(notifications(&after, &after).is_empty(), "only once");
    let mut quiet = after.clone();
    quiet.settings.notifications = 0;
    assert!(notifications(&before, &quiet).is_empty(), "turned off");
}

#[test]
fn a_notification_acts_as_the_user_on_what_still_exists() {
    let (_, after, thread, draft) = news();
    assert_eq!(
        NoteAct::Archive {
            threads: vec![thread]
        }
        .commands(&after),
        [Command::ArchiveThreads {
            ids: vec![thread],
            messages: Vec::new()
        }]
    );
    assert_eq!(
        NoteAct::MarkRead {
            threads: vec![thread]
        }
        .commands(&after),
        [Command::MarkRead {
            ids: vec![thread],
            messages: Vec::new()
        }]
    );
    let gone: ThreadId = serde_json::from_str("\"00000000-0000-0000-0000-0000000000ff\"").unwrap();
    assert!(
        NoteAct::Archive {
            threads: vec![gone]
        }
        .commands(&after)
        .is_empty()
    );
    assert!(
        NoteAct::OpenThread { thread: gone }
            .commands(&after)
            .is_empty()
    );
    // A tap opens the draft through the presenter.
    let mut phone = Phone::new(after, 360, 800);
    phone.event(UiEvent::Notification {
        act: NoteAct::OpenDraft { draft },
    });
    assert_eq!(
        requests_of(&phone.take()),
        [Command::OpenComposer { id: draft }]
    );
}
