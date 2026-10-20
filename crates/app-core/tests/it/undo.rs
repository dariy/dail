//! Undo of triage commands: an unsent op is cancelled instead of being answered by a
//! compensating op; random command sequences undo and redo exactly.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, of_kind};
use app_core::{
    Command, Effect, Event, MailboxRole, MessageId, OpId, OpState, ProviderPreset, RemoteOp,
    RemoteStep, ThreadId,
};
use proptest::prelude::*;
use serde_json::json;

/// Hostinger with INBOX (UIDs 1–4; 1 and 2 form a thread), Archive and Trash.
fn mailbox() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "Plans", true),
            mail(2, "b@x", Some("a@x"), "Re: Plans", false),
            mail(3, "c@x", None, "Lunch", false),
            mail(4, "d@x", None, "Report", true),
        ],
    );
    m.fetch("Archive", vec![]);
    m.fetch("Trash", vec![]);
    m
}

fn running(effects: &[Effect]) -> OpId {
    let Effect::RunRemoteOp { op, .. } = of_kind(effects, "run_remote_op")[0] else {
        unreachable!()
    };
    *op
}

/// Starts an op on UID 4 so that later ops wait in the queue (unsent).
fn busy(m: &mut Mail) -> OpId {
    let report = m.message("INBOX", 4);
    let outcome = m
        .run(Command::Star {
            ids: vec![],
            messages: vec![report],
        })
        .unwrap();
    running(&outcome.effects)
}

#[test]
fn undo_cancels_an_unsent_flag_op() {
    let mut m = mailbox();
    let first = busy(&mut m);
    let lunch = m.message("INBOX", 3);
    m.run(Command::MarkRead {
        ids: vec![],
        messages: vec![lunch],
    })
    .unwrap();
    assert_eq!(m.engine.state().outbox.len(), 2);
    let undo = m.run(Command::Undo {}).unwrap();
    assert!(of_kind(&undo.effects, "run_remote_op").is_empty());
    let ops: Vec<OpId> = m.engine.state().outbox.keys().copied().collect();
    assert_eq!(ops, [first], "the queued op is gone, no compensating op");
    assert!(!m.engine.state().messages[&lunch].flags.seen);

    // Redo queues it again.
    m.run(Command::Redo {}).unwrap();
    assert_eq!(m.engine.state().outbox.len(), 2);
}

#[test]
fn undo_of_a_sent_op_queues_a_compensating_op() {
    let mut m = mailbox();
    let lunch = m.message("INBOX", 3);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let op = running(&outcome.effects);
    let undo = m.run(Command::Undo {}).unwrap();
    assert_eq!(undo.receipt.data["undone"], "Mark read (1 message)");
    let outbox = &m.engine.state().outbox;
    assert_eq!(outbox.len(), 2);
    assert_eq!(outbox[&op].state, OpState::Running);
    assert!(matches!(
        outbox.values().nth(1).unwrap().op,
        RemoteOp::SetFlag { value: false, .. }
    ));

    // An op that failed once was sent (maybe applied): it is not cancelled either.
    let mut m = mailbox();
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let op = running(&outcome.effects);
    m.feed(Event::RemoteOpFailed {
        op,
        error: "connection reset".into(),
        permanent: false,
    });
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().outbox.len(), 2);
}

#[test]
fn undo_cancels_an_unsent_move() {
    let mut m = mailbox();
    let first = busy(&mut m);
    let lunch = m.message("INBOX", 3);
    let thread = m.thread_of(lunch);
    let outcome = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert!(of_kind(&outcome.effects, "run_remote_op").is_empty());
    let undo = m.run(Command::Undo {}).unwrap();
    assert!(of_kind(&undo.effects, "run_remote_op").is_empty());
    let state = m.engine.state();
    assert_eq!(state.messages[&lunch].moved_to, None);
    assert_eq!(state.outbox.keys().copied().collect::<Vec<_>>(), [first]);

    // When the first op is done, nothing else runs.
    let done = m.feed(Event::RemoteOpSucceeded { op: first });
    assert!(of_kind(&done.effects, "run_remote_op").is_empty());
}

#[test]
fn a_second_move_replaces_an_unsent_one() {
    let mut m = mailbox();
    let first = busy(&mut m);
    let lunch = m.message("INBOX", 3);
    let thread = m.thread_of(lunch);
    m.run(Command::ArchiveThreads {
        ids: vec![thread],
        messages: vec![],
    })
    .unwrap();
    m.run(Command::TrashThreads {
        ids: vec![thread],
        messages: vec![],
    })
    .unwrap();
    assert_eq!(
        m.engine.state().outbox.len(),
        2,
        "the archive op was dropped"
    );
    let done = m.feed(Event::RemoteOpSucceeded { op: first });
    let Effect::RunRemoteOp { steps, .. } = of_kind(&done.effects, "run_remote_op")[0] else {
        unreachable!()
    };
    assert!(matches!(
        &steps[..],
        [RemoteStep::Move { path, to, .. }] if path == "INBOX" && to == "Trash"
    ));
}

#[test]
fn a_move_after_a_sent_move_still_goes_to_the_server() {
    let mut m = mailbox();
    let lunch = m.message("INBOX", 3);
    let thread = m.thread_of(lunch);
    let archived = m
        .run(Command::ArchiveThreads {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    let op = running(&archived.effects);
    m.run(Command::TrashThreads {
        ids: vec![thread],
        messages: vec![],
    })
    .unwrap();
    // Back to INBOX: the unsent trash op is dropped, but the running archive op will move the
    // message, so a move back is still needed.
    let inbox = m.mailbox("INBOX");
    let back = m
        .run(Command::MoveThreads {
            ids: vec![thread],
            messages: vec![],
            to: inbox,
        })
        .unwrap();
    assert_eq!(back.receipt.data["moved"], json!([lunch]));
    let outbox = &m.engine.state().outbox;
    assert_eq!(outbox.len(), 2);
    assert!(outbox.contains_key(&op));
    assert_eq!(m.engine.state().messages[&lunch].location(), inbox);
}

#[test]
fn undo_cancels_an_unsent_copy_out_of_all_mail() {
    let mut m = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
        ],
    );
    m.fetch("INBOX", vec![mail(1, "z@x", None, "Busy", false)]);
    m.fetch(
        "[Gmail]/All Mail",
        vec![mail(40, "a@x", None, "Plans", true)],
    );
    let busy = m.message("INBOX", 1);
    m.run(Command::Star {
        ids: vec![],
        messages: vec![busy],
    })
    .unwrap();
    let archived = m.message("[Gmail]/All Mail", 40);
    let inbox = m.mailbox("INBOX");
    let restored = m
        .run(Command::MoveThreads {
            ids: vec![],
            messages: vec![archived],
            to: inbox,
        })
        .unwrap();
    let copy: Vec<MessageId> =
        serde_json::from_value(restored.receipt.data["copied"].clone()).unwrap();
    assert!(m.engine.state().messages.contains_key(&copy[0]));
    m.run(Command::Undo {}).unwrap();
    let state = m.engine.state();
    assert!(
        !state.messages.contains_key(&copy[0]),
        "placeholder deleted"
    );
    assert_eq!(state.outbox.len(), 1, "only the star op is left");
    let thread = state.messages[&archived].thread;
    assert_eq!(state.threads[&thread].messages, vec![archived]);
    // Redo makes the same copy again.
    m.run(Command::Redo {}).unwrap();
    assert!(m.engine.state().messages.contains_key(&copy[0]));
}

/// One step of a random triage session.
#[derive(Debug, Clone)]
enum Action {
    MarkRead(usize),
    MarkUnread(usize),
    Star(usize),
    Unstar(usize),
    Archive(usize),
    Trash(usize),
    ToInbox(usize),
    /// The running op of the account succeeds.
    Succeed,
    /// The running op of the account fails.
    Fail,
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (0..4_usize).prop_map(Action::MarkRead),
        (0..4_usize).prop_map(Action::MarkUnread),
        (0..4_usize).prop_map(Action::Star),
        (0..4_usize).prop_map(Action::Unstar),
        (0..3_usize).prop_map(Action::Archive),
        (0..3_usize).prop_map(Action::Trash),
        (0..3_usize).prop_map(Action::ToInbox),
        Just(Action::Succeed),
        Just(Action::Fail),
    ]
}

/// What the user sees of every message: where it is shown and its read/star state.
fn view(m: &Mail) -> Vec<(String, app_core::MailboxId, bool, bool)> {
    let mut seen: Vec<_> = m
        .engine
        .state()
        .messages
        .values()
        .map(|e| {
            (
                e.message_id_hdr.clone().unwrap_or_default(),
                e.location(),
                e.flags.seen,
                e.flags.flagged,
            )
        })
        .collect();
    seen.sort();
    seen
}

fn apply(m: &mut Mail, action: &Action, messages: &[MessageId], threads: &[ThreadId]) {
    let one = |i: usize| vec![messages[i]];
    let thread = |i: usize| vec![threads[i]];
    let result = match action {
        Action::MarkRead(i) => m.run(Command::MarkRead {
            ids: vec![],
            messages: one(*i),
        }),
        Action::MarkUnread(i) => m.run(Command::MarkUnread {
            ids: vec![],
            messages: one(*i),
        }),
        Action::Star(i) => m.run(Command::Star {
            ids: vec![],
            messages: one(*i),
        }),
        Action::Unstar(i) => m.run(Command::Unstar {
            ids: vec![],
            messages: one(*i),
        }),
        Action::Archive(i) => m.run(Command::ArchiveThreads {
            ids: thread(*i),
            messages: vec![],
        }),
        Action::Trash(i) => m.run(Command::TrashThreads {
            ids: thread(*i),
            messages: vec![],
        }),
        Action::ToInbox(i) => {
            let inbox = m.mailbox("INBOX");
            m.run(Command::MoveThreads {
                ids: thread(*i),
                messages: vec![],
                to: inbox,
            })
        }
        Action::Succeed | Action::Fail => {
            let Some(op) = m
                .engine
                .state()
                .outbox
                .values()
                .find(|op| op.state == OpState::Running)
                .map(|op| op.id)
            else {
                return;
            };
            if matches!(action, Action::Succeed) {
                m.feed(Event::RemoteOpSucceeded { op });
            } else {
                m.feed(Event::RemoteOpFailed {
                    op,
                    error: "connection reset".into(),
                    permanent: false,
                });
            }
            return;
        }
    };
    result.unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn random_triage_undoes_and_redoes_exactly(actions in prop::collection::vec(action(), 1..30)) {
        let mut m = mailbox();
        let messages: Vec<MessageId> = (1..=4).map(|uid| m.message("INBOX", uid)).collect();
        let mut threads: Vec<ThreadId> = Vec::new();
        for id in &messages {
            let thread = m.thread_of(*id);
            if !threads.contains(&thread) {
                threads.push(thread);
            }
        }
        let before = view(&m);
        let base = m.engine.history().undo_len();
        for action in &actions {
            apply(&mut m, action, &messages, &threads);
        }
        let after = view(&m);
        let recorded = m.engine.history().undo_len() - base;
        for _ in 0..recorded {
            m.run(Command::Undo {}).unwrap();
        }
        prop_assert_eq!(view(&m), before);
        for _ in 0..recorded {
            m.run(Command::Redo {}).unwrap();
        }
        prop_assert_eq!(view(&m), after);
        // Every op left in the outbox still has something to do.
        for op in m.engine.state().outbox.values() {
            prop_assert!(!op.op.messages().is_empty());
        }
    }
}
