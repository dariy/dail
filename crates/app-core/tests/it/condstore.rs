//! CONDSTORE/QRESYNC flag resync: the mailbox remembers HIGHESTMODSEQ from the last resync; the
//! next one asks for changes since then, and `existing` (CONDSTORE) or `vanished` (QRESYNC) tells
//! which unlisted UIDs are still there.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail};
use app_core::{Effect, Event, Flags, PersistDelta, UidFlags, UidRange};

/// INBOX with UIDs 1–3 cached (all read).
fn cached() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "One", true),
            mail(2, "b@x", None, "Two", true),
            mail(3, "c@x", None, "Three", true),
        ],
    );
    m
}

fn resync_of(effects: &[Effect]) -> Option<u64> {
    effects.iter().find_map(|e| match e {
        Effect::ResyncFlags { changed_since, .. } => Some(changed_since.unwrap_or(0)),
        _ => None,
    })
}

/// The UID range of the `ResyncFlags` among `effects`.
fn range_of(effects: &[Effect]) -> Option<(u32, u32)> {
    effects.iter().find_map(|e| match e {
        Effect::ResyncFlags {
            from_uid, to_uid, ..
        } => Some((*from_uid, *to_uid)),
        _ => None,
    })
}

/// The answer to a resync of UIDs `from..=to` of INBOX.
fn resynced(
    m: &Mail,
    (from_uid, to_uid): (u32, u32),
    flags: Vec<UidFlags>,
    existing: Option<Vec<u32>>,
    modseq: Option<u64>,
) -> Event {
    Event::FlagsChanged {
        account: m.account,
        mailbox: m.mailbox("INBOX"),
        uid_validity: 7,
        from_uid,
        to_uid,
        flags,
        existing,
        vanished: None,
        highest_modseq: modseq,
    }
}

fn flags_changed(
    m: &Mail,
    flags: Vec<UidFlags>,
    existing: Option<Vec<u32>>,
    modseq: Option<u64>,
) -> Event {
    resynced(m, (1, 3), flags, existing, modseq)
}

/// A QRESYNC answer to a resync of UIDs 1–3 of INBOX: `flags` changed, the UIDs `gone` vanished.
fn qresynced(m: &Mail, flags: Vec<UidFlags>, gone: &[(u32, u32)], modseq: u64) -> Event {
    Event::FlagsChanged {
        account: m.account,
        mailbox: m.mailbox("INBOX"),
        uid_validity: 7,
        from_uid: 1,
        to_uid: 3,
        flags,
        existing: None,
        vanished: Some(
            gone.iter()
                .map(|&(first, last)| UidRange { first, last })
                .collect(),
        ),
        highest_modseq: Some(modseq),
    }
}

#[test]
fn the_modseq_of_a_resync_is_used_by_the_next_one() {
    let mut m = cached();
    // Without a modseq yet, the resync asks for every flag.
    let next = m.fetch("INBOX", vec![]);
    assert_eq!(resync_of(&next.effects), Some(0));
    let all = vec![1, 2, 3]
        .into_iter()
        .map(|uid| UidFlags {
            uid,
            flags: Flags {
                seen: true,
                ..Flags::default()
            },
        })
        .collect();
    let event = flags_changed(&m, all, None, Some(41));
    let outcome = m.feed(event);
    let inbox = m.mailbox("INBOX");
    assert_eq!(m.engine.state().mailboxes[&inbox].highest_modseq, Some(41));
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::PersistState { delta: PersistDelta::Mail(d) } if d.mailboxes.iter().any(|mb| mb.highest_modseq == Some(41))
    )));
    let next = m.fetch("INBOX", vec![]);
    assert_eq!(resync_of(&next.effects), Some(41));
}

#[test]
fn only_changed_flags_are_listed_and_existing_tells_expunges() {
    let mut m = cached();
    let (one, two, three) = (
        m.message("INBOX", 1),
        m.message("INBOX", 2),
        m.message("INBOX", 3),
    );
    // UID 2 was flagged elsewhere, UID 3 expunged; UID 1 did not change.
    let changed = vec![UidFlags {
        uid: 2,
        flags: Flags {
            seen: true,
            flagged: true,
            ..Flags::default()
        },
    }];
    let event = flags_changed(&m, changed, Some(vec![1, 2]), Some(42));
    let outcome = m.feed(event);
    assert_eq!(outcome.receipt.data["flags_updated"], 1);
    assert_eq!(outcome.receipt.data["expunged"], 1);
    let state = m.engine.state();
    assert!(
        state.messages[&one].flags.seen,
        "unlisted but existing: kept as it was"
    );
    assert!(state.messages[&two].flags.flagged);
    assert!(!state.messages.contains_key(&three));
}

#[test]
fn the_resync_after_a_fetch_covers_what_it_merged() {
    let mut m = cached();
    let outcome = m.fetch("INBOX", vec![mail(4, "d@x", None, "Four", true)]);
    assert_eq!(range_of(&outcome.effects), Some((1, 4)));
}

#[test]
fn a_resync_that_missed_a_cached_message_keeps_the_old_modseq() {
    let mut m = cached();
    let inbox = m.mailbox("INBOX");
    let modseq = |m: &Mail| m.engine.state().mailboxes[&inbox].highest_modseq;
    m.feed(flags_changed(&m, vec![], Some(vec![1, 2, 3]), Some(10)));
    assert_eq!(modseq(&m), Some(10));

    // A resync of 1–3 is under way when a fetch that ran before it merges UID 4: changes to
    // UID 4 between that fetch and the resync's SELECT lie below the resync's modseq.
    let next = m.fetch("INBOX", vec![]);
    assert_eq!(range_of(&next.effects), Some((1, 3)));
    m.fetch("INBOX", vec![mail(4, "d@x", None, "Four", true)]);
    m.feed(flags_changed(&m, vec![], Some(vec![1, 2, 3]), Some(20)));
    assert_eq!(modseq(&m), Some(10), "UID 4 was not covered");
    let next = m.fetch("INBOX", vec![]);
    assert_eq!(resync_of(&next.effects), Some(10));

    // A resync that covers every cached message counts.
    m.feed(resynced(
        &m,
        (1, 4),
        vec![],
        Some(vec![1, 2, 3, 4]),
        Some(21),
    ));
    assert_eq!(modseq(&m), Some(21));

    // Forgetting is always safe: a server that stops reporting one resets it.
    m.fetch("INBOX", vec![mail(5, "e@x", None, "Five", true)]);
    m.feed(flags_changed(&m, vec![], Some(vec![1, 2, 3]), None));
    assert_eq!(modseq(&m), None);
}

#[test]
fn vanished_uids_are_dropped_and_unlisted_ones_kept() {
    let mut m = cached();
    let (one, two, three) = (
        m.message("INBOX", 1),
        m.message("INBOX", 2),
        m.message("INBOX", 3),
    );
    let undo = m.engine.history().undo_len();
    // UID 2 was flagged elsewhere, UID 3 expunged; 7–9 were never cached.
    let changed = vec![UidFlags {
        uid: 2,
        flags: Flags {
            seen: true,
            flagged: true,
            ..Flags::default()
        },
    }];
    let outcome = m.feed(qresynced(&m, changed, &[(7, 9), (3, 3)], 50));
    assert_eq!(outcome.receipt.data["flags_updated"], 1);
    assert_eq!(outcome.receipt.data["expunged"], 1);
    let state = m.engine.state();
    assert!(
        state.messages[&one].flags.seen,
        "neither listed nor vanished: kept as it was"
    );
    assert!(state.messages[&two].flags.flagged);
    assert!(!state.messages.contains_key(&three));
    assert_eq!(
        state.mailboxes[&m.mailbox("INBOX")].highest_modseq,
        Some(50)
    );
    assert_eq!(
        m.engine.history().undo_len(),
        undo,
        "an event, not undoable"
    );
}

#[test]
fn vanished_ranges_are_never_walked() {
    let mut m = cached();
    // Reversed and up to the last UID there is: only the cached UID 3 is in it.
    let outcome = m.feed(qresynced(&m, vec![], &[(u32::MAX, 3)], 51));
    assert_eq!(outcome.receipt.data["expunged"], 1);
    let outcome = m.feed(qresynced(&m, vec![], &[(1, u32::MAX)], 52));
    assert_eq!(outcome.receipt.data["expunged"], 2);
    let inbox = m.mailbox("INBOX");
    assert!(
        !m.engine
            .state()
            .messages
            .values()
            .any(|msg| msg.mailbox == inbox)
    );
}

#[test]
fn uid_ranges_merge_and_answer_membership() {
    let r = |first, last| UidRange { first, last };
    let merged = UidRange::merged([r(9, 7), r(1, 2), r(3, 4), r(8, 12), r(20, 20)]);
    assert_eq!(
        merged,
        [r(1, 4), r(7, 12), r(20, 20)],
        "sorted, joined, reversed read"
    );
    for (uid, inside) in [
        (0, false),
        (1, true),
        (4, true),
        (5, false),
        (12, true),
        (13, false),
    ] {
        assert_eq!(UidRange::any_contains(&merged, uid), inside, "{uid}");
    }
    let all = UidRange::merged([r(u32::MAX, 0)]);
    assert_eq!(all, [r(0, u32::MAX)]);
    assert!(UidRange::any_contains(&all, u32::MAX) && UidRange::any_contains(&all, 0));
    assert!(!UidRange::any_contains(&[], 1));
}

#[test]
fn existing_wins_over_vanished() {
    let mut m = cached();
    let Event::FlagsChanged {
        account,
        mailbox,
        uid_validity,
        from_uid,
        to_uid,
        flags,
        vanished,
        highest_modseq,
        ..
    } = qresynced(&m, vec![], &[], 53)
    else {
        unreachable!()
    };
    let outcome = m.feed(Event::FlagsChanged {
        account,
        mailbox,
        uid_validity,
        from_uid,
        to_uid,
        flags,
        existing: Some(vec![1]),
        vanished,
        highest_modseq,
    });
    assert_eq!(
        outcome.receipt.data["expunged"], 2,
        "UIDs 2 and 3 do not exist"
    );
}

#[test]
fn a_new_uidvalidity_forgets_the_modseq() {
    let mut m = cached();
    let event = flags_changed(&m, vec![], Some(vec![1, 2, 3]), Some(43));
    m.feed(event);
    let inbox = m.mailbox("INBOX");
    m.feed(Event::EnvelopesFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 8,
        uid_next: Some(2),
        total: 1,
        unread: 0,
        envelopes: vec![mail(1, "z@x", None, "New", true)],
    });
    assert_eq!(m.engine.state().mailboxes[&inbox].highest_modseq, None);
}
