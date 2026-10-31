//! CONDSTORE flag resync: the mailbox remembers HIGHESTMODSEQ from the last resync; the next
//! one asks for changes since then, and `existing` tells which unlisted UIDs are still there.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail};
use app_core::{Effect, Event, Flags, PersistDelta, UidFlags};

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
