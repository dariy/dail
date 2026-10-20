//! Message bodies for the reader: which `get_thread {include_body: true}` to ask for, and the
//! bodies taken out of its receipt.
//!
//! Bodies are blobs in the store, which only the session can read. A front-end asks for them
//! whenever the open thread or one of its body states changes, and keeps the answer until then.
//! Everything here is email content: it is only displayed, never interpreted.

use std::collections::HashMap;

use app_core::document::Document;
use app_core::{AppState, BodyState, Command, CommandReceipt, MessageId, ThreadId};
use serde_json::Value;

/// A message body as returned by `get_thread`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    /// The text (HTML already turned into text by the runtime).
    pub text: String,
    /// `plain`, `html` or `none`.
    pub source: String,
    /// An HTML body's blocks, for a front-end that renders them ([`Bodies::rich`]).
    pub document: Option<Document>,
    /// Attachment names and sizes.
    pub attachments: Vec<Attachment>,
}

/// One attachment of a received message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// MIME part number (`save_attachments`).
    pub part: usize,
    /// File name (or a placeholder).
    pub name: String,
    /// MIME type.
    pub content_type: String,
    /// Decoded size in bytes.
    pub size: u64,
}

/// The link targets in a body, in order of appearance, without duplicates: `http(s)://` and
/// `mailto:` (the HTML rendering lists every link's real target as a numbered footnote).
pub fn links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some((start, scheme)) = ["https://", "http://", "mailto:"]
        .iter()
        .filter_map(|scheme| rest.find(scheme).map(|at| (at, scheme.len())))
        .min()
    {
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | ')' | ']' | '`')
            })
            .unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if url.len() > scheme && !out.iter().any(|u| u == url) {
            out.push(url.to_owned());
        }
        rest = &tail[end.max(1)..];
    }
    out
}

/// What the bodies were fetched for: the thread and each message's body state.
type BodyKey = (ThreadId, Vec<(MessageId, u8)>);

/// Front-end cache of the open thread's bodies.
#[derive(Debug, Default)]
pub struct Bodies {
    rich: bool,
    shown: Option<BodyKey>,
    requested: Option<BodyKey>,
    bodies: HashMap<MessageId, Body>,
}

impl Bodies {
    /// A cache that also asks for the [`Document`] of HTML bodies (`rich_body`).
    pub fn rich() -> Self {
        Self {
            rich: true,
            ..Self::default()
        }
    }

    /// The cached body of `message`, if any.
    pub fn get(&self, message: MessageId) -> Option<&Body> {
        self.bodies.get(&message)
    }

    /// The `get_thread` to submit when the open thread or a body state changed since the last
    /// request (submit it in the background and hand its receipt to [`Bodies::accept`]).
    pub fn refresh(&mut self, state: &AppState) -> Option<(ThreadId, Command)> {
        let Some(key) = key(state) else {
            self.shown = None;
            self.requested = None;
            self.bodies.clear();
            return None;
        };
        if self.shown.as_ref() == Some(&key) || self.requested.as_ref() == Some(&key) {
            return None;
        }
        if self.shown.as_ref().is_some_and(|(t, _)| *t != key.0) {
            self.bodies.clear();
        }
        let thread = key.0;
        self.requested = Some(key);
        Some((
            thread,
            Command::GetThread {
                id: thread,
                include_body: true,
                rich_body: self.rich,
            },
        ))
    }

    /// Takes the bodies out of the reply to the request for `thread` (`None` if it was rejected
    /// or the session failed).
    pub fn accept(&mut self, thread: ThreadId, receipt: Option<&CommandReceipt>) {
        let Some(requested) = self.requested.take_if(|(t, _)| *t == thread) else {
            return;
        };
        if let Some(receipt) = receipt {
            for message in receipt.data["messages"].as_array().into_iter().flatten() {
                let id = serde_json::from_value::<MessageId>(message["id"].clone());
                let body = &message["untrusted_content"]["body"];
                if let (Ok(id), Some(text)) = (id, body["text"].as_str()) {
                    self.bodies.insert(id, parse_body(text, body));
                }
            }
        }
        self.shown = Some(requested);
    }
}

fn parse_body(text: &str, body: &Value) -> Body {
    let attachments = body["attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| !a["inline"].as_bool().unwrap_or(false))
        .map(|a| Attachment {
            part: a["part"]
                .as_u64()
                .and_then(|p| usize::try_from(p).ok())
                .unwrap_or(0),
            name: a["filename"]
                .as_str()
                .filter(|n| !n.is_empty())
                .unwrap_or("(unnamed)")
                .to_owned(),
            content_type: a["content_type"].as_str().unwrap_or("").to_owned(),
            size: a["size"].as_u64().unwrap_or(0),
        })
        .collect();
    Body {
        text: text.to_owned(),
        source: body["text_source"].as_str().unwrap_or("none").to_owned(),
        document: body
            .get("document")
            .and_then(|d| serde_json::from_value(d.clone()).ok()),
        attachments,
    }
}

fn key(state: &AppState) -> Option<BodyKey> {
    let thread = state.threads.get(&state.view.thread?)?;
    let states = thread
        .messages
        .iter()
        .filter_map(|id| state.messages.get(id))
        .map(|m| {
            let tag = match m.body {
                BodyState::NotFetched => 0,
                BodyState::Fetched { .. } => 1,
                BodyState::Failed { .. } => 2,
            };
            (m.id, tag)
        })
        .collect();
    Some((thread.id, states))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_targets_are_found_in_text_and_footnotes() {
        let text = "See https://example.org/a, and (https://example.org/b).\n\
                    Mail me: mailto:bob@example.org!\n\
                    [1]: https://track.example/c?x=1&y=2\n\
                    again https://example.org/a\n\
                    not javascript:alert(1) nor ftp://x";
        assert_eq!(
            links(text),
            [
                "https://example.org/a",
                "https://example.org/b",
                "mailto:bob@example.org",
                "https://track.example/c?x=1&y=2",
            ]
        );
        assert!(links("no links here: https:// mailto:").is_empty());
    }
}
