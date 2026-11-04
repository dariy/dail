//! `empty_trash` (MISC-41): the cached Trash goes at once, the server's with an `empty_mailbox`
//! op that expunges what is there when it runs.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, of_kind};
use app_core::{
    Command, CoreError, Effect, Event, MailboxRole, OpId, ProviderPreset, RemoteOp, RemoteStep,
};

/// Hostinger: "Lunch" (UID 1) in INBOX; "Old" (UID 4) and "Older" (UID 5) in Trash.
fn hostinger() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Lunch", true)]);
    m.fetch(
        "Trash",
        vec![
            mail(4, "o@x", None, "Old", true),
            mail(5, "p@x", None, "Older", false),
        ],
    );
    m
}

fn running(effects: &[Effect]) -> (OpId, Vec<RemoteStep>) {
    let found = of_kind(effects, "run_remote_op");
    assert_eq!(found.len(), 1);
    let Effect::RunRemoteOp { op, steps, .. } = found[0] else {
        unreachable!()
    };
    (*op, steps.clone())
}

fn subjects_in(m: &Mail, path: &str) -> Vec<String> {
    let mailbox = m.mailbox(path);
    let state = m.engine.state();
    let mut out: Vec<String> = state
        .messages
        .values()
        .filter(|e| e.location() == mailbox)
        .map(|e| e.subject.clone())
        .collect();
    out.sort();
    out
}

#[test]
fn empties_the_cached_trash_and_expunges_it_on_the_server() {
    let mut m = hostinger();
    let trash = m.mailbox("Trash");
    let outcome = m
        .run(Command::EmptyTrash {
            account: m.account,
            confirm: true,
        })
        .unwrap();
    let data = &outcome.receipt.data;
    assert_eq!(data["removed"], 2);
    assert_eq!(data["total"], 2);
    assert_eq!(data["undoable"], false);
    assert!(outcome.receipt.undo_label.is_none());
    assert!(subjects_in(&m, "Trash").is_empty());
    assert_eq!(subjects_in(&m, "INBOX"), ["Lunch"]);
    assert_eq!(m.engine.state().threads.len(), 1, "emptied threads go");
    assert_eq!(m.engine.state().mailboxes[&trash].total, 0);
    let (op, steps) = running(&outcome.effects);
    assert!(
        matches!(&steps[..], [RemoteStep::ExpungeAll { path, uid_validity: 7 }] if path == "Trash")
    );
    // Done on the server: Trash is fetched again for its counts.
    let done = m.feed(Event::RemoteOpSucceeded { op });
    assert!(m.engine.state().outbox.is_empty());
    assert!(
        done.effects
            .iter()
            .any(|e| matches!(e, Effect::FetchEnvelopes { mailbox, .. } if *mailbox == trash))
    );
    // Not in history: undo skips it and undoes what came before.
    let undo = m.run(Command::Undo {}).unwrap();
    assert_eq!(undo.receipt.data["undone"], "Add account me@example.com");
}

#[test]
fn needs_confirm_an_account_and_a_trash() {
    let mut m = hostinger();
    let err = m
        .run(Command::EmptyTrash {
            account: m.account,
            confirm: false,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::ConfirmationRequired { .. }));
    assert_eq!(subjects_in(&m, "Trash").len(), 2, "nothing changed");
    let gone = app_core::IdGen::new(99).next_id();
    let err = m
        .run(Command::EmptyTrash {
            account: gone,
            confirm: true,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));
    let mut bare = Mail::new(ProviderPreset::Custom, &[("INBOX", None)]);
    let err = bare
        .run(Command::EmptyTrash {
            account: bare.account,
            confirm: true,
        })
        .unwrap_err();
    assert!(matches!(
        err,
        CoreError::NoMailboxForRole {
            role: MailboxRole::Trash,
            ..
        }
    ));
}

#[test]
fn a_message_on_its_way_to_trash_goes_once_its_move_has_run() {
    let mut m = hostinger();
    let lunch = m.message("INBOX", 1);
    let thread = m.thread_of(lunch);
    let trashed = m
        .run(Command::TrashThreads {
            ids: vec![thread],
            messages: Vec::new(),
        })
        .unwrap();
    let (move_op, _) = running(&trashed.effects);
    let outcome = m
        .run(Command::EmptyTrash {
            account: m.account,
            confirm: true,
        })
        .unwrap();
    assert_eq!(outcome.receipt.data["removed"], 2);
    assert_eq!(outcome.receipt.data["waiting"][0], serde_json::json!(lunch));
    assert_eq!(subjects_in(&m, "Trash"), ["Lunch"], "still moving");
    assert!(
        of_kind(&outcome.effects, "run_remote_op").is_empty(),
        "the move runs first"
    );
    // The move lands, then the empty op runs and takes the message with it.
    let next = m.feed(Event::RemoteOpSucceeded { op: move_op });
    let (empty_op, steps) = running(&next.effects);
    assert!(matches!(&steps[..], [RemoteStep::ExpungeAll { .. }]));
    assert!(matches!(
        m.engine.state().outbox[&empty_op].op,
        RemoteOp::EmptyMailbox { .. }
    ));
    m.feed(Event::RemoteOpSucceeded { op: empty_op });
    assert!(subjects_in(&m, "Trash").is_empty());
    assert!(!m.engine.state().messages.contains_key(&lunch));
}

#[test]
fn a_discarded_empty_fetches_the_trash_again() {
    let mut m = hostinger();
    let trash = m.mailbox("Trash");
    let outcome = m
        .run(Command::EmptyTrash {
            account: m.account,
            confirm: true,
        })
        .unwrap();
    let (op, _) = running(&outcome.effects);
    m.feed(Event::RemoteOpFailed {
        op,
        error: "NO no".into(),
        permanent: true,
    });
    let discarded = m.run(Command::DiscardPendingOp { id: op }).unwrap();
    assert!(
        discarded
            .effects
            .iter()
            .any(|e| matches!(e, Effect::FetchEnvelopes { mailbox, .. } if *mailbox == trash))
    );
    let listed = m.run(Command::ListPendingOps { account: None }).unwrap();
    assert_eq!(listed.receipt.data["ops"], serde_json::json!([]));
}
