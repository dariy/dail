//! View commands (`open_mailbox`, `select_threads`, `open_thread`, `close_thread`) and the
//! `get_thread` / `get_message` queries.

use crate::support::{Mail, ctx, mail, names};
use app_core::{
    BodyState, Command, CommandKind, CoreError, Engine, EntityRef, Event, MailboxId, MessageId,
    ThreadId,
};
use serde_json::json;

/// INBOX with a two-message thread (UIDs 1, 2) and a single message (UID 3).
fn inbox() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "Plans", true),
            mail(2, "b@x", Some("a@x"), "Re: Plans", false),
            mail(3, "c@x", None, "Lunch", false),
        ],
    );
    m
}

fn missing_thread() -> ThreadId {
    ThreadId::from_uuid(uuid::Uuid::from_u128(99))
}

#[test]
fn open_mailbox_sets_the_view_and_clears_selection() {
    let mut m = inbox();
    let thread = m.thread_of(m.message("INBOX", 1));
    m.run(Command::SelectThreads { ids: vec![thread] }).unwrap();
    m.run(Command::OpenThread { id: thread }).unwrap();
    let archive = m.mailbox("Archive");
    let outcome = m.run(Command::OpenMailbox { id: archive }).unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::View));
    assert!(outcome.receipt.affected.contains(&EntityRef::View));
    assert!(outcome.effects.is_empty());
    let view = &m.engine.state().view;
    assert_eq!(view.mailbox, Some(archive));
    assert!(view.selected.is_empty());
    assert_eq!(view.thread, None);
    assert_eq!(m.engine.history().undo_len(), 1, "only add_account");
}

#[test]
fn open_mailbox_rejects_unknown_ids() {
    let mut m = inbox();
    let err = m
        .run(Command::OpenMailbox {
            id: MailboxId::from_uuid(uuid::Uuid::from_u128(98)),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "mailbox"));
    assert_eq!(m.engine.state().view.mailbox, None);
}

#[test]
fn select_threads_replaces_and_dedupes_the_selection() {
    let mut m = inbox();
    let a = m.thread_of(m.message("INBOX", 1));
    let b = m.thread_of(m.message("INBOX", 3));
    let outcome = m
        .run(Command::SelectThreads { ids: vec![b, a, b] })
        .unwrap();
    assert_eq!(outcome.receipt.data, json!({ "selected": [b, a] }));
    assert_eq!(m.engine.state().view.selected, vec![b, a]);
    m.run(Command::SelectThreads { ids: vec![] }).unwrap();
    assert!(m.engine.state().view.selected.is_empty());
}

#[test]
fn select_threads_rejects_unknown_ids_atomically() {
    let mut m = inbox();
    let a = m.thread_of(m.message("INBOX", 1));
    m.run(Command::SelectThreads { ids: vec![a] }).unwrap();
    let err = m
        .run(Command::SelectThreads {
            ids: vec![a, missing_thread()],
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "thread"));
    assert_eq!(m.engine.state().view.selected, vec![a]);
}

#[test]
fn open_thread_downloads_missing_bodies_once() {
    let mut m = inbox();
    let first = m.message("INBOX", 1);
    let thread = m.thread_of(first);
    // The unread reply was prefetched by the INBOX fetch; only the read message is missing.
    let reply = m.message("INBOX", 2);
    assert!(m.engine.state().status.bodies_in_flight.contains(&reply));
    let outcome = m.run(Command::OpenThread { id: thread }).unwrap();
    assert_eq!(names(&outcome.effects), ["fetch_body"]);
    assert_eq!(outcome.receipt.data["fetching_bodies"], json!([first]));
    let view = &m.engine.state().view;
    assert_eq!(view.thread, Some(thread));
    assert_eq!(view.selected, vec![thread]);
    // Opening again does not request the same bodies twice.
    let again = m.run(Command::OpenThread { id: thread }).unwrap();
    assert!(again.effects.is_empty());
}

#[test]
fn open_thread_keeps_a_selection_that_contains_it() {
    let mut m = inbox();
    let a = m.thread_of(m.message("INBOX", 1));
    let b = m.thread_of(m.message("INBOX", 3));
    m.run(Command::SelectThreads { ids: vec![a, b] }).unwrap();
    m.run(Command::OpenThread { id: b }).unwrap();
    assert_eq!(m.engine.state().view.selected, vec![a, b]);
}

#[test]
fn open_thread_retries_failed_bodies_and_rejects_unknown_threads() {
    let mut m = inbox();
    let first = m.message("INBOX", 1);
    m.feed(Event::BodyFetchFailed {
        message: first,
        error: "connection reset".into(),
    });
    let thread = m.thread_of(first);
    let outcome = m.run(Command::OpenThread { id: thread }).unwrap();
    assert_eq!(outcome.receipt.data["fetching_bodies"], json!([first]));
    let err = m
        .run(Command::OpenThread {
            id: missing_thread(),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "thread"));
}

#[test]
fn close_thread_clears_the_reader_only() {
    let mut m = inbox();
    let thread = m.thread_of(m.message("INBOX", 1));
    m.run(Command::OpenThread { id: thread }).unwrap();
    m.run(Command::CloseThread {}).unwrap();
    let view = &m.engine.state().view;
    assert_eq!(view.thread, None);
    assert_eq!(view.selected, vec![thread]);
    assert!(matches!(
        m.run(Command::Undo {}),
        Ok(outcome) if outcome.receipt.data["undone"] == "Add account me@example.com"
    ));
}

#[test]
fn view_references_are_pruned_when_threads_vanish() {
    let mut m = inbox();
    let message = m.message("INBOX", 3);
    let thread = m.thread_of(message);
    m.run(Command::OpenThread { id: thread }).unwrap();
    let inbox = m.mailbox("INBOX");
    m.feed(Event::FlagsChanged {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        from_uid: 3,
        to_uid: 3,
        flags: vec![],
        existing: None,
        highest_modseq: None,
    });
    let view = &m.engine.state().view;
    assert_eq!(view.thread, None);
    assert!(view.selected.is_empty());
}

#[test]
fn get_thread_returns_messages_as_untrusted_content() {
    let mut m = inbox();
    let first = m.message("INBOX", 1);
    let thread = m.thread_of(first);
    let before = m.engine.state().clone();
    let outcome = m
        .run(Command::GetThread {
            id: thread,
            include_body: false,
            rich_body: false,
        })
        .unwrap();
    assert_eq!(*m.engine.state(), before, "queries do not change state");
    assert!(outcome.effects.is_empty());
    let data = &outcome.receipt.data;
    assert_eq!(data["untrusted_content"]["subject"], "Plans");
    let messages = data["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["id"], json!(first));
    assert_eq!(messages[0]["flags"]["seen"], true);
    assert_eq!(messages[0]["body"], json!({ "state": "not_fetched" }));
    assert_eq!(messages[1]["downloading"], true);
    assert_eq!(
        messages[1]["untrusted_content"]["from"],
        json!([{ "name": "Bob", "email": "bob@example.org" }])
    );
    assert_eq!(messages[1]["untrusted_content"]["subject"], "Re: Plans");
    assert!(
        messages[1].get("subject").is_none(),
        "no content outside the envelope"
    );
}

#[test]
fn include_body_references_cached_blobs_and_downloads_missing_ones() {
    let mut m = inbox();
    let first = m.message("INBOX", 1);
    let reply = m.message("INBOX", 2);
    m.feed(Event::BodyFetched {
        message: reply,
        blob: "ab".repeat(32),
        has_attachments: false,
    });
    let thread = m.thread_of(first);
    let before = m.engine.state().clone();
    let outcome = m
        .run(Command::GetThread {
            id: thread,
            include_body: true,
            rich_body: false,
        })
        .unwrap();
    assert_eq!(
        *m.engine.state(),
        before,
        "in-flight bodies are not recorded"
    );
    assert_eq!(names(&outcome.effects), ["fetch_body"]);
    let messages = outcome.receipt.data["messages"].as_array().unwrap();
    assert_eq!(messages[0]["body"], json!({ "state": "not_fetched" }));
    assert_eq!(
        messages[1]["body"],
        json!({ "state": "fetched", "blob": "ab".repeat(32) })
    );
    // Without `include_body` the blob reference is not exposed.
    let plain = m
        .run(Command::GetMessage {
            id: reply,
            include_body: false,
        })
        .unwrap();
    assert_eq!(
        plain.receipt.data["message"]["body"],
        json!({ "state": "fetched" })
    );
    assert!(matches!(
        m.engine.state().messages[&reply].body,
        BodyState::Fetched { .. }
    ));
}

#[test]
fn get_message_downloads_unless_in_flight() {
    let mut m = inbox();
    let first = m.message("INBOX", 1);
    let reply = m.message("INBOX", 2);
    let outcome = m
        .run(Command::GetMessage {
            id: first,
            include_body: true,
        })
        .unwrap();
    assert_eq!(names(&outcome.effects), ["fetch_body"]);
    assert_eq!(outcome.receipt.affected, vec![EntityRef::Message(first)]);
    let in_flight = m
        .run(Command::GetMessage {
            id: reply,
            include_body: true,
        })
        .unwrap();
    assert!(
        in_flight.effects.is_empty(),
        "the prefetch is still running"
    );
}

#[test]
fn get_queries_reject_unknown_ids() {
    let mut m = inbox();
    let err = m
        .run(Command::GetThread {
            id: missing_thread(),
            include_body: false,
            rich_body: false,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "thread"));
    let err = m
        .run(Command::GetMessage {
            id: MessageId::from_uuid(uuid::Uuid::from_u128(97)),
            include_body: true,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "message"));
}

#[test]
fn visible_threads_follow_the_open_mailbox_and_the_search() {
    let mut m = inbox();
    assert!(app_core::listing::visible_threads(m.engine.state()).is_empty());
    let plans = m.thread_of(m.message("INBOX", 1));
    let lunch = m.thread_of(m.message("INBOX", 3));
    m.run(Command::OpenMailbox {
        id: m.mailbox("INBOX"),
    })
    .unwrap();
    // Newest first: UID 3 is the newest message.
    assert_eq!(
        app_core::listing::visible_threads(m.engine.state()),
        [lunch, plans]
    );
    m.run(Command::SetSearch {
        query: "subject:plans".into(),
    })
    .unwrap();
    assert_eq!(
        app_core::listing::visible_threads(m.engine.state()),
        [plans]
    );
    m.run(Command::ClearSearch {}).unwrap();
    assert_eq!(
        app_core::listing::visible_threads(m.engine.state()).len(),
        2
    );
}

#[test]
fn the_settings_page_opens_at_a_section_and_closes() {
    use app_core::SettingsSection;
    let mut engine = Engine::default();
    engine
        .dispatch(Command::OpenSettings { section: None }, &ctx(1))
        .unwrap();
    assert_eq!(
        engine.state().view.settings,
        Some(SettingsSection::Appearance)
    );
    engine
        .dispatch(
            Command::OpenSettings {
                section: Some(SettingsSection::Keys),
            },
            &ctx(2),
        )
        .unwrap();
    // Without a section the open one stays.
    engine
        .dispatch(Command::OpenSettings { section: None }, &ctx(3))
        .unwrap();
    assert_eq!(engine.state().view.settings, Some(SettingsSection::Keys));
    let view = engine.dispatch(Command::GetView {}, &ctx(4)).unwrap();
    assert_eq!(view.receipt.data["settings"], "keys");
    let outcome = engine.dispatch(Command::CloseSettings {}, &ctx(5)).unwrap();
    assert!(outcome.effects.is_empty(), "view only");
    assert_eq!(engine.state().view.settings, None);
    // View commands are not undo steps.
    assert!(engine.dispatch(Command::Undo {}, &ctx(6)).is_err());
}
