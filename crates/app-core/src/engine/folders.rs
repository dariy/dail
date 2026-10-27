//! Folder management (UX-14): `create_folder`, `rename_folder`, `delete_folder`. Each changes
//! the cached mailboxes first and emits one `ManageFolder`; its result lists the account's
//! folders again, which confirms the change or (after a failure) puts the server's back.

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, FolderAction, PersistDelta};
use crate::ids::{AccountId, MailboxId};
use crate::mailbox::{FolderKey, Mailbox, MailboxRole};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::SyncIssue;

use super::sync::CacheChanges;
use super::{Applied, Engine, invalid, not_found};

/// The longest folder path accepted.
const MAX_PATH: usize = 512;

/// A folder path as typed: trimmed, not empty, without control characters, IMAP wildcards or
/// empty segments.
fn folder_path(path: &str, separator: &str) -> Result<String, CoreError> {
    let path = path.trim().to_owned();
    let bad = path.is_empty()
        || path.len() > MAX_PATH
        || path.chars().any(|c| c.is_control() || c == '*' || c == '%')
        || path.split(separator).any(str::is_empty);
    if bad {
        return Err(invalid(
            "path",
            format!("expected a folder path like Work{separator}Clients"),
        ));
    }
    Ok(path)
}

impl Engine {
    /// The account's hierarchy delimiter, as its mailboxes report it.
    fn separator_of(&self, account: AccountId) -> String {
        self.state
            .mailboxes
            .values()
            .find(|m| m.account == account && m.delimiter.is_some())
            .map_or_else(|| "/".to_owned(), |m| m.separator().to_owned())
    }

    fn has_folder(&self, account: AccountId, path: &str) -> bool {
        self.state
            .mailboxes
            .values()
            .any(|m| m.account == account && m.path.eq_ignore_ascii_case(path))
    }

    fn folder_effect(&self, account: AccountId, action: FolderAction) -> Option<Effect> {
        let imap = self.state.accounts.get(&account)?.imap_login();
        Some(Effect::ManageFolder {
            account,
            imap,
            action,
        })
    }

    /// The outcome: persist `changes`, then run `effect`.
    fn folder_outcome(
        &mut self,
        receipt: CommandReceipt,
        changes: CacheChanges,
        effect: Option<Effect>,
    ) -> Outcome {
        let delta = self.delta(changes);
        let mut outcome = Outcome::new(receipt);
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(effect);
        outcome
    }

    pub(super) fn create_folder(
        &mut self,
        account: AccountId,
        path: &str,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !self.state.accounts.contains_key(&account) {
            return Err(not_found("account", account));
        }
        let separator = self.separator_of(account);
        let path = folder_path(path, &separator)?;
        if self.has_folder(account, &path) {
            return Err(invalid("path", format!("{path} exists already")));
        }
        let mut ids = ctx.ids();
        let id = self.fresh_mailbox_id(&mut ids);
        self.state.mailboxes.insert(
            id,
            Mailbox {
                id,
                account,
                path: path.clone(),
                delimiter: Some(separator),
                role: None,
                uid_validity: None,
                uid_next: None,
                unread: 0,
                total: 0,
                highest_modseq: None,
                sync_window: None,
                history_complete: false,
                identity: None,
                last_fetch_ms: None,
                custom_keywords: None,
            },
        );
        let mut changes = CacheChanges::default();
        changes.mailboxes.insert(id);
        let effect = self.folder_effect(account, FolderAction::Create { path: path.clone() });
        let receipt = receipt
            .affecting(EntityRef::Account(account))
            .affecting(EntityRef::Mailbox(id))
            .with_data(serde_json::json!({ "mailbox": id, "path": path, "undoable": false }));
        Ok(self.folder_outcome(receipt, changes, effect))
    }

    pub(super) fn rename_folder(
        &mut self,
        id: MailboxId,
        path: &str,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let mailbox = self
            .state
            .mailboxes
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("mailbox", id))?;
        if mailbox.role == Some(MailboxRole::Inbox) {
            return Err(invalid("id", "the inbox can't be renamed"));
        }
        let separator = mailbox.separator().to_owned();
        let to = folder_path(path, &separator)?;
        if to == mailbox.path {
            return Err(invalid("path", "that is its name already"));
        }
        if to.starts_with(&format!("{}{separator}", mailbox.path)) {
            return Err(invalid("path", "a folder can't move into itself"));
        }
        if self.has_folder(mailbox.account, &to) {
            return Err(invalid("path", format!("{to} exists already")));
        }
        let from = mailbox.path.clone();
        let below = format!("{from}{separator}");
        let mut changes = CacheChanges::default();
        let mut receipt = receipt.affecting(EntityRef::Account(mailbox.account));
        for entry in self.state.mailboxes.values_mut() {
            if entry.account != mailbox.account {
                continue;
            }
            let renamed = if entry.path == from {
                Some(to.clone())
            } else {
                entry
                    .path
                    .strip_prefix(&below)
                    .map(|rest| format!("{to}{separator}{rest}"))
            };
            if let Some(path) = renamed {
                entry.path = path;
                changes.mailboxes.insert(entry.id);
                receipt = receipt.affecting(EntityRef::Mailbox(entry.id));
            }
        }
        let effect = self.folder_effect(
            mailbox.account,
            FolderAction::Rename {
                from: from.clone(),
                to: to.clone(),
            },
        );
        let receipt =
            receipt.with_data(serde_json::json!({ "mailbox": id, "from": from, "to": to }));
        let mut outcome = self.folder_outcome(receipt, changes, effect);
        // Pins, hidden folders and collapsed parents follow the folder.
        let moved = |key: &FolderKey| -> Option<FolderKey> {
            if key.account != mailbox.account {
                return None;
            }
            let path = if key.path == from {
                to.clone()
            } else {
                format!("{to}{separator}{}", key.path.strip_prefix(&below)?)
            };
            Some(FolderKey {
                account: key.account,
                path,
            })
        };
        let settings = &mut self.state.settings;
        let mut touched = false;
        for key in &mut settings.pinned_mailboxes {
            if let Some(new) = moved(key) {
                *key = new;
                touched = true;
            }
        }
        let hidden = std::mem::take(&mut settings.hidden_mailboxes);
        for key in hidden {
            let new = moved(&key);
            touched |= new.is_some();
            settings.hidden_mailboxes.insert(new.unwrap_or(key));
        }
        if touched {
            outcome.effects.push(Effect::PersistState {
                delta: PersistDelta::Settings(self.state.settings.clone()),
            });
        }
        let collapsed = std::mem::take(&mut self.state.view.collapsed);
        for key in collapsed {
            let new = moved(&key).unwrap_or(key);
            self.state.view.collapsed.insert(new);
        }
        Ok(Applied {
            outcome,
            undo: Some((
                format!("Rename {from}"),
                vec![Command::RenameFolder { id, path: from }],
            )),
        })
    }

    pub(super) fn delete_folder(
        &mut self,
        id: MailboxId,
        confirm: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !confirm {
            return Err(CoreError::ConfirmationRequired {
                command: "delete_folder".to_owned(),
            });
        }
        let mailbox = self
            .state
            .mailboxes
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("mailbox", id))?;
        if mailbox.role == Some(MailboxRole::Inbox) {
            return Err(invalid("id", "the inbox can't be deleted"));
        }
        let below = format!("{}{}", mailbox.path, mailbox.separator());
        if self
            .state
            .mailboxes
            .values()
            .any(|m| m.account == mailbox.account && m.path.starts_with(&below))
        {
            return Err(invalid("id", "delete its subfolders first"));
        }
        let mut changes = CacheChanges::default();
        self.drop_mailbox(id, &mut changes);
        let effect = self.folder_effect(
            mailbox.account,
            FolderAction::Delete {
                path: mailbox.path.clone(),
            },
        );
        let receipt = receipt
            .affecting(EntityRef::Account(mailbox.account))
            .affecting(EntityRef::Mailbox(id))
            .with_data(serde_json::json!({
                "deleted": id,
                "path": mailbox.path,
                "undoable": false,
            }));
        Ok(self.folder_outcome(receipt, changes, effect))
    }

    /// `FolderManaged` / `FolderFailed`: list the account's folders again (the server's view
    /// wins); a failure is kept in the account's `folder_error` until a folder change succeeds.
    pub(super) fn ingest_folder(
        &mut self,
        account: AccountId,
        error: Option<String>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        if !self.state.accounts.contains_key(&account) {
            return Outcome::new(receipt);
        }
        let receipt = receipt
            .affecting(EntityRef::Account(account))
            .affecting(EntityRef::Status);
        self.state
            .status
            .sync
            .entry(account)
            .or_default()
            .folder_error = error.map(|error| SyncIssue {
            error,
            mailbox: None,
            at_ms: ctx.now_ms,
        });
        let mut outcome = Outcome::new(receipt);
        outcome.effects.extend(self.start_listing(account));
        outcome
    }

    /// `KeywordsRefused`: the mailbox keeps pins on the device from now on.
    pub(super) fn ingest_keywords_refused(
        &mut self,
        account: AccountId,
        path: &str,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(mailbox) = self
            .state
            .mailboxes
            .values_mut()
            .find(|m| m.account == account && m.path == path)
        else {
            return Outcome::new(receipt);
        };
        if mailbox.custom_keywords == Some(false) {
            return Outcome::new(receipt);
        }
        mailbox.custom_keywords = Some(false);
        let id = mailbox.id;
        let mut changes = CacheChanges::default();
        changes.mailboxes.insert(id);
        self.folder_outcome(receipt.affecting(EntityRef::Mailbox(id)), changes, None)
    }
}
