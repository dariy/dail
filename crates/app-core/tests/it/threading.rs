//! Threading: Message-ID / In-Reply-To / References links, merges, subject fallback.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::threading::{Placement, SUBJECT_WINDOW_MS, ThreadKey, Threader};
use app_core::{AccountId, BodyState, Envelope, Flags, IdGen, MailboxId, MessageId, ThreadId};

const DAY: i64 = 24 * 60 * 60 * 1000;

fn account(n: u128) -> AccountId {
    AccountId::from_uuid(uuid::Uuid::from_u128(n))
}

fn msg(id: Option<&str>, parents: &[&str], subject: &str, date_ms: i64) -> Envelope {
    Envelope {
        id: MessageId::from_uuid(uuid::Uuid::nil()),
        account: account(1),
        mailbox: MailboxId::from_uuid(uuid::Uuid::nil()),
        uid: 0,
        message_id_hdr: id.map(str::to_owned),
        in_reply_to: parents.last().map(|p| (*p).to_owned()),
        references: parents.iter().map(|p| (*p).to_owned()).collect(),
        thread: ThreadId::from_uuid(uuid::Uuid::nil()),
        from: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        subject: subject.to_owned(),
        date_ms,
        flags: Flags::default(),
        size: 0,
        has_attachments: false,
        snippet: None,
        body: BodyState::NotFetched,
        moved_to: None,
    }
}

/// Threads `messages` in order the way the engine does; returns the thread of each message.
fn thread_all(messages: &mut [Envelope]) -> Vec<ThreadId> {
    let mut threader = Threader::new(account(1), std::iter::empty());
    let mut ids = IdGen::new(1000);
    for index in 0..messages.len() {
        let thread = match threader.place(&ThreadKey::from(&messages[index])) {
            Placement::New => ids.next_id(),
            Placement::Join(thread) => thread,
            Placement::Merge { into, from } => {
                for old in from {
                    threader.merge(old, into);
                    for earlier in messages[..index].iter_mut() {
                        if earlier.thread == old {
                            earlier.thread = into;
                        }
                    }
                }
                into
            }
        };
        messages[index].thread = thread;
        threader.add(&ThreadKey::from(&messages[index]), thread);
    }
    messages.iter().map(|m| m.thread).collect()
}

#[test]
fn replies_join_their_parent_by_in_reply_to_and_references() {
    let mut messages = vec![
        msg(Some("a@x"), &[], "Plan", 0),
        msg(Some("b@x"), &["a@x"], "Re: Plan", DAY),
        msg(Some("c@x"), &["a@x", "b@x"], "Re: Plan", 2 * DAY),
        msg(Some("d@x"), &[], "Other", 3 * DAY),
    ];
    let threads = thread_all(&mut messages);
    assert_eq!(threads[0], threads[1]);
    assert_eq!(threads[1], threads[2]);
    assert_ne!(threads[0], threads[3]);
}

#[test]
fn a_reply_arriving_first_is_joined_by_its_parent() {
    let mut messages = vec![
        msg(Some("b@x"), &["a@x"], "Re: Plan", DAY),
        msg(Some("a@x"), &[], "Plan", 0),
    ];
    let threads = thread_all(&mut messages);
    assert_eq!(threads[0], threads[1]);
}

#[test]
fn siblings_of_an_uncached_parent_share_a_thread() {
    let mut messages = vec![
        msg(Some("b@x"), &["root@x"], "Something", DAY),
        msg(Some("c@x"), &["root@x"], "Different subject", 2 * DAY),
    ];
    let threads = thread_all(&mut messages);
    assert_eq!(threads[0], threads[1]);
}

#[test]
fn copies_in_other_mailboxes_share_a_thread() {
    let mut messages = vec![
        msg(Some("a@x"), &[], "Plan", 0),
        msg(Some("a@x"), &[], "Plan", 0),
    ];
    let threads = thread_all(&mut messages);
    assert_eq!(threads[0], threads[1]);
}

#[test]
fn a_message_linking_two_threads_merges_them() {
    let mut messages = vec![
        msg(Some("a@x"), &[], "Plan", 0),
        msg(Some("c@x"), &["b@x"], "Re: Plan v2", 2 * DAY),
        // `b` replies to `a` and is the parent of `c`.
        msg(Some("b@x"), &["a@x"], "Re: Plan", DAY),
    ];
    let before = {
        let mut first_two = messages[..2].to_vec();
        thread_all(&mut first_two)
    };
    assert_ne!(before[0], before[1]);
    let threads = thread_all(&mut messages);
    assert_eq!(threads[0], threads[1]);
    assert_eq!(threads[1], threads[2]);
}

#[test]
fn subject_fallback_needs_a_reply_prefix_and_recent_activity() {
    let mut messages = vec![
        msg(Some("a@x"), &[], "Lunch?", 0),
        // No references, but a reply prefix and close in time: joins.
        msg(Some("b@y"), &[], "RE: lunch?", DAY),
        // Same subject without a prefix: new thread.
        msg(Some("c@z"), &[], "Lunch?", 2 * DAY),
        // Reply prefix, but long after the last activity: new thread.
        msg(None, &[], "Re: Lunch?", 3 * DAY + SUBJECT_WINDOW_MS + 1),
    ];
    let threads = thread_all(&mut messages);
    assert_eq!(threads[0], threads[1]);
    assert_ne!(threads[0], threads[2]);
    assert_ne!(threads[3], threads[0]);
    assert_ne!(threads[3], threads[2]);
}

#[test]
fn other_accounts_are_not_indexed() {
    let mut theirs = msg(Some("a@x"), &[], "Plan", 0);
    theirs.account = account(2);
    theirs.thread = ThreadId::from_uuid(uuid::Uuid::from_u128(7));
    let threader = Threader::new(account(1), [&theirs]);
    let reply = msg(Some("b@x"), &["a@x"], "Re: Plan", DAY);
    assert_eq!(threader.place(&ThreadKey::from(&reply)), Placement::New);
}

#[test]
fn self_references_are_ignored() {
    let mut messages = vec![
        msg(Some("a@x"), &["a@x"], "Loop", 0),
        msg(Some("b@x"), &[], "Unrelated", DAY),
    ];
    let threads = thread_all(&mut messages);
    assert_ne!(threads[0], threads[1]);
}

/// The threading index the engine keeps between merges gives the same threads as one built
/// afresh before every input (`Engine::new` starts without it).
#[test]
fn the_kept_threading_index_matches_a_fresh_one() {
    use crate::support::{Mail, mail};
    use app_core::{Engine, Event};

    let mut kept = Mail::hostinger();
    let mut fresh = Mail::hostinger();
    let inbox = kept.mailbox("INBOX");
    let account = kept.account;
    let fetch = |envelopes: Vec<app_core::RemoteEnvelope>, total| Event::EnvelopesFetched {
        account,
        mailbox: inbox,
        uid_validity: 7,
        uid_next: Some(100),
        total,
        unread: 0,
        envelopes,
    };
    let inputs = vec![
        // Replies first, their parents later (as a backfill brings them), a subject-only
        // reply, an expunge in between and a merge of two threads by a late common parent.
        fetch(
            vec![
                mail(20, "r1@x", Some("p1@x"), "Re: Plans", true),
                mail(21, "r2@x", Some("p2@x"), "Re: Budget", true),
                mail(22, "s1@x", None, "Re: Lunch", true),
            ],
            30,
        ),
        Event::OlderFetched {
            account,
            mailbox: inbox,
            uid_validity: 7,
            envelopes: vec![
                mail(10, "p1@x", None, "Plans", true),
                mail(11, "lunch@x", None, "Lunch", true),
            ],
            complete: false,
        },
        Event::FlagsChanged {
            account,
            mailbox: inbox,
            uid_validity: 7,
            from_uid: 10,
            to_uid: 22,
            flags: vec![],
            existing: Some(vec![10, 20, 21, 22]),
            highest_modseq: None,
        },
        Event::OlderFetched {
            account,
            mailbox: inbox,
            uid_validity: 7,
            envelopes: vec![
                mail(5, "p2@x", Some("p1@x"), "Budget", true),
                mail(6, "lunch@x", None, "Lunch", true),
            ],
            complete: true,
        },
        fetch(vec![mail(23, "r3@x", Some("p2@x"), "Re: Budget", true)], 30),
    ];
    for input in inputs {
        kept.feed(input.clone());
        fresh.engine = Engine::new(fresh.engine.state().clone());
        fresh.feed(input);
        assert_eq!(kept.engine.state().threads, fresh.engine.state().threads);
        assert_eq!(kept.engine.state().messages, fresh.engine.state().messages);
    }
    assert!(
        kept.engine.state().threads.len() < 5,
        "replies joined their parents"
    );
}
