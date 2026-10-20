//! Desktop notifications: what is news between two states, and what is not.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    Addr, AppState, Command, Ctx, DraftId, Engine, Envelope, Event, MessageId, Origin, SettingKey,
};
use app_front::alerts::{AlertKind, alerts};

use crate::support::{self, inbox_of, synced_state};

fn ctx(seed: u128, origin: Origin) -> Ctx {
    Ctx {
        now_ms: support::NOW,
        id_seed: seed << 64,
        origin,
    }
}

/// A copy of an INBOX message as a new one: `uid`, unread, from `name`.
fn arrived(state: &mut AppState, n: u128, uid: u32, name: &str, subject: &str) {
    let (inbox, _) = inbox_of(state);
    let template = state
        .messages
        .values()
        .find(|m| m.mailbox == inbox)
        .unwrap()
        .clone();
    let id: MessageId = serde_json::from_value(serde_json::json!(format!(
        "00000000-0000-0000-0000-{n:012}"
    )))
    .unwrap();
    let mut message = Envelope {
        id,
        uid,
        subject: subject.into(),
        from: vec![Addr {
            name: Some(name.into()),
            email: "x@example.org".into(),
        }],
        ..template
    };
    message.flags.seen = false;
    state.messages.insert(id, message);
}

fn uid_next(state: &AppState) -> u32 {
    let (inbox, _) = inbox_of(state);
    state.mailboxes[&inbox].uid_next.unwrap()
}

#[test]
fn new_unread_inbox_mail_is_news() {
    let before = synced_state();
    let next = uid_next(&before);
    let mut after = before.clone();
    arrived(&mut after, 1, next, "Dana <b>", "Lunch\u{202e} today?\n");
    let [alert] = &alerts(&before, &after)[..] else {
        panic!()
    };
    assert_eq!(alert.kind, AlertKind::NewMail);
    assert_eq!(alert.title, "Dana <b>", "escaping is the delivery's job");
    assert_eq!(alert.body, "Lunch today?", "controls and bidi overrides go");
    arrived(&mut after, 2, next + 1, "Eve", "Hello");
    arrived(&mut after, 3, next + 2, "Eve", "Again");
    let [alert] = &alerts(&before, &after)[..] else {
        panic!()
    };
    assert_eq!(alert.title, "3 new messages");
    assert_eq!(alert.body, "Dana <b>, Eve");
}

#[test]
fn first_syncs_backfills_read_mail_and_other_mailboxes_are_not() {
    let before = synced_state();
    let next = uid_next(&before);
    // Older mail (a history backfill).
    let mut after = before.clone();
    arrived(&mut after, 1, next - 1, "Old", "Old news");
    assert!(alerts(&before, &after).is_empty());
    // Read already (on another device).
    let mut after = before.clone();
    arrived(&mut after, 1, next, "Dana", "Seen");
    after.messages.values_mut().last().unwrap().flags.seen = true;
    assert!(alerts(&before, &after).is_empty());
    // A first sync: the INBOX had not been fetched.
    let mut empty = before.clone();
    let (inbox, _) = inbox_of(&empty);
    empty.mailboxes[&inbox].uid_next = None;
    let mut after = before.clone();
    arrived(&mut after, 1, next, "Dana", "Hi");
    assert!(alerts(&empty, &after).is_empty());
    // Another mailbox.
    let other = *before.mailboxes.keys().find(|id| **id != inbox).unwrap();
    let mut after = before.clone();
    arrived(&mut after, 1, next, "Dana", "Hi");
    after.messages.values_mut().last().unwrap().mailbox = other;
    assert!(alerts(&before, &after).is_empty());
    // Turned off.
    let mut after = before.clone();
    arrived(&mut after, 1, next, "Dana", "Hi");
    SettingKey::Notifications.set(&mut after.settings, 0);
    assert!(alerts(&before, &after).is_empty());
}

#[test]
fn failed_sends_and_agent_requests_are_news() {
    let state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let mut engine = Engine::new(state);
    let create = |engine: &mut Engine, seed, origin| -> DraftId {
        let receipt = engine
            .dispatch(
                Command::CreateDraft {
                    account,
                    to: vec![Addr {
                        name: Some("Bob".into()),
                        email: "bob@example.org".into(),
                    }],
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: "Plans".into(),
                    body: String::new(),
                },
                &ctx(seed, origin),
            )
            .unwrap()
            .receipt;
        serde_json::from_value(receipt.data["id"].clone()).unwrap()
    };
    let draft = create(&mut engine, 1, Origin::Cli);
    let before = engine.state().clone();
    engine
        .ingest(
            Event::SendFailed {
                draft,
                error: "550 mailbox full".into(),
            },
            &ctx(2, Origin::Runtime),
        )
        .unwrap();
    let after = engine.state().clone();
    let [alert] = &alerts(&before, &after)[..] else {
        panic!("{:?}", alerts(&before, &after))
    };
    assert_eq!(alert.kind, AlertKind::SendFailed);
    assert_eq!(alert.body, "Plans: 550 mailbox full");
    assert!(alerts(&after, &after).is_empty(), "only once");

    let asked = create(&mut engine, 3, Origin::Mcp);
    let before = engine.state().clone();
    engine
        .dispatch(Command::SendDraft { id: asked }, &ctx(4, Origin::Mcp))
        .unwrap();
    let after = engine.state().clone();
    let [alert] = &alerts(&before, &after)[..] else {
        panic!("{:?}", alerts(&before, &after))
    };
    assert_eq!(alert.kind, AlertKind::ApprovalNeeded);
    assert_eq!(alert.body, "To Bob: Plans");
}
