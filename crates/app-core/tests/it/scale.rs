//! Scale: a 100k-message mailbox through the engine. Ignored by default (it is a measurement,
//! not a check of behaviour); run it in release mode:
//! `cargo test -p app-core --release --test it scale -- --ignored --nocapture`. Each step must
//! stay within a generous budget, so that a regression of an order of magnitude fails.

// reason: helpers outside `#[test]` fns may panic, and a measurement prints its results.
#![allow(clippy::unwrap_used, clippy::print_stderr)]

use std::time::{Duration, Instant};

use crate::support::{Mail, mail};
use app_core::{Command, Event, listing};

const MESSAGES: u32 = 100_000;

fn timed<T>(what: &str, budget: Duration, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = std::hint::black_box(f());
    let took = start.elapsed();
    eprintln!("{what}: {took:?} (budget {budget:?})");
    assert!(took <= budget, "{what} took {took:?}, budget {budget:?}");
    value
}

#[test]
#[ignore = "a measurement: run with --release --ignored"]
fn a_100k_mailbox() {
    let mut m = Mail::hostinger();
    let inbox = m.mailbox("INBOX");
    // Every 4th message is a reply to the one before it.
    let envelopes = |uids: std::ops::RangeInclusive<u32>| {
        uids.map(|uid| {
            let parent = (uid % 4 == 0).then(|| format!("m{}@x", uid - 1));
            mail(
                uid,
                &format!("m{uid}@x"),
                parent.as_deref(),
                &format!("Subject {}", uid / 4),
                true,
            )
        })
        .collect::<Vec<_>>()
    };
    let newest = envelopes(MESSAGES / 2 + 1..=MESSAGES);
    timed("first fetch of 50k", Duration::from_secs(2), || {
        m.feed(Event::EnvelopesFetched {
            account: m.account,
            mailbox: inbox,
            uid_validity: 7,
            uid_next: Some(MESSAGES + 1),
            total: MESSAGES,
            unread: 0,
            envelopes: newest,
        })
    });
    timed(
        "250 backfill batches of 200",
        Duration::from_secs(40),
        || {
            for batch in 0..250_u32 {
                let high = MESSAGES / 2 - batch * 200;
                m.feed(Event::OlderFetched {
                    account: m.account,
                    mailbox: inbox,
                    uid_validity: 7,
                    envelopes: envelopes(high - 199..=high),
                    complete: high == 200,
                });
            }
        },
    );
    assert_eq!(
        listing::history(m.engine.state(), inbox).unwrap().cached,
        MESSAGES
    );
    let account = m.account;
    timed(
        "indexing 100k messages for threading",
        Duration::from_secs(2),
        || app_core::threading::Threader::new(account, m.engine.state().messages.values()),
    );
    timed(
        "the history of the mailbox",
        Duration::from_millis(500),
        || listing::history(m.engine.state(), inbox),
    );
    m.run(Command::OpenMailbox { id: inbox }).unwrap();
    let threads = timed("visible threads", Duration::from_millis(500), || {
        listing::visible_threads(m.engine.state())
    });
    assert!(threads.len() > MESSAGES as usize / 2);
    timed("summaries of one screen", Duration::from_millis(50), || {
        threads
            .iter()
            .take(50)
            .map(|t| {
                listing::ThreadSummary::of(m.engine.state(), &m.engine.state().threads[t]).unread
            })
            .sum::<usize>()
    });
    timed(
        "an incremental fetch of 10",
        Duration::from_millis(500),
        || {
            m.feed(Event::EnvelopesFetched {
                account: m.account,
                mailbox: inbox,
                uid_validity: 7,
                uid_next: Some(MESSAGES + 11),
                total: MESSAGES + 10,
                unread: 0,
                envelopes: envelopes(MESSAGES + 1..=MESSAGES + 10),
            })
        },
    );
}
