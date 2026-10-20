//! Scale: the three panes with a 100k-message mailbox open. Ignored by default (a
//! measurement); run it in release mode:
//! `cargo test -p app-gui --release --test it scale -- --ignored --nocapture`. The thread list
//! lays out only its visible rows, so a frame must stay well within the budget.

// reason: a measurement prints its results.
#![allow(clippy::print_stderr)]

use std::time::{Duration, Instant};

use crate::support::{apply, gui, inbox_of, ingest, synced_state};
use app_core::{Addr, Command, Event, Flags, RemoteEnvelope};
use egui_kittest::kittest::Queryable;

#[test]
#[ignore = "a measurement: run with --release --ignored"]
fn a_frame_with_100k_messages() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let account = state.mailboxes[&inbox].account;
    let uid_validity = state.mailboxes[&inbox].uid_validity.unwrap_or(1);
    let mut h = gui(state);
    let frame = |h: &mut egui_kittest::Harness<'static, crate::support::Gui>| {
        let start = Instant::now();
        for _ in 0..10 {
            h.step();
        }
        start.elapsed() / 10
    };
    apply(&mut h, Command::OpenMailbox { id: inbox });
    let baseline = frame(&mut h);
    let envelopes = (100..100_100_u32)
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
    ingest(
        &mut h,
        Event::EnvelopesFetched {
            account,
            mailbox: inbox,
            uid_validity,
            uid_next: Some(100_100),
            total: 100_002,
            unread: 100_000,
            envelopes,
        },
    );
    h.get_by_label("Message 100099");
    let budget = Duration::from_millis(16);
    let large = frame(&mut h);
    let rows = Instant::now();
    let count = app_core::listing::visible_threads(h.state().engine.state()).len();
    let rows = rows.elapsed();
    eprintln!(
        "one frame: {baseline:?} with 2 threads, {large:?} with {count} (budget {budget:?}); \
         the list projection alone takes {rows:?}"
    );
    assert!(large <= budget, "a frame took {large:?}");
}
