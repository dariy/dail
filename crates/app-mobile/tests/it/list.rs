//! The thread list: densities, the window over a large mailbox, pull to refresh, empty states
//! and the account tags.

// reason: a measurement prints its results.
#![allow(clippy::print_stderr)]
// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use app_core::{Addr, Command, Ctx, Event, Flags, Origin, ProviderPreset, RemoteEnvelope};
use app_mobile::UiEvent;
use app_mobile::frame::Density;

use crate::support::{NOW, Phone, inbox_of};

#[test]
fn the_density_follows_the_setting() {
    for (value, density) in [
        (0, Density::Cozy),
        (1, Density::Compact),
        (2, Density::Preview),
    ] {
        let mut phone = Phone::inbox(360, 800);
        phone.dispatch(Command::SetSetting {
            key: app_core::SettingKey::ListDensity,
            value: serde_json::json!(value),
        });
        assert_eq!(phone.frame().list.unwrap().density, density);
    }
}

#[test]
fn refreshing_shows_while_a_sync_runs() {
    let mut phone = Phone::inbox(360, 800);
    assert!(!phone.frame().list.unwrap().refreshing);
    phone.event(UiEvent::Refresh);
    assert!(phone.frame().list.unwrap().refreshing);
}

#[test]
fn empty_states() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::SetSearch {
        query: "nothing matches this".to_owned(),
    });
    let list = phone.frame().list.unwrap();
    assert_eq!(list.total, 0);
    assert_eq!(list.empty.as_deref(), Some("No matching conversations."));
}

#[test]
fn rows_carry_the_account_tag_with_two_accounts() {
    let mut phone = Phone::inbox(360, 800);
    assert!(
        phone
            .frame()
            .list
            .unwrap()
            .rows
            .iter()
            .all(|r| r.tag.is_none())
    );
    phone.dispatch(Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "bob@gmail.com".to_owned(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    });
    assert!(
        phone
            .frame()
            .list
            .unwrap()
            .rows
            .iter()
            .all(|r| r.tag == Some(0))
    );
}

/// The inbox with `count` more threads, one message each.
fn large(count: u32) -> Phone {
    let mut phone = Phone::inbox(360, 800);
    let (inbox, _) = inbox_of(phone.state());
    let mailbox = &phone.state().mailboxes[&inbox];
    let (account, uid_validity) = (mailbox.account, mailbox.uid_validity.unwrap_or(1));
    let envelopes = (100..100 + count)
        .map(|uid| RemoteEnvelope {
            uid,
            message_id_hdr: Some(format!("m{uid}@x")),
            in_reply_to: None,
            references: Vec::new(),
            from: vec![Addr {
                name: Some("Bob".into()),
                email: "bob@example.org".into(),
            }],
            to: Vec::new(),
            cc: Vec::new(),
            subject: format!("Message {uid}"),
            date_ms: 1_699_000_000_000 + i64::from(uid) * 1000,
            flags: Flags::default(),
            size: 1000,
            has_attachments: false,
        })
        .collect();
    let ctx = Ctx {
        now_ms: NOW,
        id_seed: 7 << 64,
        origin: Origin::Runtime,
    };
    phone
        .engine
        .ingest(
            Event::EnvelopesFetched {
                account,
                mailbox: inbox,
                uid_validity,
                uid_next: Some(100 + count),
                total: count + 2,
                unread: count,
                envelopes,
            },
            &ctx,
        )
        .unwrap();
    phone
}

#[test]
fn the_window_follows_the_scrolling() {
    let mut phone = large(2_000);
    let list = phone.frame().list.unwrap();
    assert_eq!(list.total, 2_002);
    assert_eq!(list.offset, 0);
    assert_eq!(list.rows.len(), 50);
    phone.event(UiEvent::Scrolled {
        first: 1_000,
        count: 12,
    });
    let list = phone.frame().list.unwrap();
    assert_eq!(list.offset, 980);
    assert_eq!(list.rows.len(), 52);
    assert_eq!(list.rows[20].subject, "Message 1101");
}

#[test]
#[ignore = "a measurement: run with --release --ignored"]
fn scale_a_frame_over_100k_threads() {
    let mut phone = large(100_000);
    let state = phone.state().clone();
    let start = Instant::now();
    let _ = phone.mobile.frame(&state);
    let first = start.elapsed();
    let start = Instant::now();
    for _ in 0..10 {
        let frame = phone.mobile.frame(&state);
        assert!(frame.list.unwrap().rows.len() <= 50);
    }
    let frame = start.elapsed() / 10;
    let budget = Duration::from_millis(8);
    eprintln!("first frame (the listing): {first:?}; later frames: {frame:?} (budget {budget:?})");
    assert!(frame <= budget, "a frame took {frame:?}");
}
