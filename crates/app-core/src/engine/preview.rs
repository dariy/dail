//! Dry runs (`dry_run: true` on CLI, batch and MCP calls): a command is dispatched on a copy of
//! the engine and its outcome reported — the receipt, the effects that would run (by name: their
//! arguments can hold message text) and what changes in each affected entity — while nothing
//! changes, runs or enters the history.

use serde::{Deserialize, Serialize};

use super::Engine;
use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::receipt::{CommandReceipt, EntityRef};
use crate::state::AppState;

/// What a command would do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preview {
    /// The receipt the command would get.
    #[serde(flatten)]
    pub receipt: CommandReceipt,
    /// Always `true`: nothing happened.
    pub dry_run: bool,
    /// The effects that would run, by name (`run_remote_op`, `persist_state`, …).
    pub would_run: Vec<String>,
    /// For each affected entity, the top-level fields that would change.
    pub changes: Vec<Change>,
}

/// The fields of one entity that a command would change. Values can hold email content, so
/// they sit inside `untrusted_content`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    /// The entity.
    pub entity: EntityRef,
    /// `before` and `after`: the differing fields (`null`: the entity is created or removed).
    pub untrusted_content: Diff,
}

/// Before and after of the changed fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    /// The fields as they are (`null` if the entity does not exist yet).
    pub before: serde_json::Value,
    /// The fields as they would be (`null` if the entity would be gone).
    pub after: serde_json::Value,
}

impl Engine {
    /// Dispatches `cmd` on a copy of the engine and reports what it would do; this engine is not
    /// changed. Rejections are the same as [`Engine::dispatch`]'s.
    pub fn preview(&self, cmd: Command, ctx: &Ctx) -> Result<Preview, CoreError> {
        let mut trial = self.clone();
        let outcome = trial.dispatch(cmd, ctx)?;
        let changes = outcome
            .receipt
            .affected
            .iter()
            .filter_map(|entity| change(&self.state, &trial.state, entity))
            .collect();
        Ok(Preview {
            would_run: outcome
                .effects
                .iter()
                .map(|e| e.name().to_owned())
                .collect(),
            receipt: outcome.receipt,
            dry_run: true,
            changes,
        })
    }
}

/// The JSON of `entity` in `state`, if it exists.
fn entity_json(state: &AppState, entity: &EntityRef) -> Option<serde_json::Value> {
    let value = match entity {
        EntityRef::Account(id) => serde_json::to_value(state.accounts.get(id)?),
        EntityRef::Mailbox(id) => serde_json::to_value(state.mailboxes.get(id)?),
        EntityRef::Message(id) => serde_json::to_value(state.messages.get(id)?),
        EntityRef::Thread(id) => serde_json::to_value(state.threads.get(id)?),
        EntityRef::Draft(id) => serde_json::to_value(state.drafts.get(id)?),
        EntityRef::Op(id) => serde_json::to_value(state.outbox.get(id)?),
        EntityRef::Approval(id) => serde_json::to_value(state.approvals.get(id)?),
        EntityRef::Settings => serde_json::to_value(&state.settings),
        EntityRef::View => serde_json::to_value(&state.view),
        EntityRef::Status => serde_json::to_value(&state.status),
        EntityRef::Update => serde_json::to_value(&state.update),
    };
    value.ok()
}

/// The differing top-level fields of `entity` between `before` and `after`; `None` if equal.
fn change(before: &AppState, after: &AppState, entity: &EntityRef) -> Option<Change> {
    let (old, new) = (entity_json(before, entity), entity_json(after, entity));
    let diff = match (old, new) {
        (None, None) => return None,
        (Some(old), Some(new)) if old == new => return None,
        (Some(serde_json::Value::Object(old)), Some(serde_json::Value::Object(new))) => {
            let mut keys: Vec<&String> = old.keys().chain(new.keys()).collect();
            keys.sort();
            keys.dedup();
            let mut was = serde_json::Map::new();
            let mut will = serde_json::Map::new();
            for key in keys {
                let (a, b) = (old.get(key), new.get(key));
                if a != b {
                    was.insert(key.clone(), a.cloned().unwrap_or_default());
                    will.insert(key.clone(), b.cloned().unwrap_or_default());
                }
            }
            Diff {
                before: serde_json::Value::Object(was),
                after: serde_json::Value::Object(will),
            }
        }
        (old, new) => Diff {
            before: old.unwrap_or_default(),
            after: new.unwrap_or_default(),
        },
    };
    Some(Change {
        entity: entity.clone(),
        untrusted_content: diff,
    })
}
