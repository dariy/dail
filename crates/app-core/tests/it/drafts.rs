//! Drafts: `create_draft`, `reply`, `reply_all`, `forward`, `update_draft`, `discard_draft`,
//! `restore_draft`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, names};
use app_core::{
    AccountId, Addr, Command, CoreError, Draft, DraftId, Effect, PersistDelta, QuoteStyle,
    RemoteEnvelope,
};
use serde_json::json;

fn addr(email: &str) -> Addr {
    Addr {
        name: None,
        email: email.into(),
    }
}

fn named(name: &str, email: &str) -> Addr {
    Addr {
        name: Some(name.into()),
        email: email.into(),
    }
}

/// INBOX with a message from Bob to me and Carol, Cc Dave and me.
fn inbox() -> Mail {
    let mut m = Mail::hostinger();
    let mut first = mail(
        1,
        "q3@example.org",
        Some("root@example.org"),
        "Q3 numbers",
        false,
    );
    first.to = vec![named("Me", "ME@example.com"), addr("carol@example.org")];
    first.cc = vec![addr("dave@example.org"), addr("me@example.com")];
    let mut own = mail(2, "mine@example.com", None, "Re: Lunch", true);
    own.from = vec![addr("me@example.com")];
    own.to = vec![addr("erin@example.org")];
    m.fetch("INBOX", vec![first, own]);
    m
}

fn draft_id(data: &serde_json::Value) -> DraftId {
    serde_json::from_value(data["id"].clone()).unwrap()
}

fn create(m: &mut Mail, subject: &str) -> DraftId {
    let account = m.account;
    let outcome = m
        .run(Command::CreateDraft {
            account,
            to: vec![named(" Bob ", " bob@example.org"), addr("BOB@example.org")],
            cc: Vec::new(),
            bcc: vec![addr("boss@example.org")],
            subject: subject.into(),
            body: "Tomorrow at noon?".into(),
        })
        .unwrap();
    draft_id(&outcome.receipt.data)
}

fn draft(m: &Mail, id: DraftId) -> &Draft {
    &m.engine.state().drafts[&id]
}

#[test]
fn create_draft_validates_and_persists_the_draft() {
    let mut m = inbox();
    let account = m.account;
    let outcome = m
        .run(Command::CreateDraft {
            account,
            to: vec![named(" Bob ", " bob@example.org"), addr("BOB@example.org")],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: " Lunch? ".into(),
            body: "Tomorrow at noon?".into(),
        })
        .unwrap();
    let id = draft_id(&outcome.receipt.data);
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Create draft"));
    let content = &outcome.receipt.data["untrusted_content"];
    assert_eq!(
        content["to"],
        json!([{ "name": "Bob", "email": "bob@example.org" }])
    );
    assert_eq!(content["subject"], "Lunch?");
    assert_eq!(content["body"], "Tomorrow at noon?");
    let saved = draft(&m, id);
    assert_eq!(saved.account, account);
    assert_eq!(saved.created_ms, saved.updated_ms);
    assert!(
        saved.message_id_hdr.ends_with("@example.com"),
        "{}",
        saved.message_id_hdr
    );
    assert!(matches!(
        &outcome.effects[..],
        [Effect::PersistState { delta: PersistDelta::Mail(delta) }, Effect::ScheduleTick { .. }]
            if delta.drafts.len() == 1 && delta.drafts[0].id == id
    ));
}

#[test]
fn create_draft_rejects_bad_input() {
    let mut m = inbox();
    let account = m.account;
    let base = |to: Vec<Addr>, subject: &str| Command::CreateDraft {
        account,
        to,
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: subject.into(),
        body: String::new(),
    };
    let err = m.run(base(vec![addr("not-an-address")], "")).unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "to"));
    let err = m
        .run(base(vec![named("A\r\nBcc: x", "a@b.org")], ""))
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "to"));
    let err = m
        .run(base(Vec::new(), "Hi\r\nBcc: evil@x.org"))
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "subject"));
    let err = m
        .run(Command::CreateDraft {
            account: AccountId::from_uuid(uuid::Uuid::from_u128(9)),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "account"));
    assert!(m.engine.state().drafts.is_empty());
}

#[test]
fn undo_discards_a_new_draft_and_redo_brings_back_the_same_id() {
    let mut m = inbox();
    let id = create(&mut m, "Lunch?");
    let created = draft(&m, id).clone();
    m.run(Command::Undo {}).unwrap();
    assert!(m.engine.state().drafts.is_empty());
    let redo = m.run(Command::Redo {}).unwrap();
    assert_eq!(redo.receipt.undo_label.as_deref(), Some("Create draft"));
    let redone = draft(&m, id);
    assert_eq!(redone.message_id_hdr, created.message_id_hdr);
    assert_eq!(redone.to, created.to);
    assert_eq!(redone.body_text, created.body_text);
}

#[test]
fn reply_answers_the_sender_with_threading_headers_and_a_quote() {
    let mut m = inbox();
    let original = m.message("INBOX", 1);
    let outcome = m
        .run(Command::Reply {
            message: original,
            body: Some("Thanks!".into()),
        })
        .unwrap();
    let id = draft_id(&outcome.receipt.data);
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Reply"));
    // The quote needs the original's body; the INBOX prefetch is already downloading it.
    assert_eq!(names(&outcome.effects), ["persist_state", "schedule_tick"]);
    assert!(m.engine.state().status.bodies_in_flight.contains(&original));
    let reply = draft(&m, id);
    assert_eq!(reply.to, [named("Bob", "bob@example.org")]);
    assert!(reply.cc.is_empty());
    assert_eq!(reply.subject, "Re: Q3 numbers");
    assert_eq!(reply.body_text, "Thanks!");
    assert_eq!(reply.reply_to, Some(original));
    assert_eq!(reply.in_reply_to.as_deref(), Some("q3@example.org"));
    assert_eq!(reply.references, ["root@example.org", "q3@example.org"]);
    let quote = reply.quote.as_ref().unwrap();
    assert_eq!(quote.style, QuoteStyle::Reply);
    assert_eq!(quote.message, original);
    assert_eq!(
        quote.header,
        "On 2023-11-13 22:13 UTC, Bob <bob@example.org> wrote:"
    );
}

#[test]
fn reply_all_adds_every_recipient_except_me() {
    let mut m = inbox();
    let original = m.message("INBOX", 1);
    let outcome = m
        .run(Command::ReplyAll {
            message: original,
            body: None,
        })
        .unwrap();
    let reply = draft(&m, draft_id(&outcome.receipt.data));
    assert_eq!(
        reply.to,
        [named("Bob", "bob@example.org"), addr("carol@example.org")]
    );
    assert_eq!(reply.cc, [addr("dave@example.org")]);
    assert_eq!(reply.body_text, "");
}

#[test]
fn reply_to_my_own_message_goes_to_its_recipients() {
    let mut m = inbox();
    let own = m.message("INBOX", 2);
    let outcome = m
        .run(Command::Reply {
            message: own,
            body: None,
        })
        .unwrap();
    // Read mail is not prefetched: replying starts the download for the quote.
    assert_eq!(
        names(&outcome.effects),
        ["persist_state", "schedule_tick", "fetch_body"]
    );
    let reply = draft(&m, draft_id(&outcome.receipt.data));
    assert_eq!(reply.to, [addr("erin@example.org")]);
    assert_eq!(reply.subject, "Re: Lunch", "no second prefix");
}

#[test]
fn forward_has_no_recipients_and_inlines_the_original() {
    let mut m = inbox();
    let original = m.message("INBOX", 1);
    let outcome = m
        .run(Command::Forward {
            message: original,
            body: Some("FYI".into()),
        })
        .unwrap();
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Forward"));
    let forward = draft(&m, draft_id(&outcome.receipt.data));
    assert!(forward.to.is_empty());
    assert_eq!(forward.subject, "Fwd: Q3 numbers");
    assert_eq!(forward.in_reply_to, None);
    assert_eq!(forward.references, ["root@example.org", "q3@example.org"]);
    let quote = forward.quote.as_ref().unwrap();
    assert_eq!(quote.style, QuoteStyle::Forward);
    assert_eq!(
        quote.header,
        "---------- Forwarded message ----------\n\
         From: Bob <bob@example.org>\n\
         Date: 2023-11-13 22:13 UTC\n\
         Subject: Q3 numbers\n\
         To: Me <ME@example.com>, carol@example.org\n\
         Cc: dave@example.org, me@example.com"
    );
}

#[test]
fn answers_reject_unknown_messages() {
    let mut m = inbox();
    let missing = app_core::MessageId::from_uuid(uuid::Uuid::from_u128(77));
    for cmd in [
        Command::Reply {
            message: missing,
            body: None,
        },
        Command::ReplyAll {
            message: missing,
            body: None,
        },
        Command::Forward {
            message: missing,
            body: None,
        },
    ] {
        let err = m.run(cmd).unwrap_err();
        assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "message"));
    }
}

#[test]
fn a_cached_body_is_not_downloaded_again() {
    let mut m = inbox();
    let original = m.message("INBOX", 1);
    m.feed(app_core::Event::BodyFetched {
        message: original,
        blob: "ab".repeat(32),
        has_attachments: false,
        snippet: None,
    });
    let outcome = m
        .run(Command::Reply {
            message: original,
            body: None,
        })
        .unwrap();
    assert_eq!(names(&outcome.effects), ["persist_state", "schedule_tick"]);
}

#[test]
fn update_draft_changes_given_fields_and_undo_restores_them() {
    let mut m = inbox();
    let id = create(&mut m, "Lunch?");
    let before = draft(&m, id).clone();
    let outcome = m
        .run(Command::UpdateDraft {
            id,
            to: None,
            cc: Some(vec![addr("carol@example.org")]),
            bcc: None,
            subject: Some("Lunch on Friday?".into()),
            body: None,
        })
        .unwrap();
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Edit draft"));
    let after = draft(&m, id);
    assert_eq!(after.cc, [addr("carol@example.org")]);
    assert_eq!(after.subject, "Lunch on Friday?");
    assert_eq!(after.to, before.to, "untouched");
    assert!(after.updated_ms > before.updated_ms);

    m.run(Command::Undo {}).unwrap();
    let undone = draft(&m, id);
    assert_eq!(undone.cc, before.cc);
    assert_eq!(undone.subject, before.subject);
    m.run(Command::Redo {}).unwrap();
    assert_eq!(draft(&m, id).subject, "Lunch on Friday?");
}

#[test]
fn update_draft_rejects_empty_patches_bad_values_and_unknown_drafts() {
    let mut m = inbox();
    let id = create(&mut m, "Lunch?");
    let patch = |id, to: Option<Vec<Addr>>| Command::UpdateDraft {
        id,
        to,
        cc: None,
        bcc: None,
        subject: None,
        body: None,
    };
    let err = m.run(patch(id, None)).unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { .. }));
    let err = m.run(patch(id, Some(vec![addr("@x")]))).unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "to"));
    let missing = DraftId::from_uuid(uuid::Uuid::from_u128(5));
    let err = m.run(patch(missing, Some(Vec::new()))).unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "draft"));
    assert_eq!(draft(&m, id).to, [named("Bob", "bob@example.org")]);
}

#[test]
fn discard_draft_is_undone_by_restoring_it_exactly() {
    let mut m = inbox();
    let original = m.message("INBOX", 1);
    let outcome = m
        .run(Command::Reply {
            message: original,
            body: Some("Thanks!".into()),
        })
        .unwrap();
    let id = draft_id(&outcome.receipt.data);
    let before = draft(&m, id).clone();
    let discard = m.run(Command::DiscardDraft { id }).unwrap();
    assert_eq!(discard.receipt.data["discarded"], json!(id));
    assert!(matches!(
        &discard.effects[..],
        [Effect::PersistState { delta: PersistDelta::Mail(delta) }]
            if delta.removed_drafts == [id]
    ));
    assert!(m.engine.state().drafts.is_empty());
    m.run(Command::Undo {}).unwrap();
    assert_eq!(draft(&m, id), &before);
    m.run(Command::Redo {}).unwrap();
    assert!(m.engine.state().drafts.is_empty());

    let err = m.run(Command::DiscardDraft { id }).unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "draft"));
}

#[test]
fn restore_draft_rejects_duplicates_and_unknown_accounts() {
    let mut m = inbox();
    let id = create(&mut m, "Lunch?");
    let existing = draft(&m, id).clone();
    let err = m
        .run(Command::RestoreDraft {
            draft: Box::new(existing.clone()),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::AlreadyExists { ref entity, .. } if entity == "draft"));
    let err = m
        .run(Command::RestoreDraft {
            draft: Box::new(Draft {
                id: DraftId::from_uuid(uuid::Uuid::from_u128(8)),
                account: AccountId::from_uuid(uuid::Uuid::from_u128(9)),
                ..existing.clone()
            }),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "account"));
    let err = m
        .run(Command::RestoreDraft {
            draft: Box::new(Draft {
                id: DraftId::from_uuid(uuid::Uuid::from_u128(8)),
                subject: "a\nb".into(),
                ..existing
            }),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "subject"));
}

#[test]
fn restore_draft_rejects_message_ids_that_would_break_a_line() {
    let mut m = inbox();
    let id = create(&mut m, "Lunch?");
    let existing = draft(&m, id).clone();
    m.run(Command::DiscardDraft { id }).unwrap();
    for (field, bad) in [
        (
            "message_id_hdr",
            Draft {
                message_id_hdr: "a@b\r\nZ1 DELETE INBOX".into(),
                ..existing.clone()
            },
        ),
        (
            "in_reply_to",
            Draft {
                in_reply_to: Some("p@q x".into()),
                ..existing.clone()
            },
        ),
        (
            "references",
            Draft {
                references: vec!["r@s".into(), "t@u\n".into()],
                ..existing.clone()
            },
        ),
    ] {
        let err = m
            .run(Command::RestoreDraft {
                draft: Box::new(bad),
            })
            .unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { field: ref f, .. } if f == field),
            "{field}: {err:?}"
        );
    }
    assert!(m.engine.state().drafts.is_empty());
}

#[test]
fn replies_leave_out_message_ids_that_would_break_a_line() {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![RemoteEnvelope {
            message_id_hdr: Some("a@b\r\nBcc: x@y".into()),
            references: vec!["ok@example.org".into(), "bad id@example.org".into()],
            ..mail(1, "unused", None, "Hi", false)
        }],
    );
    let message = m.message("INBOX", 1);
    let outcome = m
        .run(Command::Reply {
            message,
            body: None,
        })
        .unwrap();
    let reply = draft(&m, draft_id(&outcome.receipt.data));
    assert_eq!(reply.in_reply_to, None);
    assert_eq!(reply.references, ["ok@example.org"]);
    // The draft therefore always comes back after a discard.
    let id = reply.id;
    m.run(Command::DiscardDraft { id }).unwrap();
    m.run(Command::Undo {}).unwrap();
    assert!(m.engine.state().drafts.contains_key(&id));
}

#[test]
fn removing_the_account_drops_its_drafts() {
    let mut m = inbox();
    let id = create(&mut m, "Lunch?");
    let account = m.account;
    let outcome = m
        .run(Command::RemoveAccount {
            id: account,
            confirm: true,
        })
        .unwrap();
    assert!(m.engine.state().drafts.is_empty());
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::PersistState { delta: PersistDelta::Mail(delta) } if delta.removed_drafts == [id]
    )));
}

#[test]
fn reply_to_a_message_without_message_id_has_no_threading_headers() {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![RemoteEnvelope {
            message_id_hdr: None,
            references: Vec::new(),
            in_reply_to: None,
            subject: String::new(),
            ..mail(1, "unused", None, "", false)
        }],
    );
    let message = m.message("INBOX", 1);
    let outcome = m
        .run(Command::Reply {
            message,
            body: None,
        })
        .unwrap();
    let reply = draft(&m, draft_id(&outcome.receipt.data));
    assert_eq!(reply.in_reply_to, None);
    assert!(reply.references.is_empty());
    assert_eq!(reply.subject, "Re:");
}
