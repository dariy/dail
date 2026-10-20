//! History backfill: `set_sync_window` and the `FetchOlder` batches that fill a mailbox's
//! window, one at a time, until the oldest message is cached.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail, names};
use app_core::{Command, CoreError, Effect, Event, MailboxId, RemoteEnvelope, listing};

/// INBOX whose server has 450 messages; UIDs 448–450 are cached.
fn partial() -> (Mail, MailboxId) {
    let mut m = Mail::hostinger();
    let inbox = m.mailbox("INBOX");
    m.feed(Event::EnvelopesFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        uid_next: Some(451),
        total: 450,
        unread: 0,
        envelopes: (448..=450)
            .map(|uid| mail(uid, &format!("m{uid}@x"), None, "Hi", true))
            .collect(),
    });
    (m, inbox)
}

fn older_of(effects: &[Effect]) -> Option<(u32, u32)> {
    effects.iter().find_map(|e| match e {
        Effect::FetchOlder {
            before_uid, limit, ..
        } => Some((*before_uid, *limit)),
        _ => None,
    })
}

fn batch(m: &Mail, inbox: MailboxId, uids: std::ops::RangeInclusive<u32>, complete: bool) -> Event {
    let envelopes: Vec<RemoteEnvelope> = uids
        .map(|uid| mail(uid, &format!("m{uid}@x"), None, "Hi", true))
        .collect();
    Event::OlderFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        envelopes,
        complete,
    }
}

#[test]
fn a_window_of_everything_backfills_in_batches() {
    let (mut m, inbox) = partial();
    // The default window (the setting, 500 here) asks for more than the 3 cached as well.
    assert!(m.engine.state().status.backfilling.contains(&inbox));
    let outcome = m
        .run(Command::SetSyncWindow {
            mailbox: inbox,
            window: Some(0),
        })
        .unwrap();
    assert_eq!(
        older_of(&outcome.effects),
        None,
        "a batch is in flight already"
    );
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Keep all of INBOX offline")
    );

    let next = m.feed(batch(&m, inbox, 248..=447, false));
    assert_eq!(older_of(&next.effects), Some((248, 200)));
    let history = listing::history(m.engine.state(), inbox).unwrap();
    assert_eq!((history.cached, history.wanted), (203, 450));
    let next = m.feed(batch(&m, inbox, 48..=247, false));
    assert_eq!(older_of(&next.effects), Some((48, 47)), "only what is left");
    let last = m.feed(batch(&m, inbox, 1..=47, true));
    assert_eq!(older_of(&last.effects), None);
    let state = m.engine.state();
    assert!(state.mailboxes[&inbox].history_complete);
    assert!(state.status.backfilling.is_empty());
    assert_eq!(listing::history(state, inbox).unwrap().cached, 450);
}

#[test]
fn a_small_window_stops_early_and_undo_restores_it() {
    let (mut m, inbox) = partial();
    m.feed(Event::BackfillFailed {
        account: m.account,
        mailbox: inbox,
        error: "connection reset".into(),
    });
    assert!(m.engine.state().status.backfilling.is_empty());
    let outcome = m
        .run(Command::SetSyncWindow {
            mailbox: inbox,
            window: Some(10),
        })
        .unwrap();
    assert_eq!(
        older_of(&outcome.effects),
        Some((448, 7)),
        "only what the window lacks"
    );
    m.feed(batch(&m, inbox, 441..=447, false));
    assert!(
        m.engine.state().status.backfilling.is_empty(),
        "the window is full"
    );
    m.run(Command::Undo {}).unwrap();
    assert_eq!(m.engine.state().mailboxes[&inbox].sync_window, None);
    m.run(Command::Redo {}).unwrap();
    assert_eq!(m.engine.state().mailboxes[&inbox].sync_window, Some(10));
}

#[test]
fn a_failure_waits_for_the_next_fetch() {
    let (mut m, inbox) = partial();
    let failed = m.feed(Event::BackfillFailed {
        account: m.account,
        mailbox: inbox,
        error: "connection reset".into(),
    });
    assert!(failed.effects.is_empty());
    assert_eq!(failed.receipt.data["error"], "connection reset");
    let fetch = m.feed(Event::EnvelopesFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        uid_next: Some(451),
        total: 450,
        unread: 0,
        envelopes: vec![],
    });
    assert!(names(&fetch.effects).contains(&"fetch_older"));
}

#[test]
fn stale_batches_and_unknown_mailboxes() {
    let (mut m, inbox) = partial();
    let mut stale = batch(&m, inbox, 440..=447, false);
    if let Event::OlderFetched { uid_validity, .. } = &mut stale {
        *uid_validity = 8;
    }
    let outcome = m.feed(stale);
    assert!(outcome.effects.is_empty());
    assert_eq!(listing::history(m.engine.state(), inbox).unwrap().cached, 3);
    let err = m
        .run(Command::SetSyncWindow {
            mailbox: MailboxId::from_uuid(uuid::Uuid::from_u128(99)),
            window: Some(0),
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }));
    // An empty batch means there is nothing older.
    m.run(Command::SetSyncWindow {
        mailbox: inbox,
        window: Some(0),
    })
    .unwrap();
    m.feed(Event::OlderFetched {
        account: m.account,
        mailbox: inbox,
        uid_validity: 7,
        envelopes: Vec::new(),
        complete: false,
    });
    assert!(m.engine.state().mailboxes[&inbox].history_complete);
}
