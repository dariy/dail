//! `send_draft` / `cancel_send` and the tick-driven dispatch of scheduled sends
//! (`ARCHITECTURE.md §4.4`, `Outbound`).
//!
//! `send_draft` schedules the send at `now + settings.undo_send_ms`; until then `cancel_send`
//! (or undo) stops it. A `Tick` at or after `send_at` hands the message to the runtime
//! (`SmtpSend`): from then on it is irreversible, so every history step about the draft is
//! dropped.

use serde_json::json;

use crate::command::Command;
use crate::ctx::{Ctx, Origin};
use crate::draft::{AttachmentState, Draft};
use crate::error::CoreError;
use crate::event::{Effect, SentCopy};
use crate::ids::DraftId;
use crate::mailbox::MailboxRole;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::SyncIssue;

use super::drafts::draft_json;
use super::sync::CacheChanges;
use super::{Applied, Engine, not_found};

impl Engine {
    pub(super) fn send_draft(
        &mut self,
        id: DraftId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let draft = self
            .state
            .drafts
            .get(&id)
            .ok_or_else(|| not_found("draft", id))?;
        ensure_editable(draft)?;
        sendable(draft)?;
        if ctx.origin == Origin::Mcp {
            // Agents ask; the user decides (`approve_request`).
            return self.request_approval(id, ctx, receipt);
        }
        // Sending it settles any agent's request for it.
        let withdrawn = self.withdraw_approvals(id);
        let window = i64::try_from(self.state.settings.undo_send_ms).unwrap_or(i64::MAX);
        let send_at = ctx.now_ms.saturating_add(window);
        if let Some(draft) = self.state.drafts.get_mut(&id) {
            draft.send_at = Some(send_at);
            draft.send_error = None;
        }
        let mut outcome = self.persist_draft(id, receipt);
        outcome.receipt.data["send_at"] = json!(send_at);
        outcome
            .effects
            .extend(super::approvals::removed_approvals_effect(withdrawn));
        if window == 0 {
            // No undo window: it leaves now and can't be undone.
            outcome.effects.extend(self.pump_sends(ctx.now_ms));
            return Ok(Applied::plain(outcome));
        }
        outcome = outcome.with_effect(Effect::ScheduleTick { at_ms: send_at });
        Ok(Applied {
            outcome,
            undo: Some(("Send".to_owned(), vec![Command::CancelSend { id }])),
        })
    }

    pub(super) fn cancel_send(
        &mut self,
        id: DraftId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let draft = self
            .state
            .drafts
            .get(&id)
            .ok_or_else(|| not_found("draft", id))?;
        if draft.sending {
            return Err(not_sendable(
                id,
                "it is being sent and can't be stopped any more",
            ));
        }
        if draft.send_at.is_none() {
            return Err(not_sendable(id, "no send is scheduled"));
        }
        if let Some(draft) = self.state.drafts.get_mut(&id) {
            draft.send_at = None;
            draft.updated_ms = ctx.now_ms;
        }
        let outcome = self
            .persist_draft(id, receipt)
            .with_effect(super::draft_saves::save_timer(ctx.now_ms));
        Ok(Applied {
            outcome,
            undo: Some(("Cancel send".to_owned(), vec![Command::SendDraft { id }])),
        })
    }

    /// Hands every scheduled send that is due to the runtime. Their history steps are dropped:
    /// a message that left can't be unsent.
    pub(super) fn pump_sends(&mut self, now_ms: i64) -> Vec<Effect> {
        let due: Vec<DraftId> = self
            .state
            .drafts
            .values()
            .filter(|d| !d.sending && d.send_at.is_some_and(|at| at <= now_ms))
            .map(|d| d.id)
            .collect();
        let mut effects = Vec::new();
        for id in due {
            let Some(effect) = self.send_effect(id, now_ms) else {
                continue;
            };
            if let Some(draft) = self.state.drafts.get_mut(&id) {
                draft.sending = true;
            }
            let mut changes = CacheChanges::default();
            changes.drafts.insert(id);
            let delta = self.delta(changes);
            effects.push(Effect::PersistState {
                delta: crate::event::PersistDelta::Mail(delta),
            });
            effects.push(effect);
            self.history.forget(|entry| {
                entry.forward.draft_id() == Some(id)
                    || entry.inverse.iter().any(|cmd| cmd.draft_id() == Some(id))
            });
        }
        effects
    }

    /// The `SmtpSend` effect of draft `id`. The copy goes to the Sent mailbox unless the
    /// provider's SMTP server stores it itself (`PresetProfile::smtp_saves_sent`).
    fn send_effect(&self, id: DraftId, now_ms: i64) -> Option<Effect> {
        let draft = self.state.drafts.get(&id)?;
        let account = self.state.accounts.get(&draft.account)?;
        let sent_copy = if account.preset.profile().smtp_saves_sent {
            None
        } else {
            self.state
                .mailboxes
                .values()
                .find(|m| m.account == account.id && m.role == Some(MailboxRole::Sent))
                .map(|sent| SentCopy {
                    imap: account.imap_login(),
                    path: sent.path.clone(),
                })
        };
        Some(Effect::SmtpSend {
            draft: id,
            account: account.id,
            smtp: account.smtp_login(),
            message: Box::new(self.outgoing(draft, now_ms)),
            sent_copy,
        })
    }

    /// The message left: the draft is gone (its server copy is deleted). A failed Sent copy is
    /// shown in the account's sync status.
    pub(super) fn ingest_message_sent(
        &mut self,
        id: DraftId,
        sent_copy_error: Option<String>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(draft) = self.state.drafts.shift_remove(&id) else {
            return Outcome::new(receipt);
        };
        let mut changes = CacheChanges::default();
        changes.removed_drafts.insert(id);
        let mut receipt = receipt
            .affecting(EntityRef::Draft(id))
            .affecting(EntityRef::Account(draft.account));
        if let Some(error) = &sent_copy_error {
            let mailbox = self
                .state
                .mailboxes
                .values()
                .find(|m| m.account == draft.account && m.role == Some(MailboxRole::Sent))
                .map(|m| m.id);
            self.state
                .status
                .sync
                .entry(draft.account)
                .or_default()
                .last_error = Some(SyncIssue {
                error: format!("sent, but saving the copy in Sent failed: {error}"),
                mailbox,
                at_ms: ctx.now_ms,
            });
            receipt = receipt.affecting(EntityRef::Status);
        }
        let data = json!({
            "sent": id,
            "message_id": draft.message_id_hdr,
            "sent_copy_error": sent_copy_error,
        });
        let mut outcome = self.draft_outcome(receipt, data, changes);
        outcome.effects.extend(self.delete_server_copy(&draft));
        outcome
    }

    /// Nothing was sent: the draft is editable again and shows the error.
    pub(super) fn ingest_send_failed(
        &mut self,
        id: DraftId,
        error: String,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(draft) = self.state.drafts.get_mut(&id) else {
            return Outcome::new(receipt);
        };
        draft.sending = false;
        draft.send_at = None;
        draft.send_error = Some(error);
        self.persist_draft(id, receipt)
    }

    /// Persists draft `id` and returns it as the receipt data.
    fn persist_draft(&mut self, id: DraftId, receipt: CommandReceipt) -> Outcome {
        let data = self
            .state
            .drafts
            .get(&id)
            .map(draft_json)
            .unwrap_or_default();
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        self.draft_outcome(receipt.affecting(EntityRef::Draft(id)), data, changes)
    }
}

/// A draft with a scheduled or running send can't be changed.
pub(super) fn ensure_editable(draft: &Draft) -> Result<(), CoreError> {
    if draft.sending {
        return Err(not_sendable(draft.id, "it is being sent"));
    }
    if draft.send_at.is_some() {
        return Err(not_sendable(
            draft.id,
            "a send is scheduled; run cancel_send first",
        ));
    }
    Ok(())
}

/// Checks that a draft has recipients and only checked attachments.
fn sendable(draft: &Draft) -> Result<(), CoreError> {
    if draft.to.is_empty() && draft.cc.is_empty() && draft.bcc.is_empty() {
        return Err(not_sendable(
            draft.id,
            "it has no recipients; set to, cc or bcc with update_draft",
        ));
    }
    for attachment in &draft.attachments {
        match &attachment.state {
            AttachmentState::Ready { .. } => {}
            AttachmentState::Checking => {
                return Err(not_sendable(
                    draft.id,
                    &format!("{} is still being checked; try again", attachment.name),
                ));
            }
            AttachmentState::Failed { error } => {
                return Err(not_sendable(
                    draft.id,
                    &format!(
                        "attachment {} failed ({error}); remove it with remove_attachment",
                        attachment.name
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn not_sendable(id: DraftId, reason: &str) -> CoreError {
    CoreError::NotSendable {
        draft: id.to_string(),
        reason: reason.to_owned(),
    }
}
