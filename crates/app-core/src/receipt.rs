//! `Outcome` and `CommandReceipt`: what `dispatch` / `ingest` return.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::CommandKind;
use crate::event::Effect;
use crate::ids::{AccountId, ApprovalId, DraftId, MailboxId, MessageId, OpId, ThreadId};

/// The result of a successful `dispatch` or `ingest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Outcome {
    /// What happened, for the caller.
    pub receipt: CommandReceipt,
    /// Side effects the runtime must perform.
    pub effects: Vec<Effect>,
}

impl Outcome {
    /// An outcome without effects.
    pub fn new(receipt: CommandReceipt) -> Self {
        Self {
            receipt,
            effects: Vec::new(),
        }
    }

    /// Adds an effect.
    #[must_use]
    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.effects.push(effect);
        self
    }
}

/// The caller-facing result of one input. Query results travel in `data`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandReceipt {
    /// Always `true`; failures are returned as `CoreError` instead.
    pub ok: bool,
    /// Name of the command or event that produced this receipt.
    pub name: String,
    /// Kind of the command; `null` for events.
    pub kind: Option<CommandKind>,
    /// Entities that were read or changed.
    pub affected: Vec<EntityRef>,
    /// Command-specific result data (`null` if none).
    pub data: serde_json::Value,
    /// Label of the history entry that `undo` would revert, if the command was recorded.
    pub undo_label: Option<String>,
}

impl CommandReceipt {
    /// An empty receipt for a command.
    pub fn command(name: &str, kind: CommandKind) -> Self {
        Self {
            ok: true,
            name: name.to_owned(),
            kind: Some(kind),
            affected: Vec::new(),
            data: serde_json::Value::Null,
            undo_label: None,
        }
    }

    /// An empty receipt for an event.
    pub fn event(name: &str) -> Self {
        Self {
            ok: true,
            name: name.to_owned(),
            kind: None,
            affected: Vec::new(),
            data: serde_json::Value::Null,
            undo_label: None,
        }
    }

    /// Records an affected entity.
    #[must_use]
    pub fn affecting(mut self, entity: EntityRef) -> Self {
        self.affected.push(entity);
        self
    }

    /// Sets the result data.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }
}

/// A reference to something a command read or changed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "entity", content = "id", rename_all = "snake_case")]
pub enum EntityRef {
    /// An account.
    Account(AccountId),
    /// A mailbox.
    Mailbox(MailboxId),
    /// A message.
    Message(MessageId),
    /// A thread.
    Thread(ThreadId),
    /// A draft.
    Draft(DraftId),
    /// A pending remote operation.
    Op(OpId),
    /// An approval request.
    Approval(ApprovalId),
    /// The user settings.
    Settings,
    /// The view state.
    View,
    /// The runtime status.
    Status,
}
