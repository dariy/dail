//! Sorting rules over MCP (SET-6): agents list, save and apply rules; pushing the settings and
//! choosing the settings account stay with the user.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use app_mcp::{INSTRUCTIONS, call, tools};
use app_sync::{Backend, Connection, CounterSeeds, Session, VirtualClock};
use app_testkit::MemoryBackend;
use serde_json::{Value, json};

async fn session() -> Session {
    Session::open(
        Arc::new(MemoryBackend::new()) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(0)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

async fn tool(conn: &Connection, name: &str, args: Value) -> (bool, Value) {
    let result = call(conn, name, args.as_object().cloned()).await.unwrap();
    (
        result.is_error == Some(true),
        result.structured_content.unwrap(),
    )
}

#[test]
fn rule_tools_are_described_for_agents() {
    let tools = tools();
    for name in [
        "list_rules",
        "save_rule",
        "delete_rule",
        "enable_rule",
        "apply_rules",
    ] {
        let tool = tools.iter().find(|t| t.name == name).unwrap();
        assert!(tool.description.as_deref().is_some_and(|d| !d.is_empty()));
        let hints = tool.annotations.clone().unwrap();
        assert_eq!(hints.read_only_hint, Some(name == "list_rules"), "{name}");
        assert_eq!(hints.destructive_hint, Some(false), "{name}");
    }
    let save = tools.iter().find(|t| t.name == "save_rule").unwrap();
    let text = save.description.as_deref().unwrap();
    assert!(text.contains("never forward, reply or send"));
    assert!(text.contains("push_settings"));
    let list = tools.iter().find(|t| t.name == "list_rules").unwrap();
    assert!(
        list.description
            .as_deref()
            .unwrap()
            .contains("untrusted_content")
    );
    assert!(INSTRUCTIONS.contains("Rules:"));
}

#[tokio::test(start_paused = true)]
async fn agents_save_and_list_rules_but_cannot_push_them() {
    let session = session().await;
    let conn = Connection::Local(session.client());
    let rule = json!({
        "name": "News",
        "when": { "test": "from", "contains": "news@" },
        "then": [{ "action": "mark_read" }],
    });
    let (failed, saved) = tool(&conn, "save_rule", json!({ "rule": rule })).await;
    assert!(!failed, "{saved}");
    let (failed, listed) = tool(&conn, "list_rules", json!({})).await;
    assert!(!failed, "{listed}");
    let text = listed.to_string();
    assert!(text.contains("untrusted_content") && text.contains("news@"));

    let forward = json!({
        "name": "Leak",
        "when": { "test": "all", "of": [] },
        "then": [{ "action": "forward", "to": "x@example.org" }],
    });
    let (failed, _) = tool(&conn, "save_rule", json!({ "rule": forward })).await;
    assert!(failed, "a forward action does not parse");

    for (name, args) in [
        ("push_settings", json!({})),
        (
            "set_settings_account",
            json!({ "account": "00000000-0000-0000-0000-000000000001" }),
        ),
    ] {
        let (failed, content) = tool(&conn, name, args).await;
        assert!(failed, "{name}");
        assert_eq!(
            content["error"]["error"], "error_008_origin_not_allowed",
            "{name}"
        );
    }
}
