//! Parity: registry ⇔ MCP tools ⇔ JSON Schema, and every tool is callable on the memory backend.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use app_core::registry::schema_command_names;
use app_core::{Command, CommandKind, CommandName};
use app_mcp::{annotations, call, tools};
use app_sync::{Backend, Connection, CounterSeeds, Session, VirtualClock};
use app_testkit::MemoryBackend;
use serde_json::json;

async fn session() -> Session {
    Session::open(
        Arc::new(MemoryBackend::new()) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(0)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap()
}

#[test]
fn every_registry_command_is_an_mcp_tool_with_matching_annotations() {
    let tools = tools();
    let tool_names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
    let spec_names: Vec<String> = Command::specs()
        .iter()
        .map(|spec| spec.name.to_owned())
        .collect();
    assert_eq!(tool_names, spec_names);

    let mut schema_names = schema_command_names();
    let mut sorted_tools = tool_names.clone();
    schema_names.sort();
    sorted_tools.sort();
    assert_eq!(
        schema_names, sorted_tools,
        "every tool has a JSON Schema variant"
    );

    for (tool, spec) in tools.iter().zip(Command::specs()) {
        assert_eq!(
            tool.input_schema.get("type"),
            Some(&json!("object")),
            "{}",
            spec.name
        );
        assert!(tool.description.as_deref().is_some_and(|d| !d.is_empty()));
        let hints = tool.annotations.clone().unwrap();
        let expected = annotations(spec.kind);
        assert_eq!(
            hints.read_only_hint, expected.read_only_hint,
            "{}",
            spec.name
        );
        assert_eq!(
            hints.destructive_hint, expected.destructive_hint,
            "{}",
            spec.name
        );
        assert_eq!(
            hints.read_only_hint,
            Some(spec.kind == CommandKind::Query),
            "only queries are read-only"
        );
        assert_eq!(
            hints.destructive_hint,
            Some(spec.kind == CommandKind::Destructive)
        );
    }
}

#[tokio::test(start_paused = true)]
async fn every_tool_accepts_its_example_arguments() {
    let session = session().await;
    for &name in CommandName::ALL {
        let args = serde_json::to_value(name.example()).unwrap()["args"]
            .as_object()
            .cloned();
        // Must never be a protocol error; `undo`/`redo` may be rejected by the engine.
        let result = call(&Connection::Local(session.client()), name.as_str(), args)
            .await
            .unwrap();
        let content = result.structured_content.unwrap();
        assert_eq!(content["name"], name.as_str());
    }
}

#[tokio::test(start_paused = true)]
async fn rejections_are_structured_tool_errors() {
    let session = session().await;
    let result = call(&Connection::Local(session.client()), "undo", None)
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["error"]["error"],
        "nothing_to_undo"
    );
    let result = call(&Connection::Local(session.client()), "no_such_tool", None)
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["error"]["error"],
        "invalid_command"
    );
}

#[tokio::test(start_paused = true)]
async fn successful_call_changes_session_state() {
    let session = session().await;
    let args = json!({"key": "poll_secs", "value": 600});
    let result = call(
        &Connection::Local(session.client()),
        "set_setting",
        args.as_object().cloned(),
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(session.state().settings.poll_secs, 600);
}
