//! Push for the INBOX: `WatchMailbox` after the first INBOX fetch, a fetch on every reported
//! change, a retry after a failure, polling for servers without IDLE.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, T0, ctx, mail, names};
use app_core::{Command, Effect, Event, WatchState};

fn watch_state(m: &Mail) -> WatchState {
    m.engine.state().status.sync[&m.account].watch.clone()
}

/// A Hostinger account whose INBOX has been fetched once (so it is watched).
fn watched() -> Mail {
    let mut m = Mail::hostinger();
    let outcome = m.fetch("INBOX", vec![mail(1, "a@x", None, "Plans", true)]);
    assert_eq!(
        names(&outcome.effects).last(),
        Some(&"watch_mailbox"),
        "{:?}",
        names(&outcome.effects)
    );
    assert_eq!(watch_state(&m), WatchState::Watching);
    // The rest of the round, so that the account is idle.
    for path in ["Archive", "Trash", "Sent"] {
        m.fetch(path, vec![]);
    }
    assert!(!m.engine.state().status.sync[&m.account].is_syncing());
    m
}

#[test]
fn the_first_inbox_fetch_starts_one_watch() {
    let mut m = watched();
    // Later fetches and other mailboxes don't start another one.
    let again = m.fetch("INBOX", vec![mail(2, "b@x", None, "Lunch", true)]);
    assert!(!names(&again.effects).contains(&"watch_mailbox"));
    let archive = m.fetch("Archive", vec![]);
    assert!(!names(&archive.effects).contains(&"watch_mailbox"));
}

#[test]
fn a_change_fetches_the_inbox_and_watches_again() {
    let mut m = watched();
    let inbox = m.mailbox("INBOX");
    let changed = m.feed(Event::MailboxWatched {
        account: m.account,
        mailbox: inbox,
        changed: true,
    });
    assert_eq!(
        names(&changed.effects),
        ["fetch_envelopes", "watch_mailbox"]
    );
    let Effect::FetchEnvelopes { since_uid, .. } = &changed.effects[0] else {
        unreachable!()
    };
    assert_eq!(*since_uid, Some(2), "incremental");
    // A quiet timeout only watches again.
    let quiet = m.feed(Event::MailboxWatched {
        account: m.account,
        mailbox: inbox,
        changed: false,
    });
    assert_eq!(names(&quiet.effects), ["watch_mailbox"]);
    assert_eq!(watch_state(&m), WatchState::Watching);
}

#[test]
fn a_failed_watch_is_retried_after_a_minute() {
    let mut m = watched();
    let inbox = m.mailbox("INBOX");
    let failed = m
        .engine
        .ingest(
            Event::WatchFailed {
                account: m.account,
                mailbox: inbox,
                error: "connection reset".into(),
                unsupported: false,
            },
            &ctx(10),
        )
        .unwrap();
    let retry_at = T0 + 10_000 + app_core::engine::WATCH_RETRY_MS;
    assert_eq!(failed.effects, [Effect::ScheduleTick { at_ms: retry_at }]);
    assert_eq!(
        watch_state(&m),
        WatchState::Retrying {
            at_ms: retry_at,
            error: "connection reset".into()
        }
    );
    let early = m
        .engine
        .ingest(
            Event::Tick {
                now_ms: retry_at - 1,
            },
            &ctx(11),
        )
        .unwrap();
    assert!(!names(&early.effects).contains(&"watch_mailbox"));
    let due = m
        .engine
        .ingest(Event::Tick { now_ms: retry_at }, &ctx(12))
        .unwrap();
    assert!(names(&due.effects).contains(&"watch_mailbox"));
    assert_eq!(watch_state(&m), WatchState::Watching);
}

#[test]
fn without_idle_the_inbox_is_polled() {
    let mut m = watched();
    let inbox = m.mailbox("INBOX");
    let start = m
        .engine
        .dispatch(Command::SyncNow { account: None }, &ctx(20))
        .unwrap();
    let Some(Effect::ScheduleTick { at_ms: poll_at }) = start
        .effects
        .iter()
        .find(|e| matches!(e, Effect::ScheduleTick { .. }))
        .cloned()
    else {
        unreachable!("{:?}", names(&start.effects))
    };
    // Finish the round that sync_now started.
    m.engine
        .ingest(
            Event::SyncFailed {
                account: m.account,
                mailbox: None,
                error: "offline".into(),
            },
            &ctx(21),
        )
        .unwrap();
    // Watched: the poll leaves the INBOX alone.
    let polled = m
        .engine
        .ingest(Event::Tick { now_ms: poll_at }, &ctx(22))
        .unwrap();
    assert!(!names(&polled.effects).contains(&"fetch_envelopes"));

    m.feed(Event::WatchFailed {
        account: m.account,
        mailbox: inbox,
        error: "the server does not support IDLE".into(),
        unsupported: true,
    });
    assert_eq!(watch_state(&m), WatchState::Unsupported);
    let next_poll = m.engine.state().status.next_poll_ms.unwrap();
    let polled = m
        .engine
        .ingest(Event::Tick { now_ms: next_poll }, &ctx(23))
        .unwrap();
    assert!(names(&polled.effects).contains(&"fetch_envelopes"));
    assert!(
        !names(&polled.effects).contains(&"watch_mailbox"),
        "not retried"
    );
}

#[test]
fn watches_of_removed_accounts_are_ignored() {
    let mut m = watched();
    let (account, inbox) = (m.account, m.mailbox("INBOX"));
    m.run(Command::RemoveAccount {
        id: account,
        confirm: true,
    })
    .unwrap();
    let late = m.feed(Event::MailboxWatched {
        account,
        mailbox: inbox,
        changed: true,
    });
    assert!(late.effects.is_empty());
    let failed = m.feed(Event::WatchFailed {
        account,
        mailbox: inbox,
        error: "gone".into(),
        unsupported: false,
    });
    assert!(failed.effects.is_empty());
}
