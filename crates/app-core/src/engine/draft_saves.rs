//! Saving drafts to the server (`ARCHITECTURE.md §5.2`): local-first, then `SaveDraft` into the
//! account's Drafts mailbox once a draft has been left alone for [`DRAFT_SAVE_IDLE_MS`] (or at
//! once when the front-end closes). Discarded drafts lose their server copy (`DeleteDraft`).

use crate::ctx::Ctx;
use crate::draft::{DRAFT_SAVE_IDLE_MS, DRAFT_SAVE_RETRY_MS, Draft};
use crate::event::Effect;
use crate::ids::{AccountId, DraftId};
use crate::mailbox::{Mailbox, MailboxRole};
use crate::message::{Addr, BodyState};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::SyncIssue;

use super::Engine;
use super::drafts::draft_json;
use super::sync::CacheChanges;

/// The timer after which a draft changed at `now_ms` is idle.
pub(super) const fn save_timer(now_ms: i64) -> Effect {
    Effect::ScheduleTick {
        at_ms: now_ms.saturating_add(DRAFT_SAVE_IDLE_MS),
    }
}

impl Engine {
    /// `SaveDraft` effects for drafts with unsaved changes: idle ones (or all, if `closing`),
    /// not being saved, with every attachment checked, of accounts with a Drafts mailbox.
    pub(super) fn pump_draft_saves(&mut self, now_ms: i64, closing: bool) -> Vec<Effect> {
        let due: Vec<DraftId> = self
            .state
            .drafts
            .values()
            .filter(|d| d.needs_save() && !d.is_outgoing())
            .filter(|d| !self.state.status.drafts_saving.contains(&d.id))
            .filter(|d| closing || d.updated_ms.saturating_add(DRAFT_SAVE_IDLE_MS) <= now_ms)
            .filter(|d| {
                d.attachments
                    .iter()
                    .all(|a| a.state != crate::draft::AttachmentState::Checking)
            })
            .map(|d| d.id)
            .collect();
        let mut effects = Vec::new();
        for id in due {
            if let Some(effect) = self.save_effect(id, now_ms) {
                self.state.status.drafts_saving.insert(id);
                effects.push(effect);
            }
        }
        effects
    }

    /// The `SaveDraft` effect of draft `id`, if its account has a Drafts mailbox.
    fn save_effect(&self, id: DraftId, now_ms: i64) -> Option<Effect> {
        let draft = self.state.drafts.get(&id)?;
        let account = self.state.accounts.get(&draft.account)?;
        let mailbox = self.drafts_mailbox(draft.account)?;
        Some(Effect::SaveDraft {
            draft: id,
            account: account.id,
            imap: account.imap_login(),
            path: mailbox.path.clone(),
            revision: draft.updated_ms,
            message: Box::new(self.outgoing(draft, now_ms)),
        })
    }

    /// The draft as it is rendered now: from its identity (`set_draft_from`) or the account's
    /// own address (with the name its identity gives it, if any), with the quoted original's
    /// blob if it is cached.
    pub(super) fn outgoing(&self, draft: &Draft, now_ms: i64) -> crate::draft::OutgoingMessage {
        let from = self
            .state
            .accounts
            .get(&draft.account)
            .map(|account| {
                draft.from.clone().unwrap_or_else(|| {
                    let named = account
                        .identity(&account.email)
                        .and_then(|i| i.name)
                        .unwrap_or_else(|| account.display_name.clone());
                    Addr {
                        name: (named != account.email).then_some(named),
                        email: account.email.clone(),
                    }
                })
            })
            .unwrap_or_else(|| Addr {
                name: None,
                email: String::new(),
            });
        let blob = draft
            .quote
            .as_ref()
            .and_then(|quote| self.state.messages.get(&quote.message))
            .and_then(|original| match &original.body {
                BodyState::Fetched { blob } => Some(blob.clone()),
                BodyState::NotFetched | BodyState::Failed { .. } => None,
            });
        draft.outgoing(from, blob, now_ms)
    }

    /// The account's Drafts mailbox (role `drafts`).
    pub(super) fn drafts_mailbox(&self, account: AccountId) -> Option<&Mailbox> {
        self.state
            .mailboxes
            .values()
            .find(|m| m.account == account && m.role == Some(MailboxRole::Drafts))
    }

    /// `DeleteDraft` for a draft that is (or may be) on the server; forgets a save in flight.
    /// Effects run in order, so a running save completes before the delete.
    pub(super) fn delete_server_copy(&mut self, draft: &Draft) -> Option<Effect> {
        let saving = self.state.status.drafts_saving.remove(&draft.id);
        if draft.saved_ms.is_none() && !saving {
            return None;
        }
        let account = self.state.accounts.get(&draft.account)?;
        let mailbox = self.drafts_mailbox(draft.account)?;
        Some(Effect::DeleteDraft {
            draft: draft.id,
            account: account.id,
            imap: account.imap_login(),
            path: mailbox.path.clone(),
            message_id: draft.message_id_hdr.clone(),
        })
    }

    /// A save finished (`Ok(revision)`) or failed. A draft that changed meanwhile is saved
    /// again after the idle delay; a failed save is retried after [`DRAFT_SAVE_RETRY_MS`].
    pub(super) fn ingest_draft_saved(
        &mut self,
        id: DraftId,
        result: Result<i64, String>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.status.drafts_saving.remove(&id);
        let receipt = receipt.affecting(EntityRef::Status);
        let Some(draft) = self.state.drafts.get_mut(&id) else {
            // Discarded (or sent) while saving; its `DeleteDraft` follows.
            return Outcome::new(receipt);
        };
        let succeeded = result.is_ok();
        let retry = match result {
            Ok(revision) => {
                draft.saved_ms = Some(revision);
                draft.save_error = None;
                None
            }
            Err(error) => {
                draft.save_error = Some(error);
                Some(Effect::ScheduleTick {
                    at_ms: ctx.now_ms.saturating_add(DRAFT_SAVE_RETRY_MS),
                })
            }
        };
        // After a failure only the retry timer saves again (no tight loop while offline).
        let changed = succeeded && draft.needs_save();
        let json = draft_json(draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        let mut outcome =
            self.draft_outcome(receipt.affecting(EntityRef::Draft(id)), json, changes);
        outcome.effects.extend(retry);
        if changed {
            outcome
                .effects
                .extend(self.pump_draft_saves(ctx.now_ms, false));
        }
        outcome
    }

    /// The front-end is closing: save every draft with unsaved changes now.
    pub(super) fn ingest_closing(&mut self, ctx: &Ctx, receipt: CommandReceipt) -> Outcome {
        let effects = self.pump_draft_saves(ctx.now_ms, true);
        let mut receipt = receipt;
        if !effects.is_empty() {
            receipt = receipt.affecting(EntityRef::Status);
        }
        Outcome { receipt, effects }
    }

    /// A server copy of a discarded or sent draft could not be deleted: the account's sync
    /// status shows it (the copy stays in Drafts; the user can delete it there).
    pub(super) fn ingest_draft_delete_failed(
        &mut self,
        account: AccountId,
        draft: DraftId,
        error: String,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        if !self.state.accounts.contains_key(&account) {
            return Outcome::new(receipt);
        }
        let mailbox = self.drafts_mailbox(account).map(|m| m.id);
        self.state
            .status
            .sync
            .entry(account)
            .or_default()
            .last_error = Some(SyncIssue {
            error: format!("could not delete the server copy of draft {draft}: {error}"),
            mailbox,
            at_ms: ctx.now_ms,
        });
        Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .affecting(EntityRef::Status),
        )
    }
}
