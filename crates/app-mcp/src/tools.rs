//! Registry → MCP tools, and tool calls → `Session::submit`.

use std::sync::Arc;

use app_core::{Command, CommandKind, CommandName, Origin};
use app_sync::Connection;
use rmcp::ErrorData;
use rmcp::model::{CallToolResult, JsonObject, Tool, ToolAnnotations};
use serde_json::{Value, json};

/// MCP annotations for a command kind (hints for clients).
pub fn annotations(kind: CommandKind) -> ToolAnnotations {
    let (read_only, destructive, idempotent, open_world) = match kind {
        CommandKind::Query => (true, false, true, false),
        CommandKind::View => (false, false, true, false),
        CommandKind::Mutation => (false, false, false, true),
        CommandKind::Outbound => (false, false, false, true),
        CommandKind::Destructive => (false, true, false, true),
    };
    ToolAnnotations::new()
        .read_only(read_only)
        .destructive(destructive)
        .idempotent(idempotent)
        .open_world(open_world)
}

/// One tool per registry entry, in registry order.
pub fn tools() -> Vec<Tool> {
    Command::specs()
        .into_iter()
        .map(|spec| {
            let schema = CommandName::from_name(spec.name)
                .map(CommandName::args_schema)
                .and_then(|schema| schema.as_object().cloned())
                .unwrap_or_else(|| {
                    let mut object = JsonObject::new();
                    object.insert("type".to_owned(), json!("object"));
                    object
                });
            let mut schema = schema;
            if changes_data(spec.kind) {
                add_dry_run(&mut schema);
            }
            let mut hints = annotations(spec.kind);
            hints.title = Some(spec.title.to_owned());
            Tool::new(spec.name, spec.description(), Arc::new(schema))
                .with_title(spec.title)
                .with_annotations(hints)
        })
        .collect()
}

/// Whether tools of `kind` change data (and take `dry_run`).
const fn changes_data(kind: CommandKind) -> bool {
    matches!(
        kind,
        CommandKind::Mutation | CommandKind::Outbound | CommandKind::Destructive
    )
}

/// Adds the optional `dry_run` argument to a tool's input schema.
fn add_dry_run(schema: &mut JsonObject) {
    let properties = schema.entry("properties").or_insert_with(|| json!({}));
    if let Some(properties) = properties.as_object_mut() {
        properties.insert(
            DRY_RUN.to_owned(),
            json!({
                "type": "boolean",
                "description": "Only report what this would do — the receipt, the effects by \
                    name (`would_run`) and the changed fields of each affected entity \
                    (`changes`) — without doing it."
            }),
        );
    }
}

/// The argument that turns a call into a dry run.
pub const DRY_RUN: &str = "dry_run";

/// Runs one tool call as `Origin::Mcp`; see [`call_as`].
pub async fn call(
    session: &Connection,
    name: &str,
    arguments: Option<JsonObject>,
) -> Result<CallToolResult, ErrorData> {
    call_as(session, Origin::Mcp, name, arguments).await
}

/// Runs one tool call with `origin` (`mcp`, or `mcp_allow_send` for `dail mcp --allow-send`).
/// Engine rejections are tool-level errors (`isError: true`) with the structured `CoreError`;
/// only a dead session is a protocol error.
pub async fn call_as(
    session: &Connection,
    origin: Origin,
    name: &str,
    arguments: Option<JsonObject>,
) -> Result<CallToolResult, ErrorData> {
    let mut arguments = arguments.unwrap_or_default();
    let dry_run = arguments.remove(DRY_RUN) == Some(Value::Bool(true));
    let args = Value::Object(arguments);
    let parsed = Command::from_value(json!({ "cmd": name, "args": args }));
    if dry_run {
        let result = match parsed {
            Ok(cmd) => session
                .preview(cmd, origin)
                .await
                .map_err(|err| ErrorData::internal_error(err.to_string(), None))?,
            Err(err) => Err(err),
        };
        return Ok(match result {
            // reason: a preview is plain data; serializing it cannot fail.
            Ok(preview) => {
                CallToolResult::structured(serde_json::to_value(preview).unwrap_or_default())
            }
            Err(err) => CallToolResult::structured_error(json!({
                "ok": false,
                "name": name,
                "error": err,
            })),
        });
    }
    let result = match parsed {
        Ok(cmd) => session
            .submit(cmd, origin)
            .await
            .map_err(|err| ErrorData::internal_error(err.to_string(), None))?,
        Err(err) => Err(err),
    };
    Ok(match result {
        // reason: a receipt is plain data; serializing it cannot fail.
        Ok(receipt) => {
            CallToolResult::structured(serde_json::to_value(receipt).unwrap_or_default())
        }
        Err(err) => CallToolResult::structured_error(json!({
            "ok": false,
            "name": name,
            "error": err,
        })),
    })
}
