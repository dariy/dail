//! Approval requests: `send_draft` from an agent, `approve_request`, `reject_request` and
//! `list_approvals` (`ARCHITECTURE.md §9`).
//!
//! An agent (`Origin::Mcp`) can prepare a draft but not send it: its `send_draft` records an
//! [`ApprovalRequest`] instead. Only the user (GUI, CLI, batch) decides it: `approve_request` and
//! `reject_request` are user-only in the registry (`AgentAccess::UserOnly`). Approving sends the
//! draft as it is at that moment, exactly like the user's own `send_draft`; the agent changing the
//! draft afterwards withdraws the request, so the user never approves text they haven't seen.

use serde_json::{Value, json};

use crate::approval::ApprovalRequest;
use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, MailDelta, PersistDelta};
use crate::ids::{ApprovalId, DraftId};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::drafts::draft_json;
use super::sync::CacheChanges;
use super::{Applied, Engine, not_found};

/// What a rejected draft's `send_error` says.
pub const REJECTED: &str = "the user declined to send it";

impl Engine {
    /// `send_draft` from an agent: records (or repeats) the draft's approval request. The draft
    /// has been checked to be sendable.
    pub(super) fn request_approval(
        &mut self,
        draft: DraftId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let account = self
            .state
            .drafts
            .get(&draft)
            .ok_or_else(|| not_found("draft", draft))?
            .account;
        let existing = self
            .state
            .approvals
            .values()
            .find(|approval| approval.draft == draft)
            .map(|approval| approval.id);
        let id = match existing {
            Some(id) => id,
            None => {
                let mut ids = ctx.ids();
                let id = loop {
                    let id: ApprovalId = ids.next_id();
                    if !self.state.approvals.contains_key(&id) {
                        break id;
                    }
                };
                self.state.approvals.insert(
                    id,
                    ApprovalRequest {
                        id,
                        draft,
                        account,
                        origin: ctx.origin,
                        requested_ms: ctx.now_ms,
                    },
                );
                id
            }
        };
        let mut changes = CacheChanges::default();
        changes.approvals.insert(id);
        let data = json!({
            "state": "awaiting_approval",
            "approval": self.approval_json(id),
        });
        let receipt = receipt
            .affecting(EntityRef::Approval(id))
            .affecting(EntityRef::Draft(draft));
        Ok(Applied::plain(self.draft_outcome(receipt, data, changes)))
    }

    /// `approve_request`: sends the draft like the user's own `send_draft` (undo cancels the
    /// send) and drops the request.
    pub(super) fn approve_request(
        &mut self,
        id: ApprovalId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let draft = self
            .state
            .approvals
            .get(&id)
            .ok_or_else(|| not_found("approval", id))?
            .draft;
        let mut applied =
            self.send_draft(draft, ctx, receipt.affecting(EntityRef::Approval(id)))?;
        // `send_draft` from the user withdrew the request already; record which one it was.
        if let Some(data) = applied.outcome.receipt.data.as_object_mut() {
            data.insert("approved".to_owned(), json!(id));
        }
        Ok(applied)
    }

    /// `reject_request`: drops the request and tells the agent through the draft's `send_error`.
    pub(super) fn reject_request(
        &mut self,
        id: ApprovalId,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let approval = self
            .state
            .approvals
            .shift_remove(&id)
            .ok_or_else(|| not_found("approval", id))?;
        let mut changes = CacheChanges::default();
        changes.removed_approvals.insert(id);
        if let Some(draft) = self.state.drafts.get_mut(&approval.draft) {
            draft.send_error = Some(REJECTED.to_owned());
            changes.drafts.insert(approval.draft);
        }
        let data = json!({
            "rejected": id,
            "draft": self.state.drafts.get(&approval.draft).map(draft_json),
            "undoable": false,
        });
        let receipt = receipt
            .affecting(EntityRef::Approval(id))
            .affecting(EntityRef::Draft(approval.draft));
        Ok(self.draft_outcome(receipt, data, changes))
    }

    /// `list_approvals`: every request with the draft it would send, oldest first.
    pub(super) fn list_approvals(&self, receipt: CommandReceipt) -> Outcome {
        let approvals: Vec<Value> = self
            .state
            .approvals
            .keys()
            .map(|id| self.approval_json(*id))
            .collect();
        Outcome::new(receipt.with_data(json!({ "approvals": approvals })))
    }

    /// Drops the approval requests of `draft` (state only) and returns their IDs.
    pub(super) fn withdraw_approvals(&mut self, draft: DraftId) -> Vec<ApprovalId> {
        let ids: Vec<ApprovalId> = self
            .state
            .approvals
            .values()
            .filter(|approval| approval.draft == draft)
            .map(|approval| approval.id)
            .collect();
        for id in &ids {
            self.state.approvals.shift_remove(id);
        }
        ids
    }

    /// After a command: an agent that changes a draft withdraws its pending approval request
    /// (the user would otherwise approve text they never saw). The receipt lists them in
    /// `approvals_withdrawn`.
    pub(super) fn withdraw_on_agent_edit(&mut self, draft: DraftId, applied: &mut Applied) {
        let withdrawn = self.withdraw_approvals(draft);
        if withdrawn.is_empty() {
            return;
        }
        if let Some(data) = applied.outcome.receipt.data.as_object_mut() {
            data.insert("approvals_withdrawn".to_owned(), json!(withdrawn));
        }
        applied
            .outcome
            .effects
            .extend(removed_approvals_effect(withdrawn));
    }

    /// The command recorded in history for `cmd`: redoing an approval is the user's own
    /// `send_draft` (the request itself is gone after the first run).
    pub(super) fn recorded_forward(&self, cmd: &Command) -> Command {
        match cmd {
            Command::ApproveRequest { id } => self.state.approvals.get(id).map_or_else(
                || cmd.clone(),
                |approval| Command::SendDraft { id: approval.draft },
            ),
            _ => cmd.clone(),
        }
    }

    /// An approval request in receipts, with the draft it would send (whose content is inside
    /// `untrusted_content`).
    fn approval_json(&self, id: ApprovalId) -> Value {
        let Some(approval) = self.state.approvals.get(&id) else {
            return Value::Null;
        };
        json!({
            "id": approval.id,
            "draft": approval.draft,
            "account": approval.account,
            "origin": approval.origin,
            "requested_ms": approval.requested_ms,
            "message": self.state.drafts.get(&approval.draft).map(draft_json),
        })
    }
}

/// `PersistState` for approval requests removed outside a cache delta.
pub(super) fn removed_approvals_effect(ids: Vec<ApprovalId>) -> Option<Effect> {
    (!ids.is_empty()).then(|| Effect::PersistState {
        delta: PersistDelta::Mail(MailDelta {
            removed_approvals: ids,
            ..MailDelta::default()
        }),
    })
}
