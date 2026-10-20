//! Shared helpers: a synced state of the `basic` fixture.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{AppState, MailboxId, MailboxRole, ThreadId, listing};
use app_testkit::Fixture;

/// A fixed "now": 2023-11-15 08:00 UTC, the day after the fixture's mail.
pub const NOW: i64 = 1_700_035_200_000;

/// alice@example.com's account, added and synced.
pub fn synced_state() -> AppState {
    app_testkit::synced_state(Fixture::builtin("basic").unwrap()).unwrap()
}

/// The inbox and its threads, newest first.
pub fn inbox_of(state: &AppState) -> (MailboxId, Vec<ThreadId>) {
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Inbox))
        .unwrap()
        .id;
    let threads = listing::threads_in(state, inbox)
        .iter()
        .map(|t| t.id)
        .collect();
    (inbox, threads)
}
