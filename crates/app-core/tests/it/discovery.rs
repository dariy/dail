//! `discover_servers` (ADR 0006): the lookup effect, its answer in `status.discovery`, and the
//! addresses it refuses.

use crate::support::ctx;
use app_core::{
    Command, CoreError, DiscoveredServers, Effect, Endpoint, Engine, Event, Origin, Security,
};

fn endpoint(host: &str, port: u16, security: Security) -> Endpoint {
    Endpoint {
        host: host.into(),
        port,
        security,
    }
}

#[test]
fn a_lookup_asks_the_runtime_and_keeps_the_answer() {
    let mut engine = Engine::default();
    let mut c = ctx(1);
    c.origin = Origin::Mcp;
    let outcome = engine
        .dispatch(
            Command::DiscoverServers {
                email: " me@Example.ORG ".into(),
            },
            &c,
        )
        .unwrap();
    assert_eq!(
        outcome.effects,
        [Effect::DiscoverServers {
            email: "me@Example.ORG".into(),
            domain: "example.org".into()
        }],
        "only the domain is looked up; agents may ask (public data)"
    );
    assert!(engine.state().status.discovery.is_none());
    let imap = endpoint("imap.example.org", 993, Security::Tls);
    let smtp = endpoint("smtp.example.org", 587, Security::StartTls);
    engine
        .ingest(
            Event::ServersDiscovered {
                email: "me@Example.ORG".into(),
                imap: imap.clone(),
                smtp: smtp.clone(),
                source: "https://autoconfig.example.org/mail/config-v1.1.xml".into(),
            },
            &ctx(2),
        )
        .unwrap();
    let discovery = engine.state().status.discovery.clone().unwrap();
    assert_eq!(discovery.email, "me@Example.ORG");
    assert_eq!(
        discovery.found,
        Ok(DiscoveredServers {
            imap,
            smtp,
            source: "https://autoconfig.example.org/mail/config-v1.1.xml".into()
        })
    );
    assert!(engine.state().accounts.is_empty(), "nothing is configured");
    // Asking again answers from what was found, without another lookup.
    let again = engine
        .dispatch(
            Command::DiscoverServers {
                email: "me@Example.ORG".into(),
            },
            &ctx(3),
        )
        .unwrap();
    assert!(again.effects.is_empty());
    assert_eq!(again.receipt.data["state"], "found");
    assert_eq!(again.receipt.data["imap"]["host"], "imap.example.org");
    assert_eq!(
        again.receipt.data["source"],
        "https://autoconfig.example.org/mail/config-v1.1.xml"
    );
    // A failed lookup replaces it.
    engine
        .ingest(
            Event::DiscoveryFailed {
                email: "me@nowhere.example".into(),
                error: "no autoconfig document".into(),
            },
            &ctx(3),
        )
        .unwrap();
    let discovery = engine.state().status.discovery.clone().unwrap();
    assert_eq!(discovery.found, Err("no autoconfig document".into()));
    // A failed lookup is tried again.
    let retry = engine
        .dispatch(
            Command::DiscoverServers {
                email: "me@nowhere.example".into(),
            },
            &ctx(4),
        )
        .unwrap();
    assert_eq!(retry.effects.len(), 1);
}

#[test]
fn a_lookup_is_a_query_without_history() {
    let mut engine = Engine::default();
    engine
        .dispatch(
            Command::DiscoverServers {
                email: "me@example.org".into(),
            },
            &ctx(1),
        )
        .unwrap();
    assert_eq!(engine.history().undo_len(), 0);
}

#[test]
fn addresses_without_a_domain_are_refused() {
    let mut engine = Engine::default();
    for email in ["", "me", "me@localhost", "me@exa mple.org", "me@x/../y.org"] {
        let err = engine
            .dispatch(
                Command::DiscoverServers {
                    email: email.into(),
                },
                &ctx(1),
            )
            .unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "email"),
            "{email}: {err:?}"
        );
    }
}

#[test]
fn endpoints_print_in_the_form_they_are_parsed() {
    for endpoint in [
        endpoint("imap.example.org", 993, Security::Tls),
        endpoint("smtp.example.org", 587, Security::StartTls),
        endpoint("mail.example.org", 2525, Security::StartTls),
    ] {
        assert_eq!(Endpoint::parse(&endpoint.to_string()), Ok(endpoint));
    }
}
