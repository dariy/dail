//! Sending addresses: aliases with names and signatures, the From of replies and of drafts
//! from a mailbox, and the From of the sent message.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, T0, ctx, mail};
use app_core::{Addr, Command, CoreError, DraftId, Effect, Event, Origin, RemoteEnvelope};

fn alias(m: &mut Mail) {
    m.run(Command::SetIdentity {
        account: m.account,
        email: "sales@example.com".into(),
        name: Some("Sales".into()),
        signature: Some("Sales team\nExample Ltd.".into()),
    })
    .unwrap();
}

fn draft_of(outcome: &app_core::Outcome) -> DraftId {
    serde_json::from_value(outcome.receipt.data["id"].clone()).unwrap()
}

#[test]
fn identities_are_kept_with_the_account_and_undone() {
    let mut m = Mail::hostinger();
    alias(&mut m);
    let account = &m.engine.state().accounts[&m.account];
    assert_eq!(account.identities.len(), 1);
    assert_eq!(
        account
            .identity("SALES@example.com")
            .unwrap()
            .name
            .as_deref(),
        Some("Sales")
    );
    m.run(Command::Undo {}).unwrap();
    assert!(m.engine.state().accounts[&m.account].identities.is_empty());
    m.run(Command::Redo {}).unwrap();
    m.run(Command::RemoveIdentity {
        account: m.account,
        email: "sales@example.com".into(),
    })
    .unwrap();
    assert!(m.engine.state().accounts[&m.account].identities.is_empty());
    assert!(matches!(
        m.run(Command::SetIdentity {
            account: m.account,
            email: "not an address".into(),
            name: None,
            signature: None,
        }),
        Err(CoreError::InvalidArgument { .. })
    ));
    // Agents can't add addresses to send from.
    let agent = app_core::Ctx {
        origin: Origin::Mcp,
        ..ctx(90)
    };
    let err = m
        .engine
        .dispatch(
            Command::SetIdentity {
                account: m.account,
                email: "evil@example.com".into(),
                name: None,
                signature: None,
            },
            &agent,
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }));
}

#[test]
fn a_reply_goes_out_from_the_address_the_mail_came_to() {
    let mut m = Mail::hostinger();
    alias(&mut m);
    let to_sales = RemoteEnvelope {
        to: vec![Addr {
            name: None,
            email: "sales@example.com".into(),
        }],
        ..mail(1, "q@x", None, "Quote?", false)
    };
    m.fetch("INBOX", vec![to_sales]);
    let message = m.message("INBOX", 1);
    let outcome = m
        .run(Command::Reply {
            message,
            body: Some("Sure.".into()),
        })
        .unwrap();
    let id = draft_of(&outcome);
    let draft = &m.engine.state().drafts[&id];
    assert_eq!(draft.from.as_ref().unwrap().email, "sales@example.com");
    assert_eq!(draft.body_text, "Sure.\n\n-- \nSales team\nExample Ltd.");

    // The sent message carries it.
    let sent = m.run(Command::SendDraft { id }).unwrap();
    let Some(Effect::ScheduleTick { at_ms }) = sent
        .effects
        .iter()
        .find(|e| matches!(e, Effect::ScheduleTick { .. }))
        .cloned()
    else {
        panic!("{:?}", sent.effects)
    };
    let fired = m
        .engine
        .ingest(Event::Tick { now_ms: at_ms }, &ctx(80))
        .unwrap();
    let Some(Effect::SmtpSend { message, .. }) = fired
        .effects
        .iter()
        .find(|e| matches!(e, Effect::SmtpSend { .. }))
    else {
        panic!("{:?}", fired.effects)
    };
    assert_eq!(
        message.from,
        Addr {
            name: Some("Sales".into()),
            email: "sales@example.com".into()
        }
    );
    assert!(at_ms > T0);
}

#[test]
fn a_mailbox_default_and_an_explicit_choice() {
    let mut m = Mail::hostinger();
    alias(&mut m);
    let archive = m.mailbox("Archive");
    m.run(Command::SetMailboxIdentity {
        mailbox: archive,
        email: Some("Sales@Example.com".into()),
    })
    .unwrap();
    assert_eq!(
        m.engine.state().mailboxes[&archive].identity.as_deref(),
        Some("sales@example.com")
    );
    m.run(Command::OpenMailbox { id: archive }).unwrap();
    let new = |m: &mut Mail| {
        let outcome = m
            .run(Command::CreateDraft {
                account: m.account,
                to: vec![],
                cc: vec![],
                bcc: vec![],
                subject: String::new(),
                body: String::new(),
            })
            .unwrap();
        draft_of(&outcome)
    };
    let id = new(&mut m);
    assert_eq!(
        m.engine.state().drafts[&id].from.as_ref().unwrap().email,
        "sales@example.com"
    );
    // Back to the account's own address.
    m.run(Command::SetDraftFrom {
        id,
        email: "me@example.com".into(),
    })
    .unwrap();
    assert!(m.engine.state().drafts[&id].from.is_none());
    m.run(Command::Undo {}).unwrap();
    assert!(m.engine.state().drafts[&id].from.is_some());
    assert!(matches!(
        m.run(Command::SetDraftFrom {
            id,
            email: "ceo@other.org".into(),
        }),
        Err(CoreError::InvalidArgument { .. })
    ));
    assert!(matches!(
        m.run(Command::SetMailboxIdentity {
            mailbox: archive,
            email: Some("ceo@other.org".into()),
        }),
        Err(CoreError::InvalidArgument { .. })
    ));
}
