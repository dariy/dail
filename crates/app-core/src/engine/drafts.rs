//! Draft commands: `create_draft`, `reply`, `reply_all`, `forward`, `update_draft`,
//! `discard_draft`, `restore_draft` (`ARCHITECTURE.md §4.2, §8`).
//!
//! Drafts are local-first: every change is applied to `AppState::drafts` and persisted with the
//! mail cache at once.

use serde_json::{Value, json};

use crate::command::Command;
use crate::ctx::Ctx;
use crate::draft::{
    AttachmentRef, AttachmentState, Draft, MAX_ATTACHMENT_BYTES, Quote, QuoteStyle, ReplyTo,
    file_name, forward_header, is_absolute_path, is_message_id, prefixed_subject, reply_header,
    reply_recipients, reply_references,
};
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, DraftId, IdGen, MessageId};
use crate::message::{Addr, BodyState};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::accounts::validate_email;
use super::draft_saves::save_timer;
use super::sends::{ensure_editable, ensure_not_sending};
use super::sync::CacheChanges;
use super::{Applied, Engine, invalid, not_found};

/// The editable fields of a draft, as given to `create_draft` / `update_draft`.
#[derive(Debug, Default)]
pub(super) struct DraftFields {
    pub to: Option<Vec<Addr>>,
    pub cc: Option<Vec<Addr>>,
    pub bcc: Option<Vec<Addr>>,
    pub subject: Option<String>,
    pub body: Option<String>,
}

impl DraftFields {
    fn is_empty(&self) -> bool {
        self.to.is_none()
            && self.cc.is_none()
            && self.bcc.is_none()
            && self.subject.is_none()
            && self.body.is_none()
    }
}

/// Which kind of answer `reply`, `reply_all` and `forward` start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    Reply,
    ReplyAll,
    Forward,
}

impl Answer {
    const fn title(self) -> &'static str {
        match self {
            Self::Reply => "Reply",
            Self::ReplyAll => "Reply all",
            Self::Forward => "Forward",
        }
    }
}

impl Engine {
    pub(super) fn create_draft(
        &mut self,
        account: AccountId,
        fields: DraftFields,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let email = self
            .state
            .accounts
            .get(&account)
            .map(|a| a.email.clone())
            .ok_or_else(|| not_found("account", account))?;
        let mut ids = ctx.ids();
        let id = self.fresh_draft_id(&mut ids);
        let mut draft = Draft {
            id,
            account,
            reply_to: None,
            message_id_hdr: message_id(id, &email),
            in_reply_to: None,
            references: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body_text: String::new(),
            quote: None,
            attachments: Vec::new(),
            created_ms: ctx.now_ms,
            updated_ms: ctx.now_ms,
            saved_ms: None,
            save_error: None,
            send_at: None,
            sending: false,
            send_error: None,
            loading_from: None,
            from: None,
        };
        apply_fields(&mut draft, fields)?;
        let mailbox = self.state.view.mailbox;
        self.sign(&mut draft, mailbox, None);
        Ok(self.insert_draft(draft, "Create draft", ctx.now_ms, Vec::new(), receipt))
    }

    pub(super) fn answer(
        &mut self,
        message: MessageId,
        body: Option<String>,
        answer: Answer,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let original = self
            .state
            .messages
            .get(&message)
            .cloned()
            .ok_or_else(|| not_found("message", message))?;
        let email = self
            .state
            .accounts
            .get(&original.account)
            .map(|a| a.email.clone())
            .ok_or_else(|| not_found("account", original.account))?;
        let mut ids = ctx.ids();
        let id = self.fresh_draft_id(&mut ids);
        let (to, cc, subject, quote) = match answer {
            Answer::Reply | Answer::ReplyAll => {
                let mode = if answer == Answer::Reply {
                    ReplyTo::Sender
                } else {
                    ReplyTo::All
                };
                let (to, cc) = reply_recipients(&original, &email, mode);
                let quote = Quote {
                    message,
                    style: QuoteStyle::Reply,
                    header: reply_header(&original),
                };
                (to, cc, prefixed_subject("Re:", &original.subject), quote)
            }
            Answer::Forward => {
                let quote = Quote {
                    message,
                    style: QuoteStyle::Forward,
                    header: forward_header(&original),
                };
                let subject = prefixed_subject("Fwd:", &original.subject);
                (Vec::new(), Vec::new(), subject, quote)
            }
        };
        let in_reply_to = match answer {
            Answer::Reply | Answer::ReplyAll => original
                .message_id_hdr
                .clone()
                .filter(|id| is_message_id(id)),
            Answer::Forward => None,
        };
        let mut draft = Draft {
            id,
            account: original.account,
            reply_to: Some(message),
            message_id_hdr: message_id(id, &email),
            in_reply_to,
            references: reply_references(&original),
            to,
            cc,
            bcc: Vec::new(),
            subject: strip_controls(&subject),
            body_text: String::new(),
            quote: Some(quote),
            attachments: Vec::new(),
            created_ms: ctx.now_ms,
            updated_ms: ctx.now_ms,
            saved_ms: None,
            save_error: None,
            send_at: None,
            sending: false,
            send_error: None,
            loading_from: None,
            from: None,
        };
        apply_fields(
            &mut draft,
            DraftFields {
                body,
                ..DraftFields::default()
            },
        )?;
        self.sign(&mut draft, Some(original.location()), Some(&original));
        // The quote needs the original's text when the draft is rendered.
        let mut effects = Vec::new();
        if !matches!(original.body, BodyState::Fetched { .. })
            && let Some(fetch) = self.start_body_fetch(message)
        {
            effects.push(fetch);
        }
        let mut applied = self.insert_draft(draft, answer.title(), ctx.now_ms, effects, receipt);
        applied.outcome.receipt = applied
            .outcome
            .receipt
            .clone()
            .affecting(EntityRef::Message(message));
        Ok(applied)
    }

    pub(super) fn update_draft(
        &mut self,
        id: DraftId,
        fields: DraftFields,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        if fields.is_empty() {
            return Err(invalid(
                "id",
                "nothing to update: give to, cc, bcc, subject or body",
            ));
        }
        let previous = self
            .state
            .drafts
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("draft", id))?;
        ensure_editable(&previous)?;
        let inverse = Command::UpdateDraft {
            id,
            to: fields.to.as_ref().map(|_| previous.to.clone()),
            cc: fields.cc.as_ref().map(|_| previous.cc.clone()),
            bcc: fields.bcc.as_ref().map(|_| previous.bcc.clone()),
            subject: fields.subject.as_ref().map(|_| previous.subject.clone()),
            body: fields.body.as_ref().map(|_| previous.body_text.clone()),
        };
        let mut draft = previous;
        apply_fields(&mut draft, fields)?;
        draft.updated_ms = ctx.now_ms;
        let json = draft_json(&draft);
        self.state.drafts.insert(id, draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        let outcome = self
            .draft_outcome(receipt.affecting(EntityRef::Draft(id)), json, changes)
            .with_effect(save_timer(ctx.now_ms));
        Ok(Applied {
            outcome,
            undo: Some(("Edit draft".to_owned(), vec![inverse])),
        })
    }

    pub(super) fn discard_draft(
        &mut self,
        id: DraftId,
        keep_server_copy: bool,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        ensure_not_sending(
            self.state
                .drafts
                .get(&id)
                .ok_or_else(|| not_found("draft", id))?,
        )?;
        let draft = self
            .state
            .drafts
            .shift_remove(&id)
            .ok_or_else(|| not_found("draft", id))?;
        let mut changes = CacheChanges::default();
        changes.removed_drafts.insert(id);
        changes.removed_approvals = self.withdraw_approvals(id).into_iter().collect();
        // A draft whose text is still loading has no changes of its own: its server copy stays,
        // so an undo can load it again.
        let keep_server_copy = keep_server_copy || draft.loading_from.is_some();
        let mut dropped = Vec::new();
        if !keep_server_copy {
            // The cached server copy goes at once, not at the next sync (MISC-44).
            dropped = self.server_copies(&draft);
            for message in &dropped {
                self.remove_message(*message, &mut changes);
            }
        }
        let data = json!({ "discarded": id, "draft": draft_json(&draft) });
        let mut receipt = receipt.affecting(EntityRef::Draft(id));
        for message in dropped {
            receipt = receipt.affecting(EntityRef::Message(message));
        }
        let mut outcome = self.draft_outcome(receipt, data, changes);
        if !keep_server_copy {
            outcome.effects.extend(self.delete_server_copy(&draft));
        }
        Ok(Applied {
            outcome,
            undo: Some((
                "Discard draft".to_owned(),
                vec![Command::RestoreDraft {
                    draft: Box::new(draft),
                }],
            )),
        })
    }

    pub(super) fn restore_draft(
        &mut self,
        draft: Draft,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        if self.state.drafts.contains_key(&draft.id) {
            return Err(CoreError::AlreadyExists {
                entity: "draft".to_owned(),
                key: draft.id.to_string(),
            });
        }
        if !self.state.accounts.contains_key(&draft.account) {
            return Err(not_found("account", draft.account));
        }
        validate_message_ids(&draft)?;
        // Checks come only from the runtime, and the files may have changed since the discard:
        // every attachment is checked again.
        let mut attachments: Vec<AttachmentRef> = Vec::with_capacity(draft.attachments.len());
        for attachment in &draft.attachments {
            validate_path(&attachment.path)?;
            if attachments.iter().any(|a| a.path == attachment.path) {
                return Err(CoreError::AlreadyExists {
                    entity: "attachment".to_owned(),
                    key: attachment.path.clone(),
                });
            }
            attachments.push(AttachmentRef {
                name: file_name(&attachment.path).to_owned(),
                path: attachment.path.clone(),
                state: AttachmentState::Checking,
            });
        }
        let inspect: Vec<Effect> = attachments
            .iter()
            .map(|a| Effect::InspectAttachment {
                draft: draft.id,
                path: a.path.clone(),
            })
            .collect();
        // Its server copy was deleted with the discard: save it again.
        let mut restored = Draft {
            saved_ms: None,
            save_error: None,
            send_at: None,
            sending: false,
            attachments,
            ..draft.clone()
        };
        apply_fields(
            &mut restored,
            DraftFields {
                to: Some(draft.to),
                cc: Some(draft.cc),
                bcc: Some(draft.bcc),
                subject: Some(draft.subject),
                body: Some(draft.body_text),
            },
        )?;
        let mut effects = inspect;
        if let Some(message) = restored.loading_from {
            effects.extend(self.draft_load_effect(restored.id, message));
        }
        Ok(self.insert_draft(restored, "Restore draft", ctx.now_ms, effects, receipt))
    }

    /// `edit_draft`: the local draft saved as `message`, or a new local draft made from a draft
    /// that is only on the server, whose text is read in the background (MISC-44).
    pub(super) fn edit_draft(
        &mut self,
        message: MessageId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let envelope = self
            .state
            .messages
            .get(&message)
            .cloned()
            .ok_or_else(|| not_found("message", message))?;
        if !self.state.is_draft(&envelope) {
            return Err(invalid(
                "message",
                "is not a draft; give a message with `is_draft` from get_thread",
            ));
        }
        let receipt = receipt.affecting(EntityRef::Message(message));
        if let Some(id) = self.state.local_draft_of(&envelope) {
            let data = self
                .state
                .drafts
                .get(&id)
                .map(draft_json)
                .unwrap_or_default();
            let outcome = Outcome::new(
                receipt
                    .affecting(EntityRef::Draft(id))
                    .with_data(json!({ "draft": data, "created": false })),
            );
            return Ok(Applied::plain(outcome));
        }
        if envelope.is_placeholder() || envelope.moved_to.is_some() {
            return Err(invalid(
                "message",
                "is being moved; try again when the move has finished",
            ));
        }
        let Some(header) = envelope
            .message_id_hdr
            .clone()
            .filter(|id| is_message_id(id))
        else {
            return Err(invalid(
                "message",
                "has no usable Message-ID, so its server copy could not be replaced",
            ));
        };
        let email = self
            .state
            .accounts
            .get(&envelope.account)
            .map(|a| a.email.clone())
            .ok_or_else(|| not_found("account", envelope.account))?;
        let mut ids = ctx.ids();
        let id = self.fresh_draft_id(&mut ids);
        let from = envelope
            .from
            .first()
            .filter(|a| !a.email.eq_ignore_ascii_case(&email))
            .and_then(|a| valid_addrs(std::slice::from_ref(a)).into_iter().next());
        let draft = Draft {
            id,
            account: envelope.account,
            from,
            reply_to: None,
            message_id_hdr: header,
            in_reply_to: envelope.in_reply_to.clone().filter(|id| is_message_id(id)),
            references: envelope
                .references
                .iter()
                .filter(|id| is_message_id(id))
                .cloned()
                .collect(),
            to: valid_addrs(&envelope.to),
            cc: valid_addrs(&envelope.cc),
            bcc: Vec::new(),
            subject: strip_controls(&envelope.subject).trim().to_owned(),
            body_text: String::new(),
            quote: None,
            attachments: Vec::new(),
            created_ms: ctx.now_ms,
            updated_ms: ctx.now_ms,
            // The server copy is this version: nothing to save until the draft changes.
            saved_ms: Some(ctx.now_ms),
            save_error: None,
            send_at: None,
            sending: false,
            send_error: None,
            loading_from: Some(message),
        };
        let load = self.draft_load_effect(id, message);
        if load.is_none() && !self.state.status.bodies_in_flight.contains(&message) {
            return Err(invalid(
                "message",
                "its text can't be downloaded (the mailbox is not synced yet); try again later",
            ));
        }
        let json = draft_json(&draft);
        self.state.drafts.insert(id, draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        let mut outcome = self.draft_outcome(
            receipt.affecting(EntityRef::Draft(id)),
            json!({ "draft": json, "created": true }),
            changes,
        );
        outcome.effects.extend(load);
        Ok(Applied {
            outcome,
            undo: Some((
                "Edit draft".to_owned(),
                vec![Command::DiscardDraft {
                    id,
                    keep_server_copy: true,
                }],
            )),
        })
    }

    /// The effect that brings server draft `message`'s text into draft `id`: `ReadDraft` if
    /// its body is cached, else its download (`None` if it is downloading or can't be).
    fn draft_load_effect(&mut self, id: DraftId, message: MessageId) -> Option<Effect> {
        match &self.state.messages.get(&message)?.body {
            BodyState::Fetched { blob } => Some(Effect::ReadDraft {
                draft: id,
                message,
                blob: blob.clone(),
            }),
            _ => self.start_body_fetch(message),
        }
    }

    /// After a body download: `ReadDraft` for each draft waiting for that message's text, or,
    /// if the download failed, those drafts dropped (the server copy stays, and the message
    /// shows why its body failed).
    pub(super) fn drafts_after_body(&mut self, message: MessageId, outcome: &mut Outcome) {
        let waiting: Vec<DraftId> = self
            .state
            .drafts
            .values()
            .filter(|d| d.loading_from == Some(message))
            .map(|d| d.id)
            .collect();
        for id in waiting {
            match self.state.messages.get(&message).map(|m| m.body.clone()) {
                Some(BodyState::Fetched { blob }) => outcome.effects.push(Effect::ReadDraft {
                    draft: id,
                    message,
                    blob,
                }),
                _ => {
                    let dropped = self.abandon_draft_load(id, outcome.receipt.clone());
                    outcome.effects.extend(dropped.effects);
                }
            }
        }
    }

    /// `DraftRead` / `DraftReadFailed`: the text of a server draft arrived in draft `id` (or
    /// could not be read: the draft is dropped, the server copy stays).
    pub(super) fn ingest_draft_read(
        &mut self,
        id: DraftId,
        message: MessageId,
        read: Result<(String, Vec<Addr>), String>,
        receipt: CommandReceipt,
    ) -> Outcome {
        let receipt = receipt.affecting(EntityRef::Draft(id));
        let waiting = self
            .state
            .drafts
            .get(&id)
            .is_some_and(|d| d.loading_from == Some(message));
        if !waiting {
            return Outcome::new(receipt);
        }
        let (body, bcc) = match read {
            Ok(read) => read,
            Err(error) => {
                return self.abandon_draft_load(
                    id,
                    receipt.with_data(json!({ "dropped": id, "error": error })),
                );
            }
        };
        let Some(draft) = self.state.drafts.get_mut(&id) else {
            return Outcome::new(receipt);
        };
        draft.body_text = body.replace('\0', "");
        draft.bcc = valid_addrs(&bcc);
        draft.loading_from = None;
        let json = draft_json(draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        self.draft_outcome(receipt, json, changes)
    }

    /// Drops a draft whose text could not be loaded, with the history steps about it. It had no
    /// changes of its own and its server copy stays.
    fn abandon_draft_load(&mut self, id: DraftId, receipt: CommandReceipt) -> Outcome {
        self.state.drafts.shift_remove(&id);
        self.history.forget(|entry| {
            entry.forward.draft_id() == Some(id)
                || entry.inverse.iter().any(|cmd| cmd.draft_id() == Some(id))
        });
        let mut changes = CacheChanges::default();
        changes.removed_drafts.insert(id);
        let mut outcome = Outcome::new(receipt.affecting(EntityRef::Draft(id)));
        let delta = self.delta(changes);
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome
    }

    /// The cached copies of a draft in its account's Drafts mailbox (same `Message-ID`).
    fn server_copies(&self, draft: &Draft) -> Vec<MessageId> {
        self.state
            .messages
            .values()
            .filter(|m| {
                m.account == draft.account
                    && m.message_id_hdr.as_deref() == Some(draft.message_id_hdr.as_str())
                    && self.state.is_draft(m)
            })
            .map(|m| m.id)
            .collect()
    }

    pub(super) fn attach_file(
        &mut self,
        id: DraftId,
        path: String,
        index: Option<u32>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let draft = self
            .state
            .drafts
            .get(&id)
            .ok_or_else(|| not_found("draft", id))?;
        ensure_editable(draft)?;
        validate_path(&path)?;
        if draft.attachments.iter().any(|a| a.path == path) {
            return Err(CoreError::AlreadyExists {
                entity: "attachment".to_owned(),
                key: path,
            });
        }
        let len = draft.attachments.len();
        let at = match index.map(usize::try_from) {
            None => len,
            Some(Ok(at)) if at <= len => at,
            Some(_) => {
                return Err(invalid(
                    "index",
                    format!("must be between 0 and {len} (the number of attachments)"),
                ));
            }
        };
        let attachment = AttachmentRef {
            name: file_name(&path).to_owned(),
            path: path.clone(),
            state: AttachmentState::Checking,
        };
        let inverse = Command::RemoveAttachment {
            draft: id,
            index: u32::try_from(at).unwrap_or(u32::MAX),
        };
        let mut outcome = self.change_draft(id, ctx.now_ms, receipt, |draft| {
            draft.attachments.insert(at, attachment);
        });
        outcome = outcome.with_effect(Effect::InspectAttachment { draft: id, path });
        Ok(Applied {
            outcome,
            undo: Some(("Attach file".to_owned(), vec![inverse])),
        })
    }

    pub(super) fn remove_attachment(
        &mut self,
        id: DraftId,
        index: u32,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let draft = self
            .state
            .drafts
            .get(&id)
            .ok_or_else(|| not_found("draft", id))?;
        ensure_editable(draft)?;
        let at = usize::try_from(index).unwrap_or(usize::MAX);
        let Some(removed) = draft.attachments.get(at).cloned() else {
            return Err(invalid(
                "index",
                format!("the draft has {} attachment(s)", draft.attachments.len()),
            ));
        };
        let outcome = self.change_draft(id, ctx.now_ms, receipt, |draft| {
            draft.attachments.remove(at);
        });
        let inverse = Command::AttachFile {
            draft: id,
            path: removed.path,
            index: Some(index),
        };
        Ok(Applied {
            outcome,
            undo: Some(("Remove attachment".to_owned(), vec![inverse])),
        })
    }

    /// Records the check of an attached file. A file that does not fit the size limit (together
    /// with the draft's other attachments) is marked failed. Results for attachments that were
    /// removed (or are no longer being checked) are ignored.
    pub(super) fn ingest_attachment(
        &mut self,
        id: DraftId,
        path: &str,
        state: AttachmentState,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(draft) = self.state.drafts.get_mut(&id) else {
            return Outcome::new(receipt);
        };
        let others: u64 = draft
            .attachments
            .iter()
            .filter(|a| a.path != path)
            .filter_map(AttachmentRef::size)
            .sum();
        let Some(attachment) = draft
            .attachments
            .iter_mut()
            .find(|a| a.path == path && a.state == AttachmentState::Checking)
        else {
            return Outcome::new(receipt);
        };
        attachment.state = match state {
            AttachmentState::Ready { size, .. }
                if others.saturating_add(size) > MAX_ATTACHMENT_BYTES =>
            {
                AttachmentState::Failed {
                    error: format!(
                        "{} is {}; all attachments together may have at most {}",
                        attachment.name,
                        megabytes(size),
                        megabytes(MAX_ATTACHMENT_BYTES)
                    ),
                }
            }
            state => state,
        };
        let json = draft_json(draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        self.draft_outcome(receipt.affecting(EntityRef::Draft(id)), json, changes)
    }

    /// Applies `change` to draft `id` (which must exist), bumps `updated_ms`, persists it and
    /// schedules its save to the server.
    fn change_draft(
        &mut self,
        id: DraftId,
        now_ms: i64,
        receipt: CommandReceipt,
        change: impl FnOnce(&mut Draft),
    ) -> Outcome {
        let json = match self.state.drafts.get_mut(&id) {
            Some(draft) => {
                change(draft);
                draft.updated_ms = now_ms;
                draft_json(draft)
            }
            None => Value::Null,
        };
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        self.draft_outcome(receipt.affecting(EntityRef::Draft(id)), json, changes)
            .with_effect(save_timer(now_ms))
    }

    /// Adds a new draft; undo discards it.
    fn insert_draft(
        &mut self,
        draft: Draft,
        label: &str,
        now_ms: i64,
        effects: Vec<Effect>,
        receipt: CommandReceipt,
    ) -> Applied {
        let id = draft.id;
        let json = draft_json(&draft);
        self.state.drafts.insert(id, draft);
        let mut changes = CacheChanges::default();
        changes.drafts.insert(id);
        let mut outcome = self
            .draft_outcome(receipt.affecting(EntityRef::Draft(id)), json, changes)
            .with_effect(save_timer(now_ms));
        outcome.effects.extend(effects);
        Applied {
            outcome,
            undo: Some((
                label.to_owned(),
                vec![Command::DiscardDraft {
                    id,
                    keep_server_copy: false,
                }],
            )),
        }
    }

    /// Sends a new draft from the identity it belongs to (`identity_for`) and adds that
    /// identity's signature below its text.
    fn sign(
        &self,
        draft: &mut Draft,
        mailbox: Option<crate::ids::MailboxId>,
        original: Option<&crate::message::Envelope>,
    ) {
        let Some(identity) = self.identity_for(draft.account, mailbox, original) else {
            return;
        };
        if let Some(account) = self.state.accounts.get(&draft.account)
            && !identity.email.eq_ignore_ascii_case(&account.email)
        {
            draft.from = Some(account.from_addr(&identity));
        }
        if let Some(signature) = &identity.signature {
            draft.body_text = format!("{}\n\n-- \n{signature}", draft.body_text);
        }
    }

    pub(super) fn draft_outcome(
        &mut self,
        receipt: CommandReceipt,
        data: Value,
        changes: CacheChanges,
    ) -> Outcome {
        let delta = self.delta(changes);
        let mut outcome = Outcome::new(receipt.with_data(data));
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome
    }

    fn fresh_draft_id(&self, ids: &mut IdGen) -> DraftId {
        loop {
            let id: DraftId = ids.next_id();
            if !self.state.drafts.contains_key(&id) {
                return id;
            }
        }
    }
}

/// The `Message-ID` of a draft: unique per draft and stable, in the sender's domain.
fn message_id(id: DraftId, email: &str) -> String {
    let domain = email.rsplit_once('@').map_or("dail.invalid", |(_, d)| d);
    format!("{}@{domain}", id.as_uuid().simple())
}

/// Validates `fields` and writes the given ones into `draft`.
fn apply_fields(draft: &mut Draft, fields: DraftFields) -> Result<(), CoreError> {
    if let Some(to) = fields.to {
        draft.to = validate_addrs("to", to)?;
    }
    if let Some(cc) = fields.cc {
        draft.cc = validate_addrs("cc", cc)?;
    }
    if let Some(bcc) = fields.bcc {
        draft.bcc = validate_addrs("bcc", bcc)?;
    }
    if let Some(subject) = fields.subject {
        if subject.chars().any(char::is_control) {
            return Err(invalid(
                "subject",
                "must not contain line breaks or control characters",
            ));
        }
        draft.subject = subject.trim().to_owned();
    }
    if let Some(body) = fields.body {
        if body.contains('\0') {
            return Err(invalid("body", "must not contain NUL characters"));
        }
        draft.body_text = body;
    }
    Ok(())
}

/// Checks an attachment path: absolute (Unix or Windows), naming a file, no control characters.
fn validate_path(path: &str) -> Result<(), CoreError> {
    if path.chars().any(char::is_control) || !is_absolute_path(path) {
        return Err(invalid("path", "must be an absolute file path"));
    }
    if file_name(path).is_empty() {
        return Err(invalid("path", "must name a file, not a directory"));
    }
    Ok(())
}

/// Checks the IDs a draft is saved and sent with ([`is_message_id`]).
fn validate_message_ids(draft: &Draft) -> Result<(), CoreError> {
    let reason = "must be a message ID without spaces, line breaks or control characters";
    if !is_message_id(&draft.message_id_hdr) {
        return Err(invalid("message_id_hdr", reason));
    }
    if draft
        .in_reply_to
        .as_deref()
        .is_some_and(|id| !is_message_id(id))
    {
        return Err(invalid("in_reply_to", reason));
    }
    if !draft.references.iter().all(|id| is_message_id(id)) {
        return Err(invalid("references", reason));
    }
    Ok(())
}

/// Checks each address and display name; drops duplicates (case-insensitive) and empty names.
fn validate_addrs(field: &str, addrs: Vec<Addr>) -> Result<Vec<Addr>, CoreError> {
    let mut out: Vec<Addr> = Vec::with_capacity(addrs.len());
    for addr in addrs {
        let email = validate_email(&addr.email).map_err(|_| {
            invalid(
                field,
                format!("`{}` is not an address like name@example.com", addr.email),
            )
        })?;
        let name = match addr.name.as_deref().map(str::trim) {
            Some(name) if name.chars().any(char::is_control) => {
                return Err(invalid(field, "names must not contain control characters"));
            }
            Some("") | None => None,
            Some(name) => Some(name.to_owned()),
        };
        if !out
            .iter()
            .any(|known| known.email.eq_ignore_ascii_case(&email))
        {
            out.push(Addr { name, email });
        }
    }
    Ok(out)
}

/// The valid addresses of `addrs` (an envelope's, which are email content): the others are left
/// out instead of failing the command.
fn valid_addrs(addrs: &[Addr]) -> Vec<Addr> {
    let mut out: Vec<Addr> = Vec::with_capacity(addrs.len());
    for addr in addrs {
        if let Ok(mut valid) = validate_addrs("to", vec![addr.clone()]).or_else(|_| {
            validate_addrs(
                "to",
                vec![Addr {
                    name: None,
                    email: addr.email.clone(),
                }],
            )
        }) {
            for valid in valid.drain(..) {
                if !out
                    .iter()
                    .any(|known| known.email.eq_ignore_ascii_case(&valid.email))
                {
                    out.push(valid);
                }
            }
        }
    }
    out
}

/// `text` without control characters (an original subject is email content).
fn strip_controls(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// `12.5 MB` (MiB, one decimal).
fn megabytes(bytes: u64) -> String {
    let tenths = bytes.saturating_mul(10) / (1024 * 1024);
    format!("{}.{} MB", tenths / 10, tenths % 10)
}

/// A draft in receipts. Recipients, subject, body, quote and the `In-Reply-To` of an answer may
/// come from email, so they sit inside `untrusted_content`.
pub(super) fn draft_json(draft: &Draft) -> Value {
    json!({
        "id": draft.id,
        "account": draft.account,
        "reply_to": draft.reply_to,
        "message_id": draft.message_id_hdr,
        "created_ms": draft.created_ms,
        "updated_ms": draft.updated_ms,
        "saved_ms": draft.saved_ms,
        "save_error": draft.save_error,
        "send_at": draft.send_at,
        "sending": draft.sending,
        "send_error": draft.send_error,
        "attachments": draft.attachments,
        "untrusted_content": {
            "in_reply_to": draft.in_reply_to,
            "to": draft.to,
            "cc": draft.cc,
            "bcc": draft.bcc,
            "subject": draft.subject,
            "body": draft.body_text,
            "quote": draft.quote,
        },
    })
}
