//! `open_unified_inbox`: every account's inbox in one list.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, ctx, mail};
use app_core::{AccountId, Command, Event, MailboxRole, ProviderPreset, RemoteMailbox, listing};

#[test]
fn the_unified_inbox_lists_every_accounts_inbox() {
    let mut m = Mail::hostinger();
    m.fetch("INBOX", vec![mail(1, "a@x", None, "First account", true)]);
    m.fetch("Archive", vec![mail(1, "z@x", None, "Archived", true)]);
    // A second account with an inbox of its own.
    let added = m
        .engine
        .dispatch(
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "two@gmail.com".into(),
                display_name: None,
                password: None,
                imap: None,
                smtp: None,
            },
            &ctx(40),
        )
        .unwrap();
    let second: AccountId = serde_json::from_value(added.receipt.data["id"].clone()).unwrap();
    m.engine
        .ingest(
            Event::MailboxesListed {
                account: second,
                mailboxes: vec![RemoteMailbox {
                    path: "INBOX".into(),
                    delimiter: Some("/".into()),
                    special_use: None,
                    selectable: true,
                }],
            },
            &ctx(41),
        )
        .unwrap();
    let inbox2 = m
        .engine
        .state()
        .mailboxes
        .values()
        .find(|mb| mb.account == second && mb.role == Some(MailboxRole::Inbox))
        .unwrap()
        .id;
    m.engine
        .ingest(
            Event::EnvelopesFetched {
                account: second,
                mailbox: inbox2,
                uid_validity: 3,
                uid_next: Some(2),
                total: 1,
                unread: 0,
                envelopes: vec![mail(1, "b@x", None, "Second account", true)],
            },
            &ctx(42),
        )
        .unwrap();

    m.run(Command::OpenUnifiedInbox {}).unwrap();
    let state = m.engine.state();
    assert!(state.view.unified && state.view.mailbox.is_none());
    let subjects: Vec<&str> = listing::visible_threads(state)
        .iter()
        .map(|t| state.threads[t].subject.as_str())
        .collect();
    assert_eq!(subjects.len(), 2, "{subjects:?}");
    assert!(subjects.contains(&"First account") && subjects.contains(&"Second account"));
    assert!(!subjects.contains(&"Archived"));

    let archive = m.mailbox("Archive");
    m.run(Command::OpenMailbox { id: archive }).unwrap();
    assert!(!m.engine.state().view.unified);
}
