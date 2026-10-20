//! MCP resources on the memory backend: listing, templates and reads.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use app_core::{Command, Origin, ProviderPreset, Secret};
use app_mcp::resources::{self, MAILBOXES, VIEW};
use app_sync::{Backend, Connection, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};
use rmcp::model::ResourceContents;
use serde_json::Value;

async fn synced() -> Session {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = Session::open(
        backend as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
    for cmd in [
        Command::AddAccount {
            preset: ProviderPreset::Hostinger,
            email: "alice@example.com".into(),
            display_name: None,
            password: Some(Secret::new("pw")),
            imap: None,
            smtp: None,
        },
        Command::SyncNow { account: None },
    ] {
        session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
        session.flush().await.unwrap();
    }
    session
}

async fn read(conn: &Connection, uri: &str) -> Value {
    let ResourceContents::TextResourceContents {
        uri: read_uri,
        mime_type,
        text,
        ..
    } = resources::read(conn, uri).await.unwrap()
    else {
        unreachable!("resources are text");
    };
    assert_eq!(read_uri, uri);
    assert_eq!(mime_type.as_deref(), Some("application/json"));
    serde_json::from_str(&text).unwrap()
}

#[tokio::test]
async fn resources_list_every_mailbox_and_read_through_queries() {
    let session = synced().await;
    let conn = Connection::Local(session.client());
    let listed = resources::list(&conn).await.unwrap();
    let uris: Vec<&str> = listed.iter().map(|r| r.uri.as_str()).collect();
    assert_eq!(&uris[..2], &[MAILBOXES, VIEW]);
    let state = session.state();
    assert_eq!(listed.len(), 2 + state.mailboxes.len());
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.path == "INBOX")
        .unwrap()
        .id;
    let inbox_uri = resources::mailbox_threads_uri(&inbox.to_string());
    assert!(uris.contains(&inbox_uri.as_str()));

    let mailboxes = read(&conn, MAILBOXES).await;
    assert_eq!(mailboxes["mailboxes"][0]["path"], "INBOX");

    let threads = read(&conn, &inbox_uri).await;
    let first = &threads["threads"][0];
    assert!(first["untrusted_content"]["subject"].is_string());
    let thread_id = first["id"].as_str().unwrap();

    let thread = read(&conn, &resources::thread_uri(thread_id)).await;
    assert_eq!(thread["id"], thread_id);
    session.flush().await.unwrap();
    // Bodies are downloaded on the first read; the second carries their text.
    let thread = read(&conn, &resources::thread_uri(thread_id)).await;
    let message = &thread["messages"][0];
    assert!(
        message["untrusted_content"]["body"]["text"].is_string(),
        "{message}"
    );

    let view = read(&conn, VIEW).await;
    assert_eq!(view["focus"], "list");
}

#[tokio::test]
async fn unknown_or_malformed_uris_are_not_found() {
    let session = synced().await;
    let conn = Connection::Local(session.client());
    for uri in [
        "dail://nope",
        "https://example.com",
        "dail://thread/not-a-uuid",
        "dail://thread/00000000-0000-0000-0000-000000000000",
    ] {
        let err = resources::read(&conn, uri).await.unwrap_err();
        assert_eq!(
            err.code,
            rmcp::model::ErrorCode::RESOURCE_NOT_FOUND,
            "{uri}"
        );
    }
}

#[test]
fn templates_name_the_parameterized_uris() {
    let templates: Vec<String> = resources::templates()
        .into_iter()
        .map(|t| t.uri_template)
        .collect();
    assert_eq!(
        templates,
        ["dail://mailbox/{id}/threads", "dail://thread/{id}"]
    );
}

#[tokio::test]
async fn a_dry_run_reports_and_changes_nothing() {
    let session = synced().await;
    let conn = Connection::Local(session.client());
    let before = session.state();
    let thread = *before.threads.keys().next().unwrap();
    let mut arguments = serde_json::Map::new();
    arguments.insert("ids".into(), serde_json::json!([thread]));
    arguments.insert("dry_run".into(), serde_json::json!(true));
    let result = app_mcp::call(&conn, "archive_threads", Some(arguments))
        .await
        .unwrap();
    let out = result.structured_content.unwrap();
    assert_eq!(out["dry_run"], true, "{out}");
    assert_eq!(out["name"], "archive_threads");
    assert!(
        out["would_run"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e == "run_remote_op")
    );
    session.flush().await.unwrap();
    assert_eq!(session.state(), before, "nothing changed");
}
