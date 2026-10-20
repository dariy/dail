//! JSONL replay logs: one input per line, replayed deterministically through an [`Engine`].
//!
//! Line format (`ARCHITECTURE.md §6`):
//!
//! ```text
//! {"t": 1700000000000, "origin": "cli", "input": {"command": {"cmd": "undo", "args": {}}}}
//! {"t": 1700000000500, "origin": "runtime", "input": {"event": {"event": "persisted", "args": {}}}}
//! ```
//!
//! `t` becomes `Ctx::now_ms`. `seed` (optional) becomes `Ctx::id_seed`; when it is absent the
//! seed is `line << 64`, so hand-written fixtures still get distinct, stable IDs. Blank lines and
//! lines starting with `#` are ignored.
//!
//! The runtime writes every input of a session as such a record (the audit log,
//! [`ReplayRecord::redacted_line`]): secrets and message text are replaced by `<redacted>`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::ctx::{Ctx, Origin};
use crate::engine::Engine;
use crate::error::CoreError;
use crate::event::Event;
use crate::receipt::Outcome;

/// One line of a replay log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReplayRecord {
    /// Time of the input, in Unix milliseconds.
    pub t: i64,
    /// ID seed of the call; defaults to `line << 64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u128>,
    /// Where the input came from.
    pub origin: Origin,
    /// The command or event.
    pub input: ReplayInput,
}

/// The input of a replay record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplayInput {
    /// Dispatched through [`Engine::dispatch`].
    Command(Command),
    /// Ingested through [`Engine::ingest`].
    Event(Event),
}

/// The result of replaying one record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReplayStep {
    /// 1-based line number of the record.
    pub line: usize,
    /// What the engine returned.
    pub result: StepResult,
}

/// Success or failure of one replayed input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepResult {
    /// The engine accepted the input.
    Ok(Outcome),
    /// The engine rejected the input; state is unchanged.
    Err(CoreError),
}

/// Parses a JSONL log into `(line number, record)` pairs.
pub fn parse_log(text: &str) -> Result<Vec<(usize, ReplayRecord)>, CoreError> {
    let mut records = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let record = serde_json::from_str(trimmed).map_err(|err| CoreError::InvalidReplay {
            line,
            reason: err.to_string(),
        })?;
        records.push((line, record));
    }
    Ok(records)
}

/// What [`ReplayRecord::redacted_line`] puts in place of removed text.
pub const REDACTED: &str = "<redacted>";

/// Keys of command arguments whose string values are message text (a draft's `body`, a restored
/// draft's `body_text`) and never go into an audit log.
const TEXT_KEYS: &[&str] = &["body", "body_text"];

impl ReplayRecord {
    /// The record for an input dispatched or ingested with `ctx`.
    pub const fn new(ctx: &Ctx, input: ReplayInput) -> Self {
        Self {
            t: ctx.now_ms,
            seed: Some(ctx.id_seed),
            origin: ctx.origin,
            input,
        }
    }

    /// The record as an audit-log line (JSON, no newline): passwords are already `<redacted>`
    /// (`Secret` never serializes its value), and the message text a command carries (`body`,
    /// `body_text`) is replaced too. Replaying it rebuilds the same state except for that text.
    pub fn redacted_line(&self) -> String {
        // `serde_json::Value` can't hold a `u128` seed: redact the rest, then put it in front.
        let rest = Self {
            seed: None,
            ..self.clone()
        };
        // reason: a record is plain data; serializing it cannot fail.
        let mut value = serde_json::to_value(&rest).unwrap_or_default();
        if let Some(args) = value.pointer_mut("/input/command/args") {
            redact_text(args);
        }
        let body = value.to_string();
        match (self.seed, body.strip_prefix('{')) {
            (Some(seed), Some(fields)) => format!("{{\"seed\":{seed},{fields}"),
            _ => body,
        }
    }

    /// The context this record is replayed with.
    pub fn ctx(&self, line: usize) -> Ctx {
        Ctx {
            now_ms: self.t,
            id_seed: self.seed.unwrap_or_else(|| (line as u128).wrapping_shl(64)),
            origin: self.origin,
        }
    }
}

/// Replays records through `engine`, continuing past rejected inputs.
pub fn run(engine: &mut Engine, records: Vec<(usize, ReplayRecord)>) -> Vec<ReplayStep> {
    records
        .into_iter()
        .map(|(line, record)| {
            let ctx = record.ctx(line);
            let result = match record.input {
                ReplayInput::Command(cmd) => engine.dispatch(cmd, &ctx),
                ReplayInput::Event(event) => engine.ingest(event, &ctx),
            };
            ReplayStep {
                line,
                result: match result {
                    Ok(outcome) => StepResult::Ok(outcome),
                    Err(err) => StepResult::Err(err),
                },
            }
        })
        .collect()
}

fn redact_text(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if TEXT_KEYS.contains(&key.as_str()) && item.is_string() {
                    *item = serde_json::Value::String(REDACTED.to_owned());
                } else {
                    redact_text(item);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(redact_text),
        _ => {}
    }
}
