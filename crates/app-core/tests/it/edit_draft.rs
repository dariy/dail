//! `edit_draft` and the drafts of a conversation (MISC-44): a server-only draft becomes a local
//! draft once its text is read, `get_thread` marks drafts, `discard_draft` drops the cached copy,
//! and a thread's Star and Reply skip drafts.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, names};
use app_core::draft::{AttachmentState, CarriedAttachment, MAX_ATTACHMENT_BYTES};
use app_core::{
    Addr, Command, CoreError, DraftId, Event, MailboxRole, MessageId, ProviderPreset,
    RemoteEnvelope,
};

fn addr(email: &str) -> Addr {
    Addr {
        name: None,
        email: email.into(),
    }
}

/// A draft envelope in Drafts answering `q3@example.org`, written by me to Bob.
fn server_draft(uid: u32, id: &str) -> RemoteEnvelope {
    let mut draft = mail(uid, id, Some("q3@example.org"), "Re: Q3 numbers", true);
    draft.from = vec![addr("me@example.com")];
    draft.to = vec![addr("bob@example.org"), addr("not an address")];
    draft.flags.draft = true;
    draft
}

/// INBOX with Bob's message and Drafts with my unsent answer to it (a server-only draft).
fn conversation() -> Mail {
    let mut m = Mail::new(
        ProviderPreset::Custom,
        &[
            ("INBOX", None),
            ("Drafts", Some(MailboxRole::Drafts)),
            ("Trash", Some(MailboxRole::Trash)),
        ],
    );
    m.fetch(
        "INBOX",
        vec![mail(1, "q3@example.org", None, "Q3 numbers", false)],
    );
    m.fetch("Drafts", vec![server_draft(1, "d1@example.com")]);
    m
}

fn draft_of(outcome: &app_core::Outcome) -> DraftId {
    serde_json::from_value(outcome.receipt.data["draft"]["id"].clone()).unwrap()
}

fn body_fetched(message: MessageId) -> Event {
    Event::BodyFetched {
        message,
        blob: "b1".into(),
        has_attachments: false,
        snippet: None,
    }
}

/// Edits the server-only draft and delivers its text; returns the draft and the message.
fn edited(m: &mut Mail) -> (DraftId, MessageId) {
    let message = m.message("Drafts", 1);
    let id = draft_of(&m.run(Command::EditDraft { message }).unwrap());
    m.feed(body_fetched(message));
    m.feed(Event::DraftRead {
        draft: id,
        message,
        body: "Numbers attached.".into(),
        bcc: vec![addr("boss@example.org")],
        attachments: Vec::new(),
    });
    (id, message)
}

#[test]
fn a_server_only_draft_becomes_local_once_its_text_is_read() {
    let mut m = conversation();
    let message = m.message("Drafts", 1);
    let outcome = m.run(Command::EditDraft { message }).unwrap();
    assert_eq!(outcome.receipt.data["created"], true);
    assert_eq!(names(&outcome.effects), ["persist_state", "fetch_body"]);
    let id = draft_of(&outcome);
    let draft = m.engine.state().drafts[&id].clone();
    assert_eq!(draft.loading_from, Some(message));
    assert_eq!(draft.message_id_hdr, "d1@example.com");
    assert_eq!(draft.in_reply_to.as_deref(), Some("q3@example.org"));
    assert_eq!(
        draft.to,
        vec![addr("bob@example.org")],
        "bad addresses are left out"
    );
    assert_eq!(draft.from, None, "my own address");
    assert!(!draft.needs_save());

    // Until the text arrives the draft can't be changed.
    let err = m
        .run(Command::UpdateDraft {
            id,
            to: None,
            cc: None,
            bcc: None,
            subject: Some("x".into()),
            body: None,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotSendable { .. }), "{err}");

    let fetched = m.feed(body_fetched(message));
    assert!(names(&fetched.effects).contains(&"read_draft"));
    let read = m.feed(Event::DraftRead {
        draft: id,
        message,
        body: "Numbers attached.".into(),
        bcc: vec![addr("boss@example.org")],
        attachments: Vec::new(),
    });
    assert_eq!(names(&read.effects), ["persist_state"], "nothing to save");
    let draft = &m.engine.state().drafts[&id];
    assert_eq!(draft.body_text, "Numbers attached.");
    assert_eq!(draft.bcc, vec![addr("boss@example.org")]);
    assert_eq!(draft.loading_from, None);
    assert!(!draft.needs_save());
}

#[test]
fn a_cached_body_is_read_at_once() {
    let mut m = conversation();
    let message = m.message("Drafts", 1);
    m.feed(body_fetched(message));
    let outcome = m.run(Command::EditDraft { message }).unwrap();
    assert_eq!(names(&outcome.effects), ["persist_state", "read_draft"]);
}

#[test]
fn a_message_with_a_local_draft_returns_that_draft() {
    let mut m = conversation();
    let (id, message) = edited(&mut m);
    let undo_before = m.engine.history().undo_len();
    let outcome = m.run(Command::EditDraft { message }).unwrap();
    assert_eq!(outcome.receipt.data["created"], false);
    assert_eq!(draft_of(&outcome), id);
    assert!(outcome.effects.is_empty());
    assert_eq!(
        m.engine.history().undo_len(),
        undo_before,
        "nothing to undo"
    );
    assert_eq!(m.engine.state().drafts.len(), 1);
}

#[test]
fn get_thread_marks_drafts_and_their_local_draft() {
    let mut m = conversation();
    let inbox = m.message("INBOX", 1);
    let thread = m.thread_of(inbox);
    let (id, message) = edited(&mut m);
    let outcome = m
        .run(Command::GetThread {
            id: thread,
            include_body: false,
            rich_body: false,
        })
        .unwrap();
    let messages = outcome.receipt.data["messages"].as_array().unwrap().clone();
    let find = |wanted: MessageId| {
        messages
            .iter()
            .find(|m| m["id"] == serde_json::json!(wanted))
            .unwrap()
            .clone()
    };
    assert_eq!(find(inbox)["is_draft"], false);
    assert_eq!(find(inbox)["draft"], serde_json::Value::Null);
    assert_eq!(find(message)["is_draft"], true);
    assert_eq!(find(message)["draft"], serde_json::json!(id));
}

#[test]
fn errors() {
    let mut m = conversation();
    let unknown = MessageId::from_uuid(uuid::Uuid::from_u128(99));
    let err = m.run(Command::EditDraft { message: unknown }).unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }), "{err}");

    let inbox = m.message("INBOX", 1);
    let err = m.run(Command::EditDraft { message: inbox }).unwrap_err();
    assert!(
        matches!(&err, CoreError::InvalidArgument { field, reason } if field == "message" && reason.contains("not a draft")),
        "{err}"
    );

    let mut no_id = server_draft(2, "unused");
    no_id.message_id_hdr = None;
    m.fetch("Drafts", vec![no_id]);
    let err = m
        .run(Command::EditDraft {
            message: m.message("Drafts", 2),
        })
        .unwrap_err();
    assert!(
        matches!(&err, CoreError::InvalidArgument { reason, .. } if reason.contains("Message-ID")),
        "{err}"
    );

    let draft = m.message("Drafts", 1);
    m.run(Command::TrashThreads {
        ids: Vec::new(),
        messages: vec![draft],
    })
    .unwrap();
    let err = m.run(Command::EditDraft { message: draft }).unwrap_err();
    assert!(
        matches!(&err, CoreError::InvalidArgument { reason, .. } if reason.contains("moved")),
        "{err}"
    );
    assert!(
        m.engine.state().drafts.is_empty(),
        "a rejected input changes nothing"
    );
}

#[test]
fn undo_keeps_the_server_copy_and_redo_makes_the_draft_again() {
    let mut m = conversation();
    let (id, message) = edited(&mut m);
    let undo = m.run(Command::Undo {}).unwrap();
    assert!(!names(&undo.effects).contains(&"delete_draft"));
    assert!(m.engine.state().drafts.is_empty());
    assert!(
        m.engine.state().messages.contains_key(&message),
        "server copy stays"
    );
    let redo = m.run(Command::Redo {}).unwrap();
    assert!(
        names(&redo.effects).contains(&"fetch_body")
            || names(&redo.effects).contains(&"read_draft")
    );
    assert_eq!(m.engine.state().drafts[&id].loading_from, Some(message));
}

#[test]
fn discard_draft_drops_the_cached_server_copy_at_once() {
    let mut m = conversation();
    let (id, message) = edited(&mut m);
    let thread = m.thread_of(message);
    let outcome = m
        .run(Command::DiscardDraft {
            id,
            keep_server_copy: false,
        })
        .unwrap();
    assert!(names(&outcome.effects).contains(&"delete_draft"));
    assert!(!m.engine.state().messages.contains_key(&message));
    assert!(
        !m.engine.state().threads[&thread]
            .messages
            .contains(&message)
    );
    m.run(Command::Undo {}).unwrap();
    assert!(
        m.engine.state().drafts.contains_key(&id),
        "undo restores the draft"
    );
}

#[test]
fn keep_server_copy_deletes_only_the_local_draft() {
    let mut m = conversation();
    let (id, message) = edited(&mut m);
    let outcome = m
        .run(Command::DiscardDraft {
            id,
            keep_server_copy: true,
        })
        .unwrap();
    assert!(!names(&outcome.effects).contains(&"delete_draft"));
    assert!(m.engine.state().messages.contains_key(&message));
}

#[test]
fn a_failed_read_or_download_drops_the_draft_and_keeps_the_server_copy() {
    let mut m = conversation();
    let message = m.message("Drafts", 1);
    let id = draft_of(&m.run(Command::EditDraft { message }).unwrap());
    let failed = m.feed(Event::BodyFetchFailed {
        message,
        error: "gone".into(),
    });
    assert!(!names(&failed.effects).contains(&"delete_draft"));
    assert!(!m.engine.state().drafts.contains_key(&id));
    assert!(m.engine.state().messages.contains_key(&message));

    let id = draft_of(&m.run(Command::EditDraft { message }).unwrap());
    m.feed(body_fetched(message));
    let failed = m.feed(Event::DraftReadFailed {
        draft: id,
        message,
        error: "unreadable".into(),
    });
    assert_eq!(failed.receipt.data["error"], "unreadable");
    assert!(!m.engine.state().drafts.contains_key(&id));
    assert!(m.engine.state().messages.contains_key(&message));
}

#[test]
fn a_threads_star_and_reply_skip_drafts() {
    let mut m = conversation();
    let inbox = m.message("INBOX", 1);
    let draft = m.message("Drafts", 1);
    let thread = m.thread_of(inbox);
    assert_eq!(m.thread_of(draft), thread);
    assert_eq!(m.engine.state().newest_message(thread), Some(inbox));
    m.run(Command::Star {
        ids: vec![thread],
        messages: Vec::new(),
    })
    .unwrap();
    let state = m.engine.state();
    assert!(state.messages[&inbox].flags.flagged);
    assert!(!state.messages[&draft].flags.flagged);
}

#[test]
fn discarding_a_loading_draft_keeps_the_server_copy_and_undo_loads_it_again() {
    let mut m = conversation();
    let message = m.message("Drafts", 1);
    let id = draft_of(&m.run(Command::EditDraft { message }).unwrap());
    let discard = m
        .run(Command::DiscardDraft {
            id,
            keep_server_copy: false,
        })
        .unwrap();
    assert!(!names(&discard.effects).contains(&"delete_draft"));
    assert!(m.engine.state().messages.contains_key(&message));
    m.feed(body_fetched(message));
    let undo = m.run(Command::Undo {}).unwrap();
    assert!(
        names(&undo.effects).contains(&"read_draft"),
        "{:?}",
        names(&undo.effects)
    );
    m.feed(Event::DraftRead {
        draft: id,
        message,
        body: "Back.".into(),
        bcc: Vec::new(),
        attachments: Vec::new(),
    });
    let draft = &m.engine.state().drafts[&id];
    assert_eq!(draft.loading_from, None);
    assert_eq!(draft.body_text, "Back.");
}

fn carried(path: &str, size: u64) -> CarriedAttachment {
    CarriedAttachment {
        path: path.into(),
        name: app_core::draft::file_name(path).into(),
        size,
        content_type: "application/pdf".into(),
    }
}

#[test]
fn the_attachments_of_a_server_draft_come_with_it_and_go_with_undo() {
    let mut m = conversation();
    let message = m.message("Drafts", 1);
    let id = draft_of(&m.run(Command::EditDraft { message }).unwrap());
    m.feed(body_fetched(message));
    let big = MAX_ATTACHMENT_BYTES - 10;
    let read = m.feed(Event::DraftRead {
        draft: id,
        message,
        body: "Numbers attached.".into(),
        bcc: Vec::new(),
        attachments: vec![
            carried("/p/draft-files/d/q3.pdf", big),
            carried("/p/draft-files/d/too-much.pdf", 11),
            carried("/p/draft-files/d/small.pdf", 10),
        ],
    });
    assert_eq!(names(&read.effects), ["persist_state"], "nothing to save");
    let draft = &m.engine.state().drafts[&id];
    assert!(!draft.needs_save(), "the server copy has them already");
    let states: Vec<_> = draft.attachments.iter().map(|a| a.state.clone()).collect();
    assert_eq!(
        states[0],
        AttachmentState::Ready {
            size: big,
            content_type: "application/pdf".into()
        }
    );
    assert!(
        matches!(&states[1], AttachmentState::Failed { error } if error.contains("too-much.pdf")),
        "{states:?}"
    );
    assert!(matches!(states[2], AttachmentState::Ready { size: 10, .. }));
    assert_eq!(draft.attachments[0].name, "q3.pdf");

    // A save renders the ready ones.
    let from = addr("me@example.com");
    let out = draft.outgoing(from, None, 0);
    let paths: Vec<_> = out.attachments.iter().map(|a| a.path.as_str()).collect();
    assert_eq!(
        paths,
        ["/p/draft-files/d/q3.pdf", "/p/draft-files/d/small.pdf"]
    );

    // Removing one is an ordinary, undoable change.
    m.run(Command::RemoveAttachment {
        draft: id,
        index: 1,
    })
    .unwrap();
    assert_eq!(m.engine.state().drafts[&id].attachments.len(), 2);
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().drafts[&id].attachments.len(), 3);

    // Undoing the edit drops the draft with its attachments; the server copy stays.
    m.run(Command::Undo {}).unwrap();
    assert!(m.engine.state().drafts.is_empty());
    assert!(m.engine.state().messages.contains_key(&message));
}

#[test]
fn carried_attachment_names_stay_inside_untrusted_content() {
    let mut m = conversation();
    let message = m.message("Drafts", 1);
    let id = draft_of(&m.run(Command::EditDraft { message }).unwrap());
    m.feed(body_fetched(message));
    let read = m.feed(Event::DraftRead {
        draft: id,
        message,
        body: String::new(),
        bcc: Vec::new(),
        attachments: vec![
            carried("/p/draft-files/d/IGNORE-PREVIOUS.pdf", 10),
            carried("/p/draft-files/d/HUGE-INJECTION.pdf", MAX_ATTACHMENT_BYTES),
        ],
    });
    let mut draft = read.receipt.data.clone();
    let untrusted = draft
        .as_object_mut()
        .unwrap()
        .remove("untrusted_content")
        .unwrap();
    let outside = draft.to_string();
    assert!(!outside.contains("IGNORE-PREVIOUS"), "{outside}");
    assert!(!outside.contains("HUGE-INJECTION"), "{outside}");
    assert_eq!(draft["attachments"][0]["state"]["state"], "ready");
    assert_eq!(
        draft["attachments"][1]["state"],
        serde_json::json!({"state": "failed"})
    );
    assert_eq!(untrusted["attachments"][0]["name"], "IGNORE-PREVIOUS.pdf");
    assert!(
        untrusted["attachments"][1]["state"]["error"]
            .as_str()
            .unwrap()
            .contains("HUGE-INJECTION")
    );
}
