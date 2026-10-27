//! Pinned threads (MISC-4): `pin_threads` / `unpin_threads`, the list order and the keyword
//! `$Pinned`, kept on the device where the server keeps no keywords.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, mail, of_kind};
use app_core::{Command, Effect, Event, FlagKind, Flags, RemoteStep, ThreadId, UidFlags, listing};

/// INBOX with three threads, oldest first: UIDs 1 (`a`), 2 (`b`), 3 (`c`).
fn inbox() -> (Mail, [ThreadId; 3]) {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "Old", true),
            mail(2, "b@x", None, "Middle", true),
            mail(3, "c@x", None, "New", true),
        ],
    );
    let threads = [1, 2, 3].map(|uid| {
        let message = m.message("INBOX", uid);
        m.thread_of(message)
    });
    (m, threads)
}

fn order(m: &Mail) -> Vec<ThreadId> {
    let inbox = m.mailbox("INBOX");
    listing::threads_in(m.engine.state(), inbox)
        .iter()
        .map(|t| t.id)
        .collect()
}

#[test]
fn pinned_threads_come_first_newest_first_and_undo_unpins() {
    let (mut m, [old, middle, new]) = inbox();
    assert_eq!(order(&m), [new, middle, old]);
    let outcome = m
        .run(Command::PinThreads {
            ids: vec![old],
            messages: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Pin (1 message)")
    );
    let Effect::RunRemoteOp { steps, .. } = of_kind(&outcome.effects, "run_remote_op")[0] else {
        panic!()
    };
    assert!(matches!(
        &steps[..],
        [RemoteStep::Store {
            flag: FlagKind::Pinned,
            value: true,
            ..
        }]
    ));
    assert_eq!(order(&m), [old, new, middle]);
    m.run(Command::PinThreads {
        ids: vec![middle],
        messages: Vec::new(),
    })
    .unwrap();
    assert_eq!(order(&m), [middle, old, new], "pinned ones newest first");
    let listed = m
        .run(Command::ListThreads {
            mailbox: m.mailbox("INBOX"),
            offset: 0,
            limit: 50,
        })
        .unwrap();
    assert_eq!(listed.receipt.data["threads"][0]["pinned"], true);
    m.run(Command::Undo {}).unwrap();
    m.run(Command::UnpinThreads {
        ids: vec![old],
        messages: Vec::new(),
    })
    .unwrap();
    assert_eq!(order(&m), [new, middle, old]);
}

#[test]
fn a_server_without_keywords_keeps_the_pin_on_the_device() {
    let (mut m, [old, _, _]) = inbox();
    let outcome = m
        .run(Command::PinThreads {
            ids: vec![old],
            messages: Vec::new(),
        })
        .unwrap();
    let Effect::RunRemoteOp { op, account, .. } =
        of_kind(&outcome.effects, "run_remote_op")[0].clone()
    else {
        panic!()
    };
    m.feed(Event::KeywordsRefused {
        account,
        path: "INBOX".into(),
    });
    m.feed(Event::RemoteOpSucceeded { op });
    let inbox = m.mailbox("INBOX");
    assert_eq!(
        m.engine.state().mailboxes[&inbox].custom_keywords,
        Some(false)
    );
    // The server's flags (no `$Pinned`) don't take the pin away.
    m.feed(Event::FlagsChanged {
        account,
        mailbox: inbox,
        uid_validity: 7,
        from_uid: 1,
        to_uid: 3,
        flags: (1..=3)
            .map(|uid| UidFlags {
                uid,
                flags: Flags {
                    seen: true,
                    ..Flags::default()
                },
            })
            .collect(),
        existing: None,
        highest_modseq: None,
    });
    assert_eq!(order(&m)[0], old, "still pinned");
    // Unpinning there needs no server step: no op runs.
    let unpinned = m
        .run(Command::UnpinThreads {
            ids: vec![old],
            messages: Vec::new(),
        })
        .unwrap();
    assert!(of_kind(&unpinned.effects, "run_remote_op").is_empty());
    assert!(m.engine.state().outbox.is_empty());
}
