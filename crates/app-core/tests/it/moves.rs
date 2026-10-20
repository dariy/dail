//! `archive_threads` / `trash_threads` / `move_threads`: preset-aware targets, optimistic
//! moves, placeholders and their reconciliation.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, names, of_kind};
use app_core::{
    Command, CoreError, Effect, Event, MailboxRole, MessageId, OpState, ProviderPreset, RemoteOp,
    RemoteStep, ThreadId,
};
use serde_json::json;

/// Hostinger: thread "Plans" with UID 1 in INBOX and my reply (UID 1) in Sent; "Lunch" (UID 2)
/// in INBOX. Archive and Trash are fetched (empty).
fn hostinger() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "Plans", true),
            mail(2, "c@x", None, "Lunch", true),
        ],
    );
    m.fetch("Sent", vec![mail(1, "b@x", Some("a@x"), "Re: Plans", true)]);
    m.fetch("Archive", vec![]);
    m.fetch("Trash", vec![]);
    m
}

/// Gmail: "Plans" (a@x) in INBOX and All Mail.
fn gmail() -> Mail {
    let mut m = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
            ("[Gmail]/Trash", None),
            ("[Gmail]/Sent Mail", Some(MailboxRole::Sent)),
        ],
    );
    m.fetch("INBOX", vec![mail(5, "a@x", None, "Plans", true)]);
    m.fetch(
        "[Gmail]/All Mail",
        vec![mail(40, "a@x", None, "Plans", true)],
    );
    m.fetch("[Gmail]/Trash", vec![]);
    m
}

fn thread_ids(m: &mut Mail, path: &str) -> Vec<ThreadId> {
    let mailbox = m.mailbox(path);
    let outcome = m
        .run(Command::ListThreads {
            mailbox,
            offset: 0,
            limit: 50,
        })
        .unwrap();
    outcome.receipt.data["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| serde_json::from_value(t["id"].clone()).unwrap())
        .collect()
}

fn steps(effects: &[Effect]) -> Vec<RemoteStep> {
    let found = of_kind(effects, "run_remote_op");
    assert_eq!(found.len(), 1, "{:?}", names(effects));
    let Effect::RunRemoteOp { steps, .. } = found[0] else {
        unreachable!()
    };
    steps.clone()
}

fn op_id(effects: &[Effect]) -> app_core::OpId {
    let Effect::RunRemoteOp { op, .. } = of_kind(effects, "run_remote_op")[0] else {
        unreachable!()
    };
    *op
}

#[test]
fn archive_moves_inbox_messages_and_leaves_sent_mail() {
    let mut m = hostinger();
    let plans = m.message("INBOX", 1);
    let reply = m.message("Sent", 1);
    let thread = m.thread_of(plans);
    assert_eq!(m.thread_of(reply), thread);
    let archive = m.mailbox("Archive");
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Archive (1 message)")
    );
    assert_eq!(outcome.receipt.data["moved"], json!([plans]));
    let state = m.engine.state();
    assert_eq!(state.messages[&plans].moved_to, Some(archive));
    assert_eq!(
        state.messages[&plans].mailbox,
        m.mailbox("INBOX"),
        "server location"
    );
    assert_eq!(state.messages[&reply].moved_to, None);
    assert_eq!(
        steps(&outcome.effects),
        vec![RemoteStep::Move {
            path: "INBOX".into(),
            uid_validity: 7,
            uids: vec![1],
            to: "Archive".into(),
            create: false,
        }]
    );
    assert!(!thread_ids(&mut m, "INBOX").contains(&thread));
    assert!(thread_ids(&mut m, "Archive").contains(&thread));
    let get = m
        .run(Command::GetMessage {
            id: plans,
            include_body: false,
        })
        .unwrap();
    assert_eq!(get.receipt.data["message"]["mailbox"], json!(archive));
    assert_eq!(get.receipt.data["message"]["moving"], true);
}

#[test]
fn a_moved_message_keeps_its_id_when_the_target_is_fetched() {
    let mut m = hostinger();
    let plans = m.message("INBOX", 1);
    let thread = m.thread_of(plans);
    let archive = m.mailbox("Archive");
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    let op = op_id(&outcome.effects);
    let done = m.feed(Event::RemoteOpSucceeded { op });
    // The message is a placeholder in Archive until a fetch finds its new UID.
    let envelope = &m.engine.state().messages[&plans];
    assert_eq!(
        (envelope.mailbox, envelope.uid, envelope.moved_to),
        (archive, 0, None)
    );
    let fetches = of_kind(&done.effects, "fetch_envelopes");
    assert!(matches!(fetches[..], [Effect::FetchEnvelopes { mailbox, .. }] if *mailbox == archive));

    let before = m.engine.state().messages.len();
    let fetched = m.fetch("Archive", vec![mail(9, "a@x", None, "Plans", true)]);
    assert_eq!(fetched.receipt.data["located"], 1);
    assert_eq!(fetched.receipt.data["added"], 0);
    let state = m.engine.state();
    assert_eq!(state.messages.len(), before);
    assert_eq!(state.messages[&plans].uid, 9);
    assert!(state.outbox.is_empty(), "flags matched: nothing to push");
}

#[test]
fn hostinger_creates_its_archive_folder_inside_inbox() {
    let mut m = Mail::new(
        ProviderPreset::Hostinger,
        &[("INBOX", None), ("INBOX/Sent", Some(MailboxRole::Sent))],
    );
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Plans", true)]);
    let plans = m.message("INBOX", 1);
    let thread = m.thread_of(plans);
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    let state = m.engine.state();
    let created = state
        .mailboxes
        .values()
        .find(|mb| mb.role == Some(MailboxRole::Archive))
        .unwrap()
        .clone();
    assert_eq!(created.path, "INBOX/Archive");
    assert_eq!(created.uid_validity, None);
    assert!(matches!(
        &steps(&outcome.effects)[0],
        RemoteStep::Move { to, create: true, .. } if to == "INBOX/Archive"
    ));
    // A listing taken before the op ran does not drop the new folder.
    let account = m.account;
    m.feed(Event::MailboxesListed {
        account,
        mailboxes: vec![app_core::RemoteMailbox {
            path: "INBOX".into(),
            delimiter: Some("/".into()),
            special_use: None,
            selectable: true,
        }],
    });
    assert!(m.engine.state().mailboxes.contains_key(&created.id));
}

#[test]
fn gmail_archive_moves_to_all_mail_and_merges_with_the_cached_copy() {
    let mut m = gmail();
    let inbox_copy = m.message("INBOX", 5);
    let all_copy = m.message("[Gmail]/All Mail", 40);
    let thread = m.thread_of(inbox_copy);
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert!(matches!(
        &steps(&outcome.effects)[0],
        RemoteStep::Move { path, to, .. } if path == "INBOX" && to == "[Gmail]/All Mail"
    ));
    let done = m.feed(Event::RemoteOpSucceeded {
        op: op_id(&outcome.effects),
    });
    assert!(
        of_kind(&done.effects, "fetch_envelopes").is_empty(),
        "nothing to locate"
    );
    let state = m.engine.state();
    assert!(
        !state.messages.contains_key(&all_copy),
        "merged into the moved message"
    );
    assert_eq!(state.messages[&inbox_copy].uid, 40);
    assert_eq!(
        state.messages[&inbox_copy].mailbox,
        m.mailbox("[Gmail]/All Mail")
    );
    assert!(thread_ids(&mut m, "INBOX").is_empty());
}

#[test]
fn moving_out_of_all_mail_copies_and_undo_removes_the_copy_again() {
    let mut m = gmail();
    let inbox_copy = m.message("INBOX", 5);
    let all_copy = m.message("[Gmail]/All Mail", 40);
    let thread = m.thread_of(inbox_copy);
    let inbox = m.mailbox("INBOX");
    let noop = m.run(Command::MoveThreads {
        ids: vec![thread],
        messages: vec![],
        to: inbox,
    });
    assert!(
        noop.unwrap().receipt.undo_label.is_none(),
        "already in INBOX"
    );
    // Restore an archived message: All Mail keeps its copy.
    let outcome = m
        .run(Command::MoveThreads {
            ids: vec![],
            messages: vec![all_copy],
            to: inbox,
        })
        .unwrap();
    let copied: Vec<MessageId> =
        serde_json::from_value(outcome.receipt.data["copied"].clone()).unwrap();
    assert_eq!(copied.len(), 1);
    let placeholder = &m.engine.state().messages[&copied[0]];
    assert_eq!((placeholder.mailbox, placeholder.uid), (inbox, 0));
    assert_eq!(placeholder.thread, thread);
    let queued = m.engine.state().outbox.values().last().unwrap().op.clone();
    assert!(matches!(queued, RemoteOp::Copy { to, .. } if to == inbox));
    assert!(m.engine.state().messages.contains_key(&all_copy));
}

#[test]
fn trash_uses_the_trash_role_or_the_preset_path() {
    let mut m = hostinger();
    let lunch = m.message("INBOX", 2);
    let thread = m.thread_of(lunch);
    let trash = m.mailbox("Trash");
    m.run(Command::TrashThreads {
        ids: vec![thread],
        messages: vec![],
    })
    .unwrap();
    assert_eq!(m.engine.state().messages[&lunch].moved_to, Some(trash));

    // Gmail's Trash has no SPECIAL-USE attribute here: the preset names it.
    let mut g = gmail();
    let inbox_copy = g.message("INBOX", 5);
    let thread = g.thread_of(inbox_copy);
    let outcome = g
        .run(Command::TrashThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert_eq!(outcome.receipt.data["moved"], json!([inbox_copy]));
    assert!(matches!(
        &steps(&outcome.effects)[0],
        RemoteStep::Move { to, .. } if to == "[Gmail]/Trash"
    ));
}

#[test]
fn trashing_an_archived_gmail_thread_moves_its_all_mail_copy() {
    let mut m = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
            ("[Gmail]/Trash", Some(MailboxRole::Trash)),
        ],
    );
    m.fetch(
        "[Gmail]/All Mail",
        vec![mail(40, "a@x", None, "Plans", true)],
    );
    m.fetch("[Gmail]/Trash", vec![]);
    let archived = m.message("[Gmail]/All Mail", 40);
    let thread = m.thread_of(archived);
    let outcome = m
        .run(Command::TrashThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert_eq!(outcome.receipt.data["moved"], json!([archived]));
    assert_eq!(outcome.receipt.data["copied"], json!([]));
    // Archiving it again is a no-op: archive never takes from All Mail.
    let again = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert_eq!(again.receipt.data["moved"], json!([]));
}

#[test]
fn missing_targets_and_bad_arguments_are_rejected() {
    let mut m = Mail::new(ProviderPreset::Custom, &[("INBOX", None)]);
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Plans", true)]);
    let thread = m.thread_of(m.message("INBOX", 1));
    let before = m.engine.state().clone();
    let err = m
        .run(Command::TrashThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap_err();
    assert!(matches!(
        err,
        CoreError::NoMailboxForRole {
            role: MailboxRole::Trash,
            ..
        }
    ));
    let err = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap_err();
    assert!(matches!(
        err,
        CoreError::NoMailboxForRole {
            role: MailboxRole::Archive,
            ..
        }
    ));
    let err = m
        .run(Command::ArchiveThreads {
            ids: vec![],
            messages: vec![],
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "ids"));
    let err = m
        .run(Command::MoveThreads {
            ids: vec![thread],
            messages: vec![],
            to: app_core::MailboxId::from_uuid(uuid::Uuid::from_u128(5)),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "mailbox"));
    let err = m
        .run(Command::MoveThreads {
            ids: vec![],
            messages: vec![MessageId::from_uuid(uuid::Uuid::from_u128(5))],
            to: m.mailbox("INBOX"),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "message"));
    assert_eq!(*m.engine.state(), before);
}

#[test]
fn move_threads_stays_within_one_account() {
    let mut m = hostinger();
    let thread = m.thread_of(m.message("INBOX", 1));
    let seed = m.seed;
    m.run(Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email: "other@example.com".into(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    })
    .unwrap();
    let other = app_core::AccountId::from_uuid(uuid::Uuid::from_u128(seed << 64));
    m.feed(Event::MailboxesListed {
        account: other,
        mailboxes: vec![app_core::RemoteMailbox {
            path: "INBOX".into(),
            delimiter: Some("/".into()),
            special_use: None,
            selectable: true,
        }],
    });
    let foreign = m
        .engine
        .state()
        .mailboxes
        .values()
        .find(|mb| mb.account == other)
        .unwrap()
        .id;
    let err = m
        .run(Command::MoveThreads {
            ids: vec![thread],
            messages: vec![],
            to: foreign,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "to"));
}

#[test]
fn undo_after_the_move_brings_the_message_back() {
    let mut m = hostinger();
    let plans = m.message("INBOX", 1);
    let thread = m.thread_of(plans);
    let inbox = m.mailbox("INBOX");
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    m.feed(Event::RemoteOpSucceeded {
        op: op_id(&outcome.effects),
    });
    m.fetch("Archive", vec![mail(9, "a@x", None, "Plans", true)]);
    let undo = m.run(Command::Undo {}).unwrap();
    assert_eq!(undo.receipt.data["undone"], "Archive (1 message)");
    assert_eq!(m.engine.state().messages[&plans].location(), inbox);
    assert_eq!(
        steps(&undo.effects),
        vec![RemoteStep::Move {
            path: "Archive".into(),
            uid_validity: 7,
            uids: vec![9],
            to: "INBOX".into(),
            create: false,
        }]
    );
    assert!(thread_ids(&mut m, "INBOX").contains(&thread));
    m.run(Command::Redo {}).unwrap();
    assert_eq!(
        m.engine.state().messages[&plans].location(),
        m.mailbox("Archive")
    );
}

#[test]
fn an_op_waits_for_a_placeholder_to_be_located() {
    let mut m = hostinger();
    let plans = m.message("INBOX", 1);
    let thread = m.thread_of(plans);
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    m.feed(Event::RemoteOpSucceeded {
        op: op_id(&outcome.effects),
    });
    // Undo before the Archive fetch located the message: the op waits for that fetch.
    let undo = m.run(Command::Undo {}).unwrap();
    assert!(of_kind(&undo.effects, "run_remote_op").is_empty());
    let waiting = m.engine.state().outbox.values().next().unwrap().clone();
    assert!(waiting.awaiting_sync);
    assert_eq!(waiting.state, OpState::Queued);
    let fetched = m.fetch("Archive", vec![mail(9, "a@x", None, "Plans", true)]);
    assert!(matches!(
        &steps(&fetched.effects)[0],
        RemoteStep::Move { path, uids, .. } if path == "Archive" && uids == &[9]
    ));
}
