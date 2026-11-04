//! `mark_junk` / `mark_not_junk` (MISC-42): moves to the account's Junk and back to the inbox,
//! with the `$Junk` / `$NotJunk` keywords stored before the move.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, of_kind};
use app_core::{Command, CoreError, Effect, MailboxRole, ProviderPreset, RemoteOp, RemoteStep};

/// Hostinger: "Offer" (UID 1) in INBOX; "Prize" (UID 3) in Junk.
fn hostinger() -> Mail {
    let mut m = Mail::new(
        ProviderPreset::Hostinger,
        &[
            ("INBOX", None),
            ("Junk", Some(MailboxRole::Junk)),
            ("Trash", Some(MailboxRole::Trash)),
        ],
    );
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Offer", false)]);
    m.fetch("Junk", vec![mail(3, "j@x", None, "Prize", false)]);
    m
}

fn steps(effects: &[Effect]) -> Vec<RemoteStep> {
    let found = of_kind(effects, "run_remote_op");
    assert_eq!(found.len(), 1);
    let Effect::RunRemoteOp { steps, .. } = found[0] else {
        unreachable!()
    };
    steps.clone()
}

#[test]
fn mark_junk_stores_the_keyword_then_moves_and_undo_moves_back() {
    let mut m = hostinger();
    let (inbox, junk) = (m.mailbox("INBOX"), m.mailbox("Junk"));
    let offer = m.message("INBOX", 1);
    let thread = m.thread_of(offer);
    let outcome = m
        .run(Command::MarkJunk {
            ids: vec![thread],
            messages: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Mark as junk (1 message)")
    );
    assert_eq!(m.engine.state().messages[&offer].location(), junk);
    assert!(m.engine.state().outbox.values().any(|op| matches!(
        &op.op,
        RemoteOp::Move { to, junk: Some(true), .. } if *to == junk
    )));
    let steps = steps(&outcome.effects);
    assert!(
        matches!(
            &steps[..],
            [
                RemoteStep::Junk { path, uids, junk: true, .. },
                RemoteStep::Move { to, .. },
            ] if path == "INBOX" && uids == &[1] && to == "Junk"
        ),
        "{steps:?}"
    );
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().messages[&offer].location(), inbox);
}

#[test]
fn not_junk_moves_only_junk_back_to_the_inbox() {
    let mut m = hostinger();
    let inbox = m.mailbox("INBOX");
    let prize = m.message("Junk", 3);
    let offer = m.message("INBOX", 1);
    let outcome = m
        .run(Command::MarkNotJunk {
            ids: vec![m.thread_of(prize)],
            messages: Vec::new(),
        })
        .unwrap();
    assert_eq!(m.engine.state().messages[&prize].location(), inbox);
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Not junk (1 message)")
    );
    let steps = steps(&outcome.effects);
    assert!(
        matches!(
            &steps[..],
            [
                RemoteStep::Junk { path, junk: false, .. },
                RemoteStep::Move { to, .. },
            ] if path == "Junk" && to == "INBOX"
        ),
        "{steps:?}"
    );
    // A thread that is not in Junk stays where it is.
    let again = m
        .run(Command::MarkNotJunk {
            ids: vec![m.thread_of(offer)],
            messages: Vec::new(),
        })
        .unwrap();
    assert_eq!(again.receipt.data["moved"], serde_json::json!([]));
}

#[test]
fn an_account_without_junk_gets_an_error_and_no_new_folder() {
    let mut m = Mail::hostinger();
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Offer", false)]);
    let offer = m.message("INBOX", 1);
    let before = m.engine.state().mailboxes.len();
    let err = m
        .run(Command::MarkJunk {
            ids: vec![m.thread_of(offer)],
            messages: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(
        err,
        CoreError::NoMailboxForRole {
            role: MailboxRole::Junk,
            ..
        }
    ));
    assert_eq!(m.engine.state().mailboxes.len(), before);
    let err = m
        .run(Command::MarkJunk {
            ids: Vec::new(),
            messages: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { .. }));
}

#[test]
fn gmail_spam_takes_a_message_out_of_all_mail() {
    let mut m = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
            ("[Gmail]/Spam", None),
        ],
    );
    m.fetch(
        "[Gmail]/All Mail",
        vec![mail(40, "a@x", None, "Offer", true)],
    );
    m.fetch("[Gmail]/Spam", vec![]);
    let spam = m.mailbox("[Gmail]/Spam");
    let offer = m.message("[Gmail]/All Mail", 40);
    let outcome = m
        .run(Command::MarkJunk {
            ids: vec![m.thread_of(offer)],
            messages: Vec::new(),
        })
        .unwrap();
    assert_eq!(m.engine.state().messages[&offer].location(), spam);
    assert_eq!(
        outcome.receipt.data["copied"],
        serde_json::json!([]),
        "a move, not a copy"
    );
}
