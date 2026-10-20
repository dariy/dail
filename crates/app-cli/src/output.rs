//! JSON output: one line per result on stdout (or pretty-printed with `--pretty`).

use std::io::Write;

use app_core::CoreError;
use app_sync::SubmitResult;
use serde_json::{Value, json};

/// The JSON document for one result. Rejections look like
/// `{"ok":false,"name":"undo","error":{"error":"nothing_to_undo"}}`; batch results add `line`.
pub fn result_json(name: Option<&str>, line: Option<usize>, result: &SubmitResult) -> Value {
    let mut value = match result {
        // reason: a receipt is plain data; serializing it cannot fail.
        Ok(receipt) => serde_json::to_value(receipt).unwrap_or_default(),
        Err(err) => error_json(name, err),
    };
    if let (Some(line), Some(object)) = (line, value.as_object_mut()) {
        object.insert("line".to_owned(), json!(line));
    }
    value
}

/// The JSON of a dry run: the preview (receipt fields, `dry_run`, `would_run`, `changes`) or
/// the rejection.
pub fn preview_json(name: &str, line: Option<usize>, result: &app_sync::PreviewResult) -> Value {
    let mut value = match result {
        // reason: a preview is plain data; serializing it cannot fail.
        Ok(preview) => serde_json::to_value(preview).unwrap_or_default(),
        Err(err) => error_json(Some(name), err),
    };
    if let (Some(line), Some(object)) = (line, value.as_object_mut()) {
        object.insert("line".to_owned(), json!(line));
    }
    value
}

fn error_json(name: Option<&str>, err: &CoreError) -> Value {
    json!({ "ok": false, "name": name, "error": err })
}

/// Writes one JSON document and flushes, so that streaming consumers see it immediately.
pub fn emit(pretty: bool, value: &Value) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    if pretty {
        serde_json::to_writer_pretty(&mut out, value)?;
    } else {
        serde_json::to_writer(&mut out, value)?;
    }
    writeln!(out)?;
    out.flush()?;
    Ok(())
}
