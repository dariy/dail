//! `list_contacts`: the people in the cached mail, for addressing.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, mail};
use app_core::{Addr, Command, CoreError, RemoteEnvelope, listing};

fn from(uid: u32, name: &str, email: &str, to: &[&str]) -> RemoteEnvelope {
    RemoteEnvelope {
        from: vec![Addr {
            name: Some(name.into()),
            email: email.into(),
        }],
        to: to
            .iter()
            .map(|e| Addr {
                name: None,
                email: (*e).into(),
            })
            .collect(),
        ..mail(uid, &format!("m{uid}@x"), None, "Hi", true)
    }
}

#[test]
fn people_you_wrote_to_come_first_and_you_are_left_out() {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            from(1, "Carol", "carol@x.org", &["me@example.com"]),
            from(2, "Carol C.", "CAROL@x.org", &["me@example.com"]),
            from(3, "Bob", "bob@x.org", &["me@example.com", "dave@x.org"]),
        ],
    );
    m.fetch(
        "Sent",
        vec![from(4, "Me", "me@example.com", &["dave@x.org"])],
    );
    let all = listing::contacts(m.engine.state(), "", 10);
    let emails: Vec<&str> = all.iter().map(|c| c.email.as_str()).collect();
    assert_eq!(emails, ["dave@x.org", "carol@x.org", "bob@x.org"]);
    assert!(all[0].written_to);
    assert_eq!(all[1].count, 2, "addresses compare without case");
    assert_eq!(all[1].name.as_deref(), Some("Carol C."), "the newest name");

    let found = m
        .run(Command::ListContacts {
            query: "CAR".into(),
            limit: 5,
        })
        .unwrap();
    let contacts = found.receipt.data["contacts"].as_array().unwrap().clone();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0]["untrusted_content"]["email"], "carol@x.org");
    assert_eq!(contacts[0]["count"], 2);
    assert!(matches!(
        m.run(Command::ListContacts {
            query: String::new(),
            limit: 0
        }),
        Err(CoreError::InvalidArgument { .. })
    ));
}
