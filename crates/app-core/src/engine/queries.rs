//! Read-only mail queries: `list_mailboxes`, `list_threads`, `get_thread`, `get_message`.
//!
//! Subjects and addresses come from email and are attacker-controlled, so they are returned
//! inside an `untrusted_content` object (`AGENTS.md §4`).

use serde_json::{Value, json};

use crate::error::CoreError;
use crate::event::Effect;
use crate::ids::{AccountId, MailboxId, MessageId, ThreadId};
use crate::listing::{self, ThreadSummary};
use crate::message::{BodyState, Envelope, Thread};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::{Engine, invalid, not_found};

/// Largest page `list_threads` returns.
pub(super) const MAX_THREAD_PAGE: u32 = 500;

impl Engine {
    pub(super) fn list_mailboxes(
        &self,
        account: Option<AccountId>,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if let Some(id) = account
            && !self.state.accounts.contains_key(&id)
        {
            return Err(not_found("account", id));
        }
        let mailboxes = listing::mailboxes(&self.state, account);
        let mut receipt = receipt;
        if let Some(id) = account {
            receipt = receipt.affecting(EntityRef::Account(id));
        }
        let list: Vec<Value> = mailboxes
            .iter()
            .map(|m| {
                let history = listing::history(&self.state, m.id);
                json!({
                    "id": m.id,
                    "account": m.account,
                    "path": m.path,
                    "parent": m.parent_path(),
                    "depth": m.depth(),
                    "role": m.role,
                    "unread": m.unread,
                    "total": m.total,
                    "cached": history.map(|h| h.cached),
                    "window": m.sync_window,
                    "backfilling": self.state.status.backfilling.contains(&m.id),
                })
            })
            .collect();
        Ok(Outcome::new(
            receipt.with_data(json!({ "mailboxes": list })),
        ))
    }

    pub(super) fn list_contacts(
        &self,
        query: &str,
        limit: u32,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if limit == 0 || limit > MAX_THREAD_PAGE {
            return Err(invalid(
                "limit",
                format!("must be between 1 and {MAX_THREAD_PAGE}"),
            ));
        }
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let contacts: Vec<Value> = listing::contacts(&self.state, query, limit)
            .into_iter()
            .map(|c| {
                json!({
                    "count": c.count,
                    "last_ms": c.last_ms,
                    "written_to": c.written_to,
                    "untrusted_content": { "email": c.email, "name": c.name },
                })
            })
            .collect();
        Ok(Outcome::new(
            receipt.with_data(json!({ "contacts": contacts })),
        ))
    }

    pub(super) fn list_threads(
        &self,
        mailbox: MailboxId,
        offset: u32,
        limit: u32,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if limit == 0 || limit > MAX_THREAD_PAGE {
            return Err(invalid(
                "limit",
                format!("must be between 1 and {MAX_THREAD_PAGE}"),
            ));
        }
        if !self.state.mailboxes.contains_key(&mailbox) {
            return Err(not_found("mailbox", mailbox));
        }
        let threads = listing::threads_in(&self.state, mailbox);
        let total = threads.len();
        let skip = usize::try_from(offset).unwrap_or(usize::MAX);
        let take = usize::try_from(limit).unwrap_or(usize::MAX);
        let page: Vec<Value> = threads
            .into_iter()
            .skip(skip)
            .take(take)
            .map(|thread| self.thread_summary(thread, Some(mailbox)))
            .collect();
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::Mailbox(mailbox))
                .with_data(json!({
                    "mailbox": mailbox,
                    "total": total,
                    "offset": offset,
                    "limit": limit,
                    "threads": page,
                })),
        ))
    }

    /// One `list_threads` / `search` entry. `in_mailbox` counts the messages shown in
    /// `mailbox`, if one is given.
    pub(super) fn thread_summary(&self, thread: &Thread, mailbox: Option<MailboxId>) -> Value {
        let summary = ThreadSummary::of(&self.state, thread);
        let mut json = json!({
            "id": thread.id,
            "account": thread.account,
            "messages": summary.messages.len(),
            "unread": summary.unread,
            "flagged": summary.flagged,
            "has_attachments": summary.has_attachments,
            "last_date_ms": thread.last_date_ms,
            "untrusted_content": {
                "subject": thread.subject,
                "from": summary.senders,
                "snippet": thread
                    .messages
                    .iter()
                    .rev()
                    .filter_map(|m| self.state.messages.get(m))
                    .find_map(|m| m.snippet.as_deref()),
            },
        });
        if let (Some(mailbox), Some(object)) = (mailbox, json.as_object_mut()) {
            object.insert(
                "in_mailbox".to_owned(),
                Value::from(summary.in_mailbox(mailbox)),
            );
        }
        json
    }

    pub(super) fn get_thread(
        &self,
        id: ThreadId,
        include_body: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let thread = self
            .state
            .threads
            .get(&id)
            .ok_or_else(|| not_found("thread", id))?;
        let envelopes: Vec<&Envelope> = thread
            .messages
            .iter()
            .filter_map(|m| self.state.messages.get(m))
            .collect();
        let effects = if include_body {
            self.body_downloads(envelopes.iter().copied())
        } else {
            Vec::new()
        };
        let messages: Vec<Value> = envelopes
            .iter()
            .map(|e| self.message_json(e, include_body))
            .collect();
        Ok(Outcome {
            receipt: receipt.affecting(EntityRef::Thread(id)).with_data(json!({
                "id": thread.id,
                "account": thread.account,
                "last_date_ms": thread.last_date_ms,
                "untrusted_content": { "subject": thread.subject },
                "messages": messages,
            })),
            effects,
        })
    }

    pub(super) fn get_message(
        &self,
        id: MessageId,
        include_body: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let envelope = self
            .state
            .messages
            .get(&id)
            .ok_or_else(|| not_found("message", id))?;
        let effects = if include_body {
            self.body_downloads(std::iter::once(envelope))
        } else {
            Vec::new()
        };
        Ok(Outcome {
            receipt: receipt
                .affecting(EntityRef::Message(id))
                .with_data(json!({ "message": self.message_json(envelope, include_body) })),
            effects,
        })
    }

    /// `FetchBody` effects for messages without a cached body that are not being downloaded.
    /// Queries do not change state, so the downloads are not recorded in
    /// `status.bodies_in_flight`.
    fn body_downloads<'a>(&self, envelopes: impl Iterator<Item = &'a Envelope>) -> Vec<Effect> {
        envelopes
            .filter(|e| !matches!(e.body, BodyState::Fetched { .. }))
            .filter(|e| !self.state.status.bodies_in_flight.contains(&e.id))
            .filter_map(|e| self.body_fetch_effect(e.id))
            .collect()
    }

    /// One message for `get_thread` / `get_message`. With `include_body`, a cached body is
    /// referenced as `body: {state: "fetched", blob}`; the runtime replaces the reference with
    /// the parsed text in `untrusted_content.body` (the core cannot read blobs).
    fn message_json(&self, envelope: &Envelope, include_body: bool) -> Value {
        let body = match (&envelope.body, include_body) {
            (BodyState::Fetched { .. }, false) => json!({ "state": "fetched" }),
            // reason: serializing a plain enum cannot fail; `Null` is a harmless fallback.
            (body, _) => serde_json::to_value(body).unwrap_or_default(),
        };
        json!({
            "id": envelope.id,
            "account": envelope.account,
            "mailbox": envelope.location(),
            "moving": envelope.moved_to.is_some() || envelope.is_placeholder(),
            "thread": envelope.thread,
            "date_ms": envelope.date_ms,
            "flags": envelope.flags,
            "size": envelope.size,
            "has_attachments": envelope.has_attachments,
            "body": body,
            "downloading": self.state.status.bodies_in_flight.contains(&envelope.id),
            "untrusted_content": {
                "subject": envelope.subject,
                "from": envelope.from,
                "to": envelope.to,
                "cc": envelope.cc,
                "message_id": envelope.message_id_hdr,
                "snippet": envelope.snippet,
            },
        })
    }
}
