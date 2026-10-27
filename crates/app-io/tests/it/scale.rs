//! Scale: the profile's redb cache and tantivy index with 50k messages, on this machine or in the
//! iOS simulator (`cargo xtask ios measure`, ROADMAP Wave 5 I3). Ignored by default (a
//! measurement, not a check of behaviour); run it in release mode:
//! `cargo test -p app-io --release --test it scale -- --ignored --nocapture`. Each step must stay
//! within a generous budget, so that a regression of an order of magnitude fails.

// reason: helpers outside `#[test]` fns may panic, and a measurement prints its results.
#![allow(clippy::unwrap_used, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use app_core::{
    AccountId, AppState, BodyState, Envelope, Flags, MailDelta, Mailbox, MailboxId, MailboxRole,
    MessageId, Thread, ThreadId,
};
use app_io::RedbStore;
use app_io::fulltext::FullText;

const MESSAGES: u128 = 50_000;
const BATCH: u128 = 500;

fn timed<T>(what: &str, budget: Duration, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = std::hint::black_box(f());
    let took = start.elapsed();
    eprintln!("{what}: {took:?} (budget {budget:?})");
    assert!(took <= budget, "{what} took {took:?}, budget {budget:?}");
    value
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-scale-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Bytes under `path`.
fn size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map_or(0, |m| m.len());
    }
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| size(&e.path()))
                .sum()
        })
        .unwrap_or(0)
}

fn id(n: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(n)
}

/// Messages `range` of one inbox, a thread every 4.
fn batch(account: AccountId, mailbox: MailboxId, range: std::ops::Range<u128>) -> MailDelta {
    let mut delta = MailDelta::default();
    for n in range {
        let thread = ThreadId::from_uuid(id(1_000_000 + n / 4));
        let message = MessageId::from_uuid(id(2_000_000 + n));
        delta.messages.push(Envelope {
            id: message,
            account,
            mailbox,
            uid: u32::try_from(n + 1).unwrap(),
            message_id_hdr: Some(format!("m{n}@scale.example")),
            in_reply_to: (n % 4 != 0).then(|| format!("m{}@scale.example", n - 1)),
            references: Vec::new(),
            thread,
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            subject: format!("Report {}", n / 4),
            date_ms: i64::try_from(n).unwrap() * 60_000,
            flags: Flags::default(),
            size: 1_500,
            has_attachments: false,
            snippet: Some(format!("Line one of message {n} about project {}", n % 97)),
            body: BodyState::NotFetched,
            moved_to: None,
        });
        if n % 4 == 3 {
            delta.threads.push(Thread {
                id: thread,
                account,
                messages: (n - 3..=n)
                    .map(|m| MessageId::from_uuid(id(2_000_000 + m)))
                    .collect(),
                subject: format!("Report {}", n / 4),
                last_date_ms: i64::try_from(n).unwrap() * 60_000,
            });
        }
    }
    delta
}

#[test]
#[ignore = "a measurement: run with --release --ignored"]
fn a_50k_redb_cache() {
    let dir = scratch("redb");
    let path = dir.join("state.redb");
    let account = AccountId::from_uuid(id(1));
    let mailbox = MailboxId::from_uuid(id(2));
    let store = RedbStore::open(&path).unwrap();
    store
        .apply_mail(&MailDelta {
            mailboxes: vec![Mailbox {
                id: mailbox,
                account,
                path: "INBOX".into(),
                delimiter: Some("/".into()),
                role: Some(MailboxRole::Inbox),
                uid_validity: Some(7),
                uid_next: None,
                unread: 0,
                total: 0,
                highest_modseq: None,
                sync_window: None,
                history_complete: false,
                last_fetch_ms: None,
                custom_keywords: None,
                identity: None,
            }],
            ..MailDelta::default()
        })
        .unwrap();
    let batches = MESSAGES / BATCH;
    timed(
        &format!("{batches} cache writes of {BATCH} envelopes"),
        Duration::from_secs(60),
        || {
            for b in 0..batches {
                store
                    .apply_mail(&batch(account, mailbox, b * BATCH..(b + 1) * BATCH))
                    .unwrap();
            }
        },
    );
    drop(store);
    eprintln!("redb file: {} KiB", size(&path) / 1024);
    let state = timed("reopen and load 50k", Duration::from_secs(10), || {
        let store = RedbStore::open(&path).unwrap();
        let mut state = AppState::default();
        store.load_into(&mut state).unwrap();
        state
    });
    assert_eq!(state.messages.len(), 50_000);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "a measurement: run with --release --ignored"]
fn a_50k_full_text_index() {
    let dir = scratch("index");
    let index = FullText::open(&dir).unwrap();
    timed("index 50k messages", Duration::from_secs(120), || {
        for n in 0..MESSAGES {
            let raw = format!(
                "Message-ID: <m{n}@scale.example>\r\nFrom: Sender {s} <s{s}@scale.example>\r\n\
                 To: alice@example.com\r\nSubject: Report {t}\r\n\r\n\
                 Line one of message {n} about project p{p}.\r\nBudget, schedule and review.\r\n",
                s = n % 50,
                t = n / 4,
                p = n % 97,
            );
            index
                .add(MessageId::from_uuid(id(2_000_000 + n)), raw.as_bytes())
                .unwrap();
            if n % 5_000 == 4_999 {
                index.commit().unwrap();
            }
        }
        index.commit().unwrap();
    });
    eprintln!("index dir: {} KiB", size(&dir) / 1024);
    let hits = timed("search a rare word", Duration::from_secs(1), || {
        index.search(&["p42".to_owned()], 1_000).unwrap()
    });
    assert!(!hits.is_empty());
    timed("search a common word", Duration::from_secs(2), || {
        index.search(&["budget".to_owned()], 200).unwrap()
    });
    drop(index);
    timed("reopen the index", Duration::from_secs(5), || {
        FullText::open(&dir).unwrap()
    });
    std::fs::remove_dir_all(dir).unwrap();
}
