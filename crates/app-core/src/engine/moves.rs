//! `archive_threads`, `trash_threads`, `move_threads` (`ARCHITECTURE.md §5.2, §5.3`).
//!
//! A move is shown at once through [`Envelope::moved_to`] while `mailbox`/`uid` keep the server
//! location; the op moves the message on the server later. Moving out of a mailbox with the
//! `all` role (Gmail's All Mail) into anything but Trash/Junk is a copy: the message stays in
//! All Mail and a placeholder appears in the target. Targets come from the account's preset
//! (`PresetProfile`): provider differences are data.

use indexmap::IndexMap;

use crate::account::ProviderPreset;
use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, IdGen, MailboxId, MessageId, ThreadId};
use crate::mailbox::{Mailbox, MailboxRole};
use crate::message::Envelope;
use crate::ops::{CopyRef, RemoteOp};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::sync::{CacheChanges, push_unique};
use super::triage::count;
use super::{Applied, Engine, invalid, not_found};

/// Where a move goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MoveKind {
    /// The account's archive.
    Archive,
    /// The account's Trash.
    Trash,
    /// An explicit mailbox.
    To(MailboxId),
}

/// Arguments of a move command.
pub(super) struct MoveRequest {
    pub kind: MoveKind,
    pub threads: Vec<ThreadId>,
    pub messages: Vec<MessageId>,
}

/// The target mailbox of one account: an existing one, or one to create.
#[derive(Debug, Clone)]
enum Target {
    Existing(MailboxId),
    Create {
        path: String,
        delimiter: Option<String>,
    },
}

impl MoveKind {
    fn title(self) -> &'static str {
        match self {
            Self::Archive => "Archive",
            Self::Trash => "Move to Trash",
            Self::To(_) => "Move",
        }
    }

    /// Whether a message shown in a mailbox with `role` is moved when its thread is.
    fn takes_from(self, role: Option<MailboxRole>) -> bool {
        match self {
            Self::Archive => matches!(role, None | Some(MailboxRole::Inbox | MailboxRole::Junk)),
            Self::Trash => !matches!(
                role,
                Some(MailboxRole::Trash | MailboxRole::All | MailboxRole::Flagged)
            ),
            Self::To(_) => !matches!(
                role,
                Some(
                    MailboxRole::Sent
                        | MailboxRole::Drafts
                        | MailboxRole::All
                        | MailboxRole::Flagged
                )
            ),
        }
    }

    /// Whether a message that is only in All Mail is taken from there.
    const fn falls_back_to_all(self) -> bool {
        !matches!(self, Self::Archive)
    }
}

impl Engine {
    pub(super) fn move_messages(
        &mut self,
        request: MoveRequest,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        if request.threads.is_empty() && request.messages.is_empty() {
            return Err(invalid("ids", "give `ids` (threads), `messages` or both"));
        }
        let mut accounts: Vec<AccountId> = Vec::new();
        for id in &request.threads {
            let thread = self
                .state
                .threads
                .get(id)
                .ok_or_else(|| not_found("thread", id))?;
            push_unique(&mut accounts, thread.account);
        }
        for id in &request.messages {
            let message = self
                .state
                .messages
                .get(id)
                .ok_or_else(|| not_found("message", id))?;
            push_unique(&mut accounts, message.account);
        }
        let mut targets: IndexMap<AccountId, Target> = IndexMap::new();
        for account in accounts {
            targets.insert(account, self.resolve_target(account, request.kind)?);
        }

        let picked = self.pick_sources(&request, &targets);
        if picked.is_empty() {
            return Ok(Applied::plain(Outcome::new(receipt.with_data(
                serde_json::json!({ "moved": [], "copied": [], "cancelled": [], "ops": [] }),
            ))));
        }

        let mut ids = ctx.ids();
        let mut changes = CacheChanges::default();
        let mut resolved: IndexMap<AccountId, MailboxId> = IndexMap::new();
        for (account, target) in targets {
            let id = match target {
                Target::Existing(id) => id,
                Target::Create { path, delimiter } => {
                    let id = self.fresh_mailbox_id(&mut ids);
                    self.state.mailboxes.insert(
                        id,
                        Mailbox {
                            id,
                            account,
                            path,
                            delimiter,
                            role: Some(MailboxRole::Archive),
                            uid_validity: None,
                            uid_next: None,
                            unread: 0,
                            total: 0,
                            highest_modseq: None,
                            sync_window: None,
                            history_complete: false,
                            last_fetch_ms: None,
                            custom_keywords: None,
                            identity: None,
                        },
                    );
                    changes.mailboxes.insert(id);
                    id
                }
            };
            resolved.insert(account, id);
        }

        let mut moved: Vec<MessageId> = Vec::new();
        let mut copied: Vec<MessageId> = Vec::new();
        let mut cancelled: Vec<MessageId> = Vec::new();
        let mut moves: IndexMap<AccountId, Vec<MessageId>> = IndexMap::new();
        let mut copies: IndexMap<AccountId, Vec<CopyRef>> = IndexMap::new();
        // Inverse: messages by the mailbox they were shown in.
        let mut back: IndexMap<MailboxId, Vec<MessageId>> = IndexMap::new();
        for id in picked {
            let Some(envelope) = self.state.messages.get(&id).cloned() else {
                continue;
            };
            let Some(&to) = resolved.get(&envelope.account) else {
                continue;
            };
            let from = envelope.location();
            let target_role = self.role(to);
            let copy = self.role(from) == Some(MailboxRole::All)
                && !matches!(target_role, Some(MailboxRole::Trash | MailboxRole::Junk));
            if copy {
                let placeholder = self.fresh_message_id(&mut ids);
                self.add_placeholder(&envelope, placeholder, to, &mut changes);
                copies.entry(envelope.account).or_default().push(CopyRef {
                    source: id,
                    copy: placeholder,
                });
                copied.push(placeholder);
                back.entry(from).or_default().push(placeholder);
            } else if self.cancel_copy(id, to, &mut changes) {
                // Moved back before its copy was made: the copy is simply not made.
                cancelled.push(id);
                back.entry(from).or_default().push(id);
            } else {
                // A move that was not sent yet is replaced by this one; if the message goes back
                // to where it is on the server, nothing has to be sent at all.
                let unsent = self.cancel_move(id, &mut changes);
                if let Some(entry) = self.state.messages.get_mut(&id) {
                    entry.moved_to = (entry.mailbox != to).then_some(to);
                }
                changes.messages.insert(id);
                changes.threads.insert(envelope.thread);
                if unsent && envelope.mailbox == to && !self.move_pending(id) {
                    cancelled.push(id);
                } else {
                    moves.entry(envelope.account).or_default().push(id);
                    moved.push(id);
                }
                back.entry(from).or_default().push(id);
            }
        }

        let mut receipt = receipt;
        let mut effects = Vec::new();
        let mut ops = Vec::new();
        for (account, to) in &resolved {
            let mut queued = Vec::new();
            if let Some(messages) = moves.shift_remove(account) {
                queued.push(RemoteOp::Move { to: *to, messages });
            }
            if let Some(copies) = copies.shift_remove(account) {
                queued.push(RemoteOp::Copy { to: *to, copies });
            }
            for op in queued {
                let id = self.queue_op(*account, op, ctx, &mut ids, &mut changes);
                ops.push(id);
            }
            effects.extend(self.pump_ops(*account, ctx.now_ms, &mut changes));
            receipt = receipt.affecting(EntityRef::Mailbox(*to));
        }
        let delta = self.delta(changes);
        for thread in &delta.threads {
            receipt = receipt.affecting(EntityRef::Thread(thread.id));
        }
        for id in &ops {
            receipt = receipt.affecting(EntityRef::Op(*id));
        }
        let total = moved.len() + copied.len() + cancelled.len();
        let data = serde_json::json!({
            "moved": moved,
            "copied": copied,
            "cancelled": cancelled,
            "ops": ops,
            "to": resolved.values().collect::<Vec<_>>(),
        });
        let mut outcome = Outcome::new(receipt.with_data(data)).with_effect(Effect::PersistState {
            delta: PersistDelta::Mail(delta),
        });
        outcome.effects.extend(effects);
        let inverse = back
            .into_iter()
            .map(|(to, messages)| Command::MoveThreads {
                ids: Vec::new(),
                messages,
                to,
            })
            .collect();
        let label = format!("{} ({})", request.kind.title(), count(total));
        Ok(Applied {
            outcome,
            undo: Some((label, inverse)),
        })
    }

    /// The mailbox `kind` moves `account`'s messages to.
    fn resolve_target(&self, account: AccountId, kind: MoveKind) -> Result<Target, CoreError> {
        let preset = self
            .state
            .accounts
            .get(&account)
            .map_or(ProviderPreset::Custom, |a| a.preset);
        let profile = preset.profile();
        let by_role = |role| self.account_mailbox(account, |m| m.role == Some(role));
        let by_path = |path: Option<&str>| {
            path.and_then(|path| self.account_mailbox(account, |m| m.path == path))
        };
        let missing = |role| CoreError::NoMailboxForRole {
            account: account.to_string(),
            role,
        };
        match kind {
            MoveKind::To(id) => {
                let mailbox = self
                    .state
                    .mailboxes
                    .get(&id)
                    .ok_or_else(|| not_found("mailbox", id))?;
                if mailbox.account != account {
                    return Err(invalid(
                        "to",
                        "the mailbox belongs to another account than the messages",
                    ));
                }
                Ok(Target::Existing(id))
            }
            MoveKind::Trash => by_role(MailboxRole::Trash)
                .or_else(|| by_path(profile.trash_mailbox))
                .map(Target::Existing)
                .ok_or_else(|| missing(MailboxRole::Trash)),
            MoveKind::Archive => {
                if let Some(id) = by_role(MailboxRole::Archive)
                    .or_else(|| by_path(profile.archive_mailbox))
                    .or_else(|| {
                        profile
                            .archive_to_all
                            .then(|| by_role(MailboxRole::All))
                            .flatten()
                    })
                {
                    return Ok(Target::Existing(id));
                }
                match profile.archive_mailbox {
                    Some(name) if profile.create_archive => Ok(self.new_folder(account, name)),
                    _ => Err(missing(MailboxRole::Archive)),
                }
            }
        }
    }

    /// A top-level folder `name`, inside `INBOX` if the account keeps its folders there
    /// (`INBOX.Sent`, as some Dovecot servers do).
    fn new_folder(&self, account: AccountId, name: &str) -> Target {
        let inbox = self
            .state
            .mailboxes
            .values()
            .find(|m| m.account == account && m.role == Some(MailboxRole::Inbox));
        let delimiter = inbox.and_then(|m| m.delimiter.clone());
        let nested = delimiter.as_deref().is_some_and(|d| {
            let prefix = format!("INBOX{d}");
            self.state
                .mailboxes
                .values()
                .any(|m| m.account == account && m.role.is_some() && m.path.starts_with(&prefix))
        });
        let path = match (&delimiter, nested) {
            (Some(d), true) => format!("INBOX{d}{name}"),
            _ => name.to_owned(),
        };
        Target::Create { path, delimiter }
    }

    fn account_mailbox(
        &self,
        account: AccountId,
        matches: impl Fn(&Mailbox) -> bool,
    ) -> Option<MailboxId> {
        self.state
            .mailboxes
            .values()
            .find(|m| m.account == account && matches(m))
            .map(|m| m.id)
    }

    pub(super) fn role(&self, mailbox: MailboxId) -> Option<MailboxRole> {
        self.state.mailboxes.get(&mailbox).and_then(|m| m.role)
    }

    /// The messages a move takes, in order. For each thread, the copies of each message (same
    /// `Message-ID`) are considered together: every copy in a mailbox the move takes from goes;
    /// if there is none and no copy is in the target yet, its All Mail copy goes (not when
    /// archiving). Named messages always go unless already there.
    fn pick_sources(
        &self,
        request: &MoveRequest,
        targets: &IndexMap<AccountId, Target>,
    ) -> Vec<MessageId> {
        let target_of = |account: &AccountId| match targets.get(account) {
            Some(Target::Existing(id)) => Some(*id),
            _ => None,
        };
        let mut picked = Vec::new();
        for id in &request.threads {
            let Some(thread) = self.state.threads.get(id) else {
                continue;
            };
            let target = target_of(&thread.account);
            let mut groups: IndexMap<String, Vec<&Envelope>> = IndexMap::new();
            for envelope in thread
                .messages
                .iter()
                .filter_map(|m| self.state.messages.get(m))
            {
                let key = envelope
                    .message_id_hdr
                    .clone()
                    .unwrap_or_else(|| envelope.id.to_string());
                groups.entry(key).or_default().push(envelope);
            }
            for copies in groups.values() {
                let taken: Vec<MessageId> = copies
                    .iter()
                    .filter(|e| Some(e.location()) != target)
                    .filter(|e| request.kind.takes_from(self.role(e.location())))
                    .map(|e| e.id)
                    .collect();
                let already_there = copies.iter().any(|e| Some(e.location()) == target);
                if !taken.is_empty() {
                    for id in taken {
                        push_unique(&mut picked, id);
                    }
                } else if request.kind.falls_back_to_all()
                    && !already_there
                    && let Some(all) = copies
                        .iter()
                        .find(|e| self.role(e.location()) == Some(MailboxRole::All))
                {
                    push_unique(&mut picked, all.id);
                }
            }
        }
        for id in &request.messages {
            if let Some(envelope) = self.state.messages.get(id)
                && Some(envelope.location()) != target_of(&envelope.account)
            {
                push_unique(&mut picked, *id);
            }
        }
        picked
    }

    /// A placeholder for a copy of `source` in `to`: same headers, flags and body, UID unknown.
    fn add_placeholder(
        &mut self,
        source: &Envelope,
        id: MessageId,
        to: MailboxId,
        changes: &mut CacheChanges,
    ) {
        let placeholder = Envelope {
            id,
            mailbox: to,
            uid: 0,
            moved_to: None,
            ..source.clone()
        };
        self.state.messages.insert(id, placeholder);
        self.messages_changed();
        if let Some(thread) = self.state.threads.get_mut(&source.thread) {
            thread.messages.push(id);
        }
        changes.messages.insert(id);
        changes.threads.insert(source.thread);
    }

    /// After a move or copy reached the server: placeholders in `to` take over a cached copy
    /// that already sits there (same `Message-ID`, e.g. Gmail's All Mail), keeping their local
    /// flags; differing flags are queued as ops (the caller pumps the queue).
    pub(super) fn absorb_copies(
        &mut self,
        to: MailboxId,
        ctx: &Ctx,
        ids: &mut IdGen,
        changes: &mut CacheChanges,
    ) {
        let placeholders: Vec<MessageId> = self
            .state
            .messages
            .values()
            .filter(|m| m.mailbox == to && m.is_placeholder() && m.message_id_hdr.is_some())
            .map(|m| m.id)
            .collect();
        for id in placeholders {
            let Some(placeholder) = self.state.messages.get(&id) else {
                continue;
            };
            let header = placeholder.message_id_hdr.clone();
            let Some(existing) = self
                .state
                .messages
                .values()
                .find(|m| m.mailbox == to && !m.is_placeholder() && m.message_id_hdr == header)
                .map(|m| (m.id, m.uid, m.flags))
            else {
                continue;
            };
            let (old, uid, server_flags) = existing;
            self.remove_message(old, changes);
            if let Some(entry) = self.state.messages.get_mut(&id) {
                entry.uid = uid;
            }
            changes.messages.insert(id);
            self.push_flag_diff(id, server_flags, ctx, ids, changes);
        }
    }
}
