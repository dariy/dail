//! Automatic periodic sync (UX-6): refresh on open, the open mailbox in the poll, full rounds
//! and the back-off after failures.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, T0, names};
use app_core::{Command, Ctx, Effect, Event, MailboxId, Origin, Outcome};

fn at(m: &mut Mail, ms: i64) -> Ctx {
    m.seed += 1;
    Ctx {
        now_ms: ms,
        id_seed: m.seed << 64,
        origin: Origin::Cli,
    }
}

fn open(m: &mut Mail, id: MailboxId, ms: i64) -> Outcome {
    let ctx = at(m, ms);
    m.engine
        .dispatch(Command::OpenMailbox { id }, &ctx)
        .unwrap()
}

fn tick(m: &mut Mail, ms: i64) -> Outcome {
    let ctx = at(m, ms);
    m.engine.ingest(Event::Tick { now_ms: ms }, &ctx).unwrap()
}

fn fetched_paths(outcome: &Outcome) -> Vec<String> {
    outcome
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::FetchEnvelopes { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

const PATHS: [&str; 4] = ["INBOX", "Archive", "Trash", "Sent"];

fn fetch_all(m: &mut Mail) {
    for path in PATHS {
        m.fetch(path, Vec::new());
    }
}

/// Hostinger, idle, after a `sync_now` at `T0 + 10 s` whose full round was answered.
fn synced() -> Mail {
    let mut m = Mail::hostinger();
    fetch_all(&mut m);
    let ctx = at(&mut m, T0 + 10_000);
    m.engine
        .dispatch(Command::SyncNow { account: None }, &ctx)
        .unwrap();
    m.feed(Event::MailboxesListed {
        account: m.account,
        mailboxes: PATHS
            .iter()
            .map(|p| app_core::RemoteMailbox {
                path: (*p).into(),
                delimiter: Some("/".into()),
                special_use: None,
                selectable: true,
            })
            .collect(),
    });
    fetch_all(&mut m);
    assert!(!m.engine.state().status.sync[&m.account].is_syncing());
    m
}

#[test]
fn opening_a_stale_mailbox_fetches_it_once() {
    let mut m = Mail::hostinger();
    m.fetch("Archive", Vec::new());
    let archive = m.mailbox("Archive");
    let fetched_at = m.engine.state().mailboxes[&archive].last_fetch_ms.unwrap();
    // Fresh: nothing.
    let fresh = open(&mut m, archive, fetched_at + 59_000);
    assert!(fresh.effects.is_empty());
    assert_eq!(fresh.receipt.data["fetching"], false);
    // Stale: one fetch, and not a second one while it is in flight.
    let stale = open(&mut m, archive, fetched_at + 60_000);
    assert_eq!(fetched_paths(&stale), ["Archive"]);
    assert!(m.engine.state().status.fetching.contains(&archive));
    let again = open(&mut m, archive, fetched_at + 61_000);
    assert!(again.effects.is_empty());
    // Never fetched (its first fetch failed): always.
    let trash = m.mailbox("Trash");
    m.feed(Event::SyncFailed {
        account: m.account,
        mailbox: Some(trash),
        error: "timeout".into(),
    });
    assert_eq!(fetched_paths(&open(&mut m, trash, T0)), ["Trash"]);
}

#[test]
fn the_poll_fetches_the_open_mailbox_and_full_rounds_come_every_full_sync_secs() {
    let mut m = synced();
    let archive = m.mailbox("Archive");
    open(&mut m, archive, T0 + 11_000);
    let poll = m.engine.state().status.next_poll_ms.unwrap();
    let tick1 = tick(&mut m, poll);
    // INBOX is watched (IMAP IDLE): only the open mailbox.
    assert_eq!(fetched_paths(&tick1), ["Archive"]);
    assert_eq!(names(&tick1.effects).last(), Some(&"schedule_tick"));
    m.fetch("Archive", Vec::new());
    // 900 s after the sync_now: a full round instead.
    let full = tick(&mut m, T0 + 10_000 + 900_000);
    assert!(
        names(&full.effects).contains(&"list_mailboxes"),
        "{:?}",
        names(&full.effects)
    );
    assert!(fetched_paths(&full).is_empty());
    assert_eq!(
        m.engine.state().status.sync[&m.account].last_full_ms,
        Some(T0 + 910_000)
    );
}

#[test]
fn failures_back_off_and_a_success_resets_them() {
    let mut m = synced();
    let account = m.account;
    let archive = m.mailbox("Archive");
    open(&mut m, archive, T0 + 11_000);
    // Two failures in a row: the next poll waits 2 × poll_secs after the last one.
    for _ in 0..2 {
        m.feed(Event::SyncFailed {
            account,
            mailbox: None,
            error: "network unreachable".into(),
        });
    }
    let status = &m.engine.state().status.sync[&account];
    assert_eq!(status.failures, 2);
    let failed_at = status.last_error.as_ref().unwrap().at_ms;
    let poll = m.engine.state().status.next_poll_ms.unwrap();
    assert!(poll < failed_at + 240_000);
    let waiting = tick(&mut m, poll.max(failed_at + 1));
    assert!(fetched_paths(&waiting).is_empty(), "backing off");
    let later = tick(&mut m, failed_at + 240_000);
    assert_eq!(fetched_paths(&later), ["Archive"]);
    m.fetch("Archive", Vec::new());
    assert_eq!(m.engine.state().status.sync[&account].failures, 0);
}
