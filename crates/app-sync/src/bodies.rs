//! Message bodies in query receipts.
//!
//! The core can't read blobs, so `get_thread` / `get_message` with `include_body` return cached
//! bodies as references (`body: {state: "fetched", blob}`). [`attach_bodies`] reads each blob,
//! parses it with `app_io::mime::parse_message` and puts the readable text and the attachment
//! list into the message's `untrusted_content.body`; the blob reference is dropped. With
//! `rich_body`, an HTML body also comes as `document` (`app_core::document`), for display.

use serde_json::{Value, json};

use crate::backend::Backend;

/// Replaces the body references in a `get_thread` / `get_message` receipt's `data` with parsed
/// bodies (with the `document` of HTML bodies if `rich`). A blob that is missing or unreadable
/// yields `untrusted_content.body: null` and an `error` in `body`.
pub async fn attach_bodies(backend: &dyn Backend, data: &mut Value, rich: bool) {
    if let Some(messages) = data.get_mut("messages").and_then(Value::as_array_mut) {
        for message in messages {
            attach(backend, message, rich).await;
        }
    }
    if let Some(message) = data.get_mut("message") {
        attach(backend, message, rich).await;
    }
}

async fn attach(backend: &dyn Backend, message: &mut Value, rich: bool) {
    let Some(blob) = message
        .pointer("/body/blob")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let (parsed, error) = match backend.get_blob(&blob).await {
        Ok(Some(raw)) => {
            let (parsed, document) = if rich {
                app_io::mime::parse_message_with_document(&raw)
            } else {
                (app_io::mime::parse_message(&raw), None)
            };
            let mut body = json!({
                "text": parsed.text,
                "text_source": parsed.text_source,
                "attachments": parsed.attachments,
            });
            if let Some(document) = document {
                body["document"] = json!(document);
            }
            (body, None)
        }
        Ok(None) => (
            Value::Null,
            Some("body is not in the local store".to_owned()),
        ),
        Err(err) => (Value::Null, Some(err.to_string())),
    };
    message["body"] = match error {
        Some(error) => json!({ "state": "fetched", "error": error }),
        None => json!({ "state": "fetched" }),
    };
    if let Some(untrusted) = message.get_mut("untrusted_content")
        && let Some(object) = untrusted.as_object_mut()
    {
        object.insert("body".to_owned(), parsed);
    }
}
