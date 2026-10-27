//! Triage commands: `mark_read` / `mark_unread`, `star` / `unstar`.
//!
//! Each command changes the local cache first and queues one [`RemoteOp`] per account. Its
//! inverse names exactly the messages that changed, so undo restores the previous flags.

use indexmap::IndexMap;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, MailboxId, MessageId, ThreadId};
use crate::ops::{FlagKind, RemoteOp};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::sync::{CacheChanges, push_unique};
use super::{Applied, Engine, invalid, not_found};

/// A flag change requested by `mark_read`, `mark_unread`, `star` or `unstar`.
pub(super) struct FlagChange {
    pub flag: FlagKind,
    pub value: bool,
    pub threads: Vec<ThreadId>,
    pub messages: Vec<MessageId>,
}

impl FlagChange {
    /// The command that applies this change to `messages`.
    fn command(flag: FlagKind, value: bool, messages: Vec<MessageId>) -> Command {
        let ids = Vec::new();
        match (flag, value) {
            (FlagKind::Seen, true) => Command::MarkRead { ids, messages },
            (FlagKind::Seen, false) => Command::MarkUnread { ids, messages },
            (FlagKind::Flagged, true) => Command::Star { ids, messages },
            (FlagKind::Flagged, false) => Command::Unstar { ids, messages },
            (FlagKind::Pinned, true) => Command::PinThreads { ids, messages },
            (FlagKind::Pinned, false) => Command::UnpinThreads { ids, messages },
        }
    }

    fn title(&self) -> &'static str {
        match (self.flag, self.value) {
            (FlagKind::Seen, true) => "Mark read",
            (FlagKind::Seen, false) => "Mark unread",
            (FlagKind::Flagged, true) => "Star",
            (FlagKind::Flagged, false) => "Unstar",
            (FlagKind::Pinned, true) => "Pin",
            (FlagKind::Pinned, false) => "Unpin",
        }
    }
}

impl Engine {
    pub(super) fn set_flag(
        &mut self,
        change: FlagChange,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let targets = self.flag_targets(&change)?;
        let changed: Vec<MessageId> = targets
            .into_iter()
            .filter(|id| {
                self.state
                    .messages
                    .get(id)
                    .is_some_and(|m| change.flag.get(&m.flags) != change.value)
            })
            .collect();

        let mut changes = CacheChanges::default();
        let mut by_account: IndexMap<AccountId, Vec<MessageId>> = IndexMap::new();
        let mut cancelled = Vec::new();
        let mut receipt = receipt;
        for id in &changed {
            // Reverting a change that has not been sent yet just drops it from its op.
            let reverted = self.cancel_flag(*id, change.flag, change.value, &mut changes);
            if let Some(envelope) = self.state.messages.get_mut(id) {
                change.flag.set(&mut envelope.flags, change.value);
                if reverted {
                    cancelled.push(*id);
                } else {
                    by_account.entry(envelope.account).or_default().push(*id);
                }
                changes.messages.insert(*id);
                receipt = receipt.affecting(EntityRef::Message(*id));
                let thread = EntityRef::Thread(envelope.thread);
                if !receipt.affected.contains(&thread) {
                    receipt = receipt.affecting(thread);
                }
            }
        }
        let mut ids = ctx.ids();
        let mut ops = Vec::new();
        let mut effects: Vec<Effect> = Vec::new();
        for (account, messages) in by_account {
            let op = RemoteOp::SetFlag {
                flag: change.flag,
                value: change.value,
                messages,
            };
            let id = self.queue_op(account, op, ctx, &mut ids, &mut changes);
            receipt = receipt.affecting(EntityRef::Op(id));
            ops.push(id);
            effects.extend(self.pump_ops(account, ctx.now_ms, &mut changes));
        }
        let delta = self.delta(changes);
        let mut outcome = Outcome::new(receipt.with_data(serde_json::json!({
            "changed": changed,
            "ops": ops,
            "cancelled": cancelled,
        })));
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(effects);
        if changed.is_empty() {
            return Ok(Applied::plain(outcome));
        }
        let label = format!("{} ({})", change.title(), count(changed.len()));
        let inverse = FlagChange::command(change.flag, !change.value, changed);
        Ok(Applied {
            outcome,
            undo: Some((label, vec![inverse])),
        })
    }

    /// The messages a flag change touches: every message of each thread (only the newest one
    /// when starring a thread) and each given message, each with its copies in other mailboxes.
    fn flag_targets(&self, change: &FlagChange) -> Result<Vec<MessageId>, CoreError> {
        if change.threads.is_empty() && change.messages.is_empty() {
            return Err(invalid("ids", "give `ids` (threads), `messages` or both"));
        }
        let mut picked: Vec<MessageId> = Vec::new();
        for id in &change.threads {
            let thread = self
                .state
                .threads
                .get(id)
                .ok_or_else(|| not_found("thread", id))?;
            // Starring or pinning a thread marks its newest message.
            if matches!(change.flag, FlagKind::Flagged | FlagKind::Pinned) && change.value {
                picked.extend(thread.messages.last());
            } else {
                picked.extend(&thread.messages);
            }
        }
        for id in &change.messages {
            if !self.state.messages.contains_key(id) {
                return Err(not_found("message", id));
            }
            picked.push(*id);
        }
        let mut targets = Vec::new();
        for id in picked {
            for copy in self.with_copies(id) {
                push_unique(&mut targets, copy);
            }
        }
        Ok(targets)
    }

    /// `message` and the cached copies of it in other mailboxes of its account (same
    /// `Message-ID`).
    pub(super) fn with_copies(&self, message: MessageId) -> Vec<MessageId> {
        let Some(envelope) = self.state.messages.get(&message) else {
            return Vec::new();
        };
        let mut all = vec![message];
        if let Some(header) = &envelope.message_id_hdr {
            all.extend(
                self.state
                    .messages
                    .values()
                    .filter(|m| {
                        m.id != message
                            && m.account == envelope.account
                            && m.message_id_hdr.as_ref() == Some(header)
                    })
                    .map(|m| m.id),
            );
        }
        all
    }
}

/// "1 message", "3 messages".
pub(super) fn count(n: usize) -> String {
    if n == 1 {
        "1 message".to_owned()
    } else {
        format!("{n} messages")
    }
}

impl Engine {
    /// `mark_mailbox_read`: `mark_read` on the cached unread messages shown in the mailbox, and a
    /// `flag_mailbox` op when the server counts more unread mail than that.
    pub(super) fn mark_mailbox_read(
        &mut self,
        id: MailboxId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let mailbox = self
            .state
            .mailboxes
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("mailbox", id))?;
        let unread: Vec<MessageId> = self
            .state
            .messages
            .values()
            .filter(|m| m.location() == id && !m.flags.seen && !m.is_placeholder())
            .map(|m| m.id)
            .collect();
        let cached = u32::try_from(unread.len()).unwrap_or(u32::MAX);
        let uncached = mailbox.unread > cached;
        let mut applied = if unread.is_empty() {
            Applied::plain(Outcome::new(receipt.with_data(serde_json::json!({
                "changed": [],
                "ops": [],
                "cancelled": [],
            }))))
        } else {
            self.set_flag(
                FlagChange {
                    flag: FlagKind::Seen,
                    value: true,
                    threads: Vec::new(),
                    messages: unread,
                },
                ctx,
                receipt,
            )?
        };
        let mut changes = CacheChanges::default();
        let mut effects = Vec::new();
        if uncached {
            let mut ids = ctx.ids();
            let op = RemoteOp::FlagMailbox {
                mailbox: id,
                flag: FlagKind::Seen,
                value: true,
            };
            let op = self.queue_op(mailbox.account, op, ctx, &mut ids, &mut changes);
            applied.outcome.receipt = applied.outcome.receipt.clone().affecting(EntityRef::Op(op));
            if let Some(ops) = applied
                .outcome
                .receipt
                .data
                .get_mut("ops")
                .and_then(|o| o.as_array_mut())
            {
                ops.push(serde_json::json!(op));
            }
            effects.extend(self.pump_ops(mailbox.account, ctx.now_ms, &mut changes));
        }
        if let Some(entry) = self.state.mailboxes.get_mut(&id)
            && entry.unread != 0
        {
            // The server's count follows with the next fetch.
            entry.unread = 0;
            changes.mailboxes.insert(id);
        }
        let delta = self.delta(changes);
        if !delta.is_empty() {
            applied.outcome.effects.push(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        applied.outcome.effects.extend(effects);
        let receipt = &mut applied.outcome.receipt;
        *receipt = receipt.clone().affecting(EntityRef::Mailbox(id));
        if let Some(data) = receipt.data.as_object_mut() {
            data.insert("mailbox".to_owned(), serde_json::json!(id));
            data.insert("uncached_affected".to_owned(), serde_json::json!(uncached));
        }
        if let Some((label, _)) = &mut applied.undo {
            *label = format!("Mark all read ({})", count(cached as usize));
        }
        Ok(applied)
    }
}
