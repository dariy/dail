//! `mark_read` / `mark_unread` / `star` / `unstar`: local change, inverse, pending ops.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, Mail, mail, names, of_kind};
use app_core::{
    Command, CommandKind, CoreError, Effect, EntityRef, Event, FlagKind, MailboxRole, MessageId,
    OpState, PersistDelta, ProviderPreset, RemoteOp, RemoteStep, ThreadId,
};

/// INBOX: thread A (UIDs 1 read, 2 unread), thread B (UID 3 unread).
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

fn seen(m: &Mail, id: MessageId) -> bool {
    m.engine.state().messages[&id].flags.seen
}

fn flagged(m: &Mail, id: MessageId) -> bool {
    m.engine.state().messages[&id].flags.flagged
}

fn run_op(effects: &[Effect]) -> (app_core::OpId, Vec<RemoteStep>) {
    let found = of_kind(effects, "run_remote_op");
    assert_eq!(found.len(), 1, "{:?}", names(effects));
    let Effect::RunRemoteOp { op, steps, .. } = found[0] else {
        unreachable!()
    };
    (*op, steps.clone())
}

#[test]
fn mark_read_changes_unread_messages_and_queues_one_op() {
    let mut m = inbox();
    let (first, reply) = (m.message("INBOX", 1), m.message("INBOX", 2));
    let thread = m.thread_of(first);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Mutation));
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Mark read (1 message)")
    );
    assert!(seen(&m, reply));
    assert_eq!(outcome.receipt.data["changed"], serde_json::json!([reply]));
    assert!(
        outcome
            .receipt
            .affected
            .contains(&EntityRef::Thread(thread))
    );

    let (op, steps) = run_op(&outcome.effects);
    assert_eq!(
        steps,
        vec![RemoteStep::Store {
            path: "INBOX".into(),
            uid_validity: 7,
            uids: vec![2],
            flag: FlagKind::Seen,
            value: true,
        }]
    );
    let pending = &m.engine.state().outbox[&op];
    assert_eq!(pending.state, OpState::Running);
    assert_eq!(pending.account, m.account);
    assert_eq!(
        pending.op,
        RemoteOp::SetFlag {
            flag: FlagKind::Seen,
            value: true,
            messages: vec![reply],
        }
    );
    // The local change and the op are persisted together, before the op runs.
    assert_eq!(names(&outcome.effects), ["persist_state", "run_remote_op"]);
    let Effect::PersistState {
        delta: PersistDelta::Mail(delta),
    } = &outcome.effects[0]
    else {
        panic!("no mail delta");
    };
    assert_eq!(delta.messages.len(), 1);
    assert_eq!(delta.ops.len(), 1);
}

#[test]
fn nothing_to_change_is_not_recorded() {
    let mut m = inbox();
    let first = m.message("INBOX", 1);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![first],
        })
        .unwrap();
    assert_eq!(outcome.receipt.undo_label, None);
    assert!(outcome.effects.is_empty());
    assert!(m.engine.state().outbox.is_empty());
    assert_eq!(m.engine.history().undo_len(), 1, "only add_account");
}

#[test]
fn invalid_arguments_leave_state_untouched() {
    let mut m = inbox();
    let before = m.engine.state().clone();
    let err = m
        .run(Command::Star {
            ids: vec![],
            messages: vec![],
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "ids"));
    let thread = m.thread_of(m.message("INBOX", 3));
    let err = m
        .run(Command::MarkUnread {
            ids: vec![thread, ThreadId::from_uuid(uuid::Uuid::from_u128(5))],
            messages: vec![],
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "thread"));
    let err = m
        .run(Command::Unstar {
            ids: vec![],
            messages: vec![MessageId::from_uuid(uuid::Uuid::from_u128(5))],
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "message"));
    assert_eq!(*m.engine.state(), before);
}

#[test]
fn star_marks_the_newest_message_and_its_copies() {
    let mut m = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
        ],
    );
    let thread_mail = vec![
        mail(1, "a@x", None, "Plans", true),
        mail(2, "b@x", Some("a@x"), "Re: Plans", true),
    ];
    m.fetch("INBOX", thread_mail.clone());
    m.fetch("[Gmail]/All Mail", thread_mail);
    let newest = m.message("INBOX", 2);
    let copy = m.message("[Gmail]/All Mail", 2);
    let thread = m.thread_of(newest);
    assert_eq!(m.thread_of(copy), thread);
    let outcome = m
        .run(Command::Star {
            ids: vec![thread],
            messages: vec![],
        })
        .unwrap();
    assert!(flagged(&m, newest) && flagged(&m, copy));
    assert!(!flagged(&m, m.message("INBOX", 1)));
    let (_, steps) = run_op(&outcome.effects);
    assert_eq!(steps.len(), 2, "one STORE per mailbox");

    // Unstar on a thread clears every flagged message.
    let first = m.message("INBOX", 1);
    m.run(Command::Star {
        ids: vec![],
        messages: vec![first],
    })
    .unwrap();
    m.run(Command::Unstar {
        ids: vec![thread],
        messages: vec![],
    })
    .unwrap();
    assert!(m.engine.state().messages.values().all(|e| !e.flags.flagged));
}

#[test]
fn ops_of_one_account_run_one_at_a_time() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    let reply = m.message("INBOX", 2);
    let first = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let (op1, _) = run_op(&first.effects);
    let second = m
        .run(Command::Star {
            ids: vec![],
            messages: vec![reply],
        })
        .unwrap();
    assert!(of_kind(&second.effects, "run_remote_op").is_empty());
    let op2 = m.engine.state().outbox.keys().nth(1).copied().unwrap();
    assert_eq!(m.engine.state().outbox[&op2].state, OpState::Queued);

    let done = m.feed(Event::RemoteOpSucceeded { op: op1 });
    let (started, steps) = run_op(&done.effects);
    assert_eq!(started, op2);
    assert!(matches!(
        steps[0],
        RemoteStep::Store {
            flag: FlagKind::Flagged,
            ..
        }
    ));
    assert!(!m.engine.state().outbox.contains_key(&op1));
    m.feed(Event::RemoteOpSucceeded { op: op2 });
    assert!(m.engine.state().outbox.is_empty());
}

#[test]
fn a_failed_op_is_retried_with_back_off_then_given_up() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let (op, _) = run_op(&outcome.effects);
    let star = m
        .run(Command::Star {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    assert!(of_kind(&star.effects, "run_remote_op").is_empty());
    let mut now = support::ctx(m.seed).now_ms;
    let failed = m.feed(Event::RemoteOpFailed {
        op,
        error: "IMAP UID STORE failed: connection reset".into(),
        permanent: false,
    });
    let pending = m.engine.state().outbox[&op].clone();
    assert_eq!(pending.state, OpState::Queued);
    assert_eq!(pending.attempts, 1);
    assert_eq!(
        pending.last_error.as_deref(),
        Some("IMAP UID STORE failed: connection reset")
    );
    assert_eq!(pending.next_attempt_ms, Some(now + 2_000));
    assert!(matches!(
        failed.effects[..],
        [_, Effect::ScheduleTick { at_ms }] if at_ms == now + 2_000
    ));
    assert!(seen(&m, lunch), "the local change stays");

    // The op keeps its place: an early tick starts nothing, a due one retries it.
    let early = m
        .engine
        .ingest(
            Event::Tick {
                now_ms: now + 1_999,
            },
            &support::ctx(50),
        )
        .unwrap();
    assert!(of_kind(&early.effects, "run_remote_op").is_empty());
    for attempt in 1..app_core::ops::MAX_ATTEMPTS {
        now += app_core::ops::retry_delay_ms(attempt);
        let tick = m
            .engine
            .ingest(Event::Tick { now_ms: now }, &support::ctx(50))
            .unwrap();
        assert_eq!(run_op(&tick.effects).0, op, "attempt {}", attempt + 1);
        let failed = m
            .engine
            .ingest(
                Event::RemoteOpFailed {
                    op,
                    error: "IMAP UID STORE in INBOX failed: no response: [SERVERBUG]".into(),
                    permanent: true,
                },
                &app_core::Ctx {
                    now_ms: now,
                    ..support::ctx(51)
                },
            )
            .unwrap();
        if attempt + 1 == app_core::ops::MAX_ATTEMPTS {
            // Given up: the next op of the account starts.
            assert_eq!(m.engine.state().outbox[&op].state, OpState::Failed);
            assert_eq!(of_kind(&failed.effects, "run_remote_op").len(), 1);
            assert!(of_kind(&failed.effects, "schedule_tick").is_empty());
        }
    }
    assert_eq!(
        m.engine.state().outbox[&op].attempts,
        app_core::ops::MAX_ATTEMPTS
    );
}

#[test]
fn an_unreachable_server_never_makes_an_op_give_up() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let (op, _) = run_op(&outcome.effects);
    let mut now = support::ctx(m.seed).now_ms;
    for attempt in 1..=3 * app_core::ops::MAX_ATTEMPTS {
        let failed = m
            .engine
            .ingest(
                Event::RemoteOpFailed {
                    op,
                    error: "connect failed: network is unreachable".into(),
                    permanent: false,
                },
                &app_core::Ctx {
                    now_ms: now,
                    ..support::ctx(60)
                },
            )
            .unwrap();
        assert_eq!(of_kind(&failed.effects, "schedule_tick").len(), 1);
        now += app_core::ops::retry_delay_ms(attempt);
        let tick = m
            .engine
            .ingest(Event::Tick { now_ms: now }, &support::ctx(61))
            .unwrap();
        assert_eq!(run_op(&tick.effects).0, op);
    }
    assert_eq!(m.engine.state().outbox[&op].state, OpState::Running);
}

#[test]
fn retry_delays_double_up_to_five_minutes() {
    use app_core::ops::retry_delay_ms;
    assert_eq!(retry_delay_ms(1), 2_000);
    assert_eq!(retry_delay_ms(2), 4_000);
    assert_eq!(retry_delay_ms(7), 128_000);
    assert_eq!(retry_delay_ms(9), 300_000);
    assert_eq!(retry_delay_ms(u32::MAX), 300_000);
}

#[test]
fn a_successful_sync_retries_waiting_ops_at_once() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let (op, _) = run_op(&outcome.effects);
    m.feed(Event::RemoteOpFailed {
        op,
        error: "connection reset".into(),
        permanent: false,
    });
    let fetched = m.fetch("INBOX", vec![]);
    assert_eq!(run_op(&fetched.effects).0, op);
    assert_eq!(m.engine.state().outbox[&op].next_attempt_ms, None);
}

#[test]
fn undo_and_redo_restore_the_exact_flags() {
    let mut m = inbox();
    let (first, reply) = (m.message("INBOX", 1), m.message("INBOX", 2));
    let thread = m.thread_of(first);
    let before: Vec<_> = m
        .engine
        .state()
        .messages
        .values()
        .map(|e| e.flags)
        .collect();
    m.run(Command::MarkUnread {
        ids: vec![thread],
        messages: vec![],
    })
    .unwrap();
    assert!(!seen(&m, first) && !seen(&m, reply));
    let after: Vec<_> = m
        .engine
        .state()
        .messages
        .values()
        .map(|e| e.flags)
        .collect();

    m.run(Command::Undo {}).unwrap();
    let undone: Vec<_> = m
        .engine
        .state()
        .messages
        .values()
        .map(|e| e.flags)
        .collect();
    assert_eq!(
        undone, before,
        "only the message that changed is read again"
    );
    m.run(Command::Redo {}).unwrap();
    let redone: Vec<_> = m
        .engine
        .state()
        .messages
        .values()
        .map(|e| e.flags)
        .collect();
    assert_eq!(redone, after);
}

#[test]
fn sync_keeps_local_changes_that_have_not_reached_the_server() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    m.run(Command::MarkRead {
        ids: vec![],
        messages: vec![lunch],
    })
    .unwrap();
    // The server still reports UID 3 as unread, and someone starred it elsewhere.
    let mut stale = mail(3, "c@x", None, "Lunch", false);
    stale.flags.flagged = true;
    m.fetch("INBOX", vec![stale]);
    assert!(seen(&m, lunch) && flagged(&m, lunch));
    let inbox = m.mailbox("INBOX");
    m.feed(Event::FlagsChanged {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        from_uid: 3,
        to_uid: 3,
        flags: vec![app_core::UidFlags {
            uid: 3,
            flags: app_core::Flags::default(),
        }],
        existing: None,
        highest_modseq: None,
    });
    assert!(seen(&m, lunch) && !flagged(&m, lunch));
}

#[test]
fn ops_without_server_work_complete_at_once() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    let reply = m.message("INBOX", 2);
    let first = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![reply],
        })
        .unwrap();
    let (op1, _) = run_op(&first.effects);
    m.run(Command::MarkRead {
        ids: vec![],
        messages: vec![lunch],
    })
    .unwrap();
    // UID 3 is expunged before its op starts.
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
    let done = m.feed(Event::RemoteOpSucceeded { op: op1 });
    assert!(of_kind(&done.effects, "run_remote_op").is_empty());
    assert!(m.engine.state().outbox.is_empty());
}

#[test]
fn running_ops_are_queued_again_after_a_restart_and_dropped_with_their_account() {
    let mut m = inbox();
    let lunch = m.message("INBOX", 3);
    let outcome = m
        .run(Command::MarkRead {
            ids: vec![],
            messages: vec![lunch],
        })
        .unwrap();
    let (op, _) = run_op(&outcome.effects);
    let restarted = app_core::Engine::new(m.engine.state().clone());
    assert_eq!(restarted.state().outbox[&op].state, OpState::Queued);

    // Results of unknown ops are ignored.
    let unknown = app_core::OpId::from_uuid(uuid::Uuid::from_u128(5));
    let ignored = m.feed(Event::RemoteOpSucceeded { op: unknown });
    assert!(ignored.effects.is_empty());
    m.feed(Event::RemoteOpFailed {
        op: unknown,
        error: "x".into(),
        permanent: false,
    });

    m.run(Command::RemoveAccount {
        id: m.account,
        confirm: true,
    })
    .unwrap();
    assert!(m.engine.state().outbox.is_empty());
}
