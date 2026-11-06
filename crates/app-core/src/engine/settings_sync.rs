//! Settings stored on the mail server (ADR 0015, SET-2): `pull_settings` and `push_settings`,
//! the `ReadSettingsBlob` / `WriteSettingsBlob` effects and their events. A pull merges the
//! newest server copy into `synced.settings`; a push writes the merged document back. Pulls also
//! run when the settings account's folders are listed (on connect and every full round) and
//! when its watch reports a change.

use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::Effect;
use crate::ids::AccountId;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::SyncIssue;
use crate::synced::{SettingsCopy, SyncedSettings, settings_folder};

use super::{Engine, invalid, not_found};

impl Engine {
    /// The account that holds the settings: the first account (SET-3 makes it choosable).
    pub(super) fn settings_account(&self) -> Option<AccountId> {
        self.state.accounts.keys().next().copied()
    }

    /// The IMAP path of `account`'s settings folder, with its hierarchy delimiter.
    pub(super) fn settings_path(&self, account: AccountId) -> String {
        settings_folder(&self.separator_of(account))
    }

    /// The `ReadSettingsBlob` effect for `account`, unless a pull is in flight.
    pub(super) fn start_settings_pull(&mut self, account: AccountId) -> Option<Effect> {
        if self.state.synced.pulling {
            return None;
        }
        let imap = self.state.accounts.get(&account)?.imap_login();
        let known = self
            .state
            .synced
            .copy
            .filter(|_| self.state.synced.source == Some(account));
        self.state.synced.pulling = true;
        Some(Effect::ReadSettingsBlob {
            account,
            imap,
            path: self.settings_path(account),
            known,
        })
    }

    pub(super) fn pull_settings(
        &mut self,
        account: Option<AccountId>,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let account = match account {
            Some(id) if self.state.accounts.contains_key(&id) => id,
            Some(id) => return Err(not_found("account", id)),
            None => self
                .settings_account()
                .ok_or_else(|| invalid("account", "add an account first"))?,
        };
        let effect = self.start_settings_pull(account);
        let receipt = receipt
            .affecting(EntityRef::Account(account))
            .affecting(EntityRef::Settings)
            .with_data(serde_json::json!({
                "account": account,
                "started": effect.is_some(),
            }));
        let mut outcome = Outcome::new(receipt);
        outcome.effects.extend(effect);
        Ok(outcome)
    }

    pub(super) fn push_settings(
        &mut self,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let account = self
            .settings_account()
            .ok_or_else(|| invalid("account", "add an account first"))?;
        if self.state.synced.pulled_ms.is_none() {
            return Err(invalid(
                "settings",
                "run pull_settings first, so that a newer server copy is not overwritten",
            ));
        }
        if self.state.synced.pushing {
            return Err(invalid(
                "settings",
                "a push is running already; wait for it",
            ));
        }
        let imap = self
            .state
            .accounts
            .get(&account)
            .ok_or_else(|| not_found("account", account))?
            .imap_login();
        let mut doc = self.state.synced.settings.clone();
        doc.prune(ctx.now_ms);
        let bytes = doc
            .to_json()
            .map_err(|err| invalid("settings", err.to_string()))?;
        let document = String::from_utf8(bytes)
            .map_err(|err| invalid("settings", format!("not UTF-8: {err}")))?;
        self.state.synced.settings = doc;
        self.state.synced.pushing = true;
        let path = self.settings_path(account);
        let receipt = receipt
            .affecting(EntityRef::Account(account))
            .affecting(EntityRef::Settings)
            .with_data(serde_json::json!({
                "account": account,
                "path": path,
                "bytes": document.len(),
            }));
        Ok(
            Outcome::new(receipt).with_effect(Effect::WriteSettingsBlob {
                account,
                imap,
                path,
                document,
                date_ms: ctx.now_ms,
            }),
        )
    }

    /// `SettingsRead`: merge a valid copy; ignore an invalid one with its error in
    /// `synced.error` (it is remembered as read, so it is not downloaded again).
    pub(super) fn ingest_settings_read(
        &mut self,
        account: AccountId,
        copy: Option<SettingsCopy>,
        document: Option<&str>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let synced = &mut self.state.synced;
        synced.pulling = false;
        synced.source = Some(account);
        synced.copy = copy;
        synced.pulled_ms = Some(ctx.now_ms);
        synced.error = None;
        if let Some(text) = document {
            match SyncedSettings::from_json(text.as_bytes()) {
                Ok(remote) => {
                    let mut merged = synced.settings.merge(&remote);
                    merged.prune(ctx.now_ms);
                    synced.settings = merged;
                }
                Err(err) => {
                    synced.error = Some(SyncIssue {
                        error: err.to_string(),
                        mailbox: None,
                        at_ms: ctx.now_ms,
                    });
                }
            }
        }
        Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .affecting(EntityRef::Settings),
        )
    }

    /// `SettingsWritten`: read the folder again, which records the new copy as known.
    pub(super) fn ingest_settings_written(
        &mut self,
        account: AccountId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.synced.pushing = false;
        self.state.synced.pushed_ms = Some(ctx.now_ms);
        self.state.synced.error = None;
        let mut outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .affecting(EntityRef::Settings),
        );
        if self.state.accounts.contains_key(&account) {
            outcome.effects.extend(self.start_settings_pull(account));
        }
        outcome
    }

    /// `SettingsFailed`: the pull or push in flight ended; the error is shown in `synced.error`.
    pub(super) fn ingest_settings_failed(
        &mut self,
        account: AccountId,
        write: bool,
        error: String,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let synced = &mut self.state.synced;
        if write {
            synced.pushing = false;
        } else {
            synced.pulling = false;
        }
        synced.error = Some(SyncIssue {
            error,
            mailbox: None,
            at_ms: ctx.now_ms,
        });
        Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .affecting(EntityRef::Settings),
        )
    }

    /// After the settings account's folders were listed (on connect and every full round):
    /// pull if the settings folder exists; otherwise there is nothing on the server to read,
    /// which counts as a finished pull.
    pub(super) fn pull_on_listing(
        &mut self,
        account: AccountId,
        has_folder: bool,
        now_ms: i64,
    ) -> Option<Effect> {
        if self.settings_account() != Some(account) {
            return None;
        }
        if has_folder {
            return self.start_settings_pull(account);
        }
        let synced = &mut self.state.synced;
        if !synced.pulling {
            synced.source = Some(account);
            synced.copy = None;
            synced.pulled_ms = Some(now_ms);
        }
        None
    }

    /// A pull after the settings account's watch saw a change, once a first pull has run.
    pub(super) fn settings_pull_after_change(&mut self, account: AccountId) -> Option<Effect> {
        if self.settings_account() != Some(account) || self.state.synced.copy.is_none() {
            return None;
        }
        self.start_settings_pull(account)
    }
}
