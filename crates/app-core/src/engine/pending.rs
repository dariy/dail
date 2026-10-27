//! `list_pending_ops`, `retry_pending_ops`, `discard_pending_op`: the outbox as seen by users
//! and agents (`ARCHITECTURE.md §5.2, §10`).

use serde_json::{Value, json};

use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, MailboxId, MessageId, OpId};
use crate::ops::{OpState, PendingOp, RemoteOp};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::sync::{CacheChanges, push_unique};
use super::{Engine, invalid, not_found};

impl Engine {
    pub(super) fn list_pending_ops(
        &self,
        account: Option<AccountId>,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if let Some(id) = account
            && !self.state.accounts.contains_key(&id)
        {
            return Err(not_found("account", id));
        }
        let ops: Vec<Value> = self
            .state
            .outbox
            .values()
            .filter(|op| account.is_none_or(|id| op.account == id))
            .map(op_json)
            .collect();
        Ok(Outcome::new(receipt.with_data(json!({ "ops": ops }))))
    }

    pub(super) fn retry_pending_ops(
        &mut self,
        id: Option<OpId>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if let Some(id) = id
            && !self.state.outbox.contains_key(&id)
        {
            return Err(not_found("op", id));
        }
        let mut changes = CacheChanges::default();
        let mut retried = Vec::new();
        let mut accounts: Vec<AccountId> = Vec::new();
        for op in self.state.outbox.values_mut() {
            let waiting = op.state == OpState::Failed
                || (op.state == OpState::Queued && op.next_attempt_ms.is_some());
            if id.is_some_and(|id| id != op.id) || !waiting {
                continue;
            }
            // A failed op gets one more attempt; its failures so far still count.
            op.state = OpState::Queued;
            op.next_attempt_ms = None;
            retried.push(op.id);
            changes.ops.insert(op.id);
            push_unique(&mut accounts, op.account);
        }
        let mut effects = Vec::new();
        for account in accounts {
            effects.extend(self.pump_ops(account, ctx.now_ms, &mut changes));
        }
        let mut receipt = receipt.with_data(json!({ "retried": retried }));
        for id in &retried {
            receipt = receipt.affecting(EntityRef::Op(*id));
        }
        Ok(self.outcome(receipt, changes, effects))
    }

    pub(super) fn discard_pending_op(
        &mut self,
        id: OpId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let state = self
            .state
            .outbox
            .get(&id)
            .map(|op| op.state)
            .ok_or_else(|| not_found("op", id))?;
        if state == OpState::Running {
            return Err(invalid(
                "id",
                "the op is running on the server; discard it after it failed",
            ));
        }
        let Some(op) = self.state.outbox.shift_remove(&id) else {
            return Err(not_found("op", id));
        };
        let mut changes = CacheChanges::default();
        changes.removed_ops.insert(id);
        let mut reverted: Vec<MessageId> = Vec::new();
        let mut resync: Vec<MailboxId> = Vec::new();
        match &op.op {
            RemoteOp::SetFlag {
                flag,
                value,
                messages,
            } => {
                for message in messages {
                    let later = self.state.outbox.values().any(|other| {
                        matches!(&other.op, RemoteOp::SetFlag { flag: f, messages: m, .. }
                            if f == flag && m.contains(message))
                    });
                    if let Some(envelope) = self.state.messages.get_mut(message) {
                        if !later && flag.get(&envelope.flags) == *value {
                            flag.set(&mut envelope.flags, !*value);
                            reverted.push(*message);
                            changes.messages.insert(*message);
                        }
                        push_unique(&mut resync, envelope.mailbox);
                    }
                }
            }
            RemoteOp::Move { messages, .. } => {
                for message in messages {
                    let pending = self.move_pending(*message);
                    if let Some(envelope) = self.state.messages.get_mut(message) {
                        if !pending && envelope.moved_to.is_some() {
                            envelope.moved_to = None;
                            reverted.push(*message);
                            changes.messages.insert(*message);
                            changes.threads.insert(envelope.thread);
                        }
                        push_unique(&mut resync, envelope.mailbox);
                    }
                }
            }
            RemoteOp::Copy { to, copies } => {
                for copy in copies {
                    if self
                        .state
                        .messages
                        .get(&copy.copy)
                        .is_some_and(|m| m.is_placeholder())
                    {
                        self.remove_message(copy.copy, &mut changes);
                        reverted.push(copy.copy);
                    }
                }
                push_unique(&mut resync, *to);
            }
            // Nothing local to revert: the cached messages had their own `set_flag` op.
            RemoteOp::FlagMailbox { mailbox, .. } => push_unique(&mut resync, *mailbox),
        }
        // The server's state wins: re-read the mailboxes involved.
        let mut effects: Vec<Effect> = resync
            .into_iter()
            .filter_map(|mailbox| self.start_fetch(mailbox))
            .collect();
        effects.extend(self.pump_ops(op.account, ctx.now_ms, &mut changes));
        let receipt = receipt
            .affecting(EntityRef::Op(id))
            .with_data(json!({ "discarded": id, "reverted": reverted, "undoable": false }));
        Ok(self.outcome(receipt, changes, effects))
    }

    /// An outcome that persists `changes` before running `effects`.
    fn outcome(
        &mut self,
        receipt: CommandReceipt,
        changes: CacheChanges,
        effects: Vec<Effect>,
    ) -> Outcome {
        let delta = self.delta(changes);
        let mut outcome = Outcome::new(receipt);
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(effects);
        outcome
    }
}

/// One op for `list_pending_ops`.
fn op_json(op: &PendingOp) -> Value {
    let (action, detail) = match &op.op {
        RemoteOp::SetFlag {
            flag,
            value,
            messages,
        } => (
            "set_flag",
            json!({ "flag": flag, "value": value, "messages": messages }),
        ),
        RemoteOp::Move { to, messages } => ("move", json!({ "to": to, "messages": messages })),
        RemoteOp::Copy { to, copies } => (
            "copy",
            json!({
                "to": to,
                "messages": copies.iter().map(|c| c.source).collect::<Vec<_>>(),
            }),
        ),
        RemoteOp::FlagMailbox {
            mailbox,
            flag,
            value,
        } => (
            "flag_mailbox",
            json!({ "mailbox": mailbox, "flag": flag, "value": value }),
        ),
    };
    json!({
        "id": op.id,
        "account": op.account,
        "action": action,
        "detail": detail,
        "state": op.state,
        "attempts": op.attempts,
        "last_error": op.last_error,
        "next_attempt_ms": op.next_attempt_ms,
        "created_ms": op.created_ms,
    })
}
