//! History backfill (`ARCHITECTURE.md §5.2`): a mailbox keeps its newest `sync_window`
//! messages cached (`set_sync_window`; the `initial_window` setting by default, 0 = all). While
//! fewer are cached, the engine asks for the next older batch (`Effect::FetchOlder`, at most
//! [`BACKFILL_BATCH`] below the oldest cached UID), one batch per mailbox at a time, after each
//! fetch of the mailbox and after each batch. Not counted as sync pending: polls go on.

use std::collections::BTreeMap;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, PersistDelta};
use crate::ids::{AccountId, MailboxId, MessageId};
use crate::listing;
use crate::message::RemoteEnvelope;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::sync::{CacheChanges, MergeTarget};
use super::{Applied, Engine, not_found};

/// Envelopes per backfill batch.
pub const BACKFILL_BATCH: u32 = 200;

impl Engine {
    /// The next `FetchOlder` for `mailbox`, if its window wants more and none is in flight.
    pub(super) fn maybe_backfill(&mut self, mailbox: MailboxId) -> Option<Effect> {
        if self.state.status.backfilling.contains(&mailbox) {
            return None;
        }
        let history = listing::history(&self.state, mailbox)?;
        let wanted = history
            .wanted
            .checked_sub(history.cached)
            .filter(|n| *n > 0)?;
        let mb = self.state.mailboxes.get(&mailbox)?;
        if mb.history_complete {
            return None;
        }
        let before_uid = history.oldest_uid?;
        let account = self.state.accounts.get(&mb.account)?;
        let effect = Effect::FetchOlder {
            account: account.id,
            mailbox,
            path: mb.path.clone(),
            imap: account.imap_login(),
            uid_validity: mb.uid_validity?,
            before_uid,
            limit: wanted.min(BACKFILL_BATCH),
        };
        self.state.status.backfilling.insert(mailbox);
        Some(effect)
    }

    pub(super) fn set_sync_window(
        &mut self,
        mailbox: MailboxId,
        window: Option<u32>,
        receipt: CommandReceipt,
    ) -> Result<Applied, CoreError> {
        let current = self
            .state
            .mailboxes
            .get_mut(&mailbox)
            .ok_or_else(|| not_found("mailbox", mailbox))?;
        let previous = current.sync_window;
        current.sync_window = window;
        let path = current.path.clone();
        let changed = previous != window;
        let mut outcome = Outcome::new(receipt.affecting(EntityRef::Mailbox(mailbox)).with_data(
            serde_json::json!({
                "mailbox": mailbox,
                "window": window,
                "previous": previous,
            }),
        ));
        if !changed {
            return Ok(Applied::plain(outcome));
        }
        let delta = self.delta(CacheChanges {
            mailboxes: [mailbox].into_iter().collect(),
            ..CacheChanges::default()
        });
        outcome = outcome.with_effect(Effect::PersistState {
            delta: PersistDelta::Mail(delta),
        });
        if let Some(fetch) = self.maybe_backfill(mailbox) {
            outcome = outcome.with_effect(fetch);
        }
        let label = match window {
            Some(0) => format!("Keep all of {path} offline"),
            Some(n) => format!("Keep the newest {n} of {path} offline"),
            None => format!("Default offline window for {path}"),
        };
        Ok(Applied {
            outcome,
            undo: Some((
                label,
                vec![Command::SetSyncWindow {
                    mailbox,
                    window: previous,
                }],
            )),
        })
    }

    pub(super) fn ingest_older(
        &mut self,
        account: AccountId,
        mailbox: MailboxId,
        older: Result<(u32, Vec<RemoteEnvelope>, bool), String>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.status.backfilling.remove(&mailbox);
        let receipt = receipt
            .affecting(EntityRef::Status)
            .affecting(EntityRef::Mailbox(mailbox));
        let Some(target) = self
            .state
            .mailboxes
            .get(&mailbox)
            .filter(|m| m.account == account)
            .cloned()
        else {
            return Outcome::new(receipt);
        };
        let (uid_validity, envelopes, complete) = match older {
            Ok(older) => older,
            // The next fetch of the mailbox tries again.
            Err(error) => {
                return Outcome::new(receipt.with_data(serde_json::json!({ "error": error })));
            }
        };
        if target.uid_validity != Some(uid_validity) {
            // Re-synced meanwhile; the next fetch starts the backfill over.
            return Outcome::new(receipt);
        }
        let mut changes = CacheChanges::default();
        let by_uid: BTreeMap<u32, MessageId> = self
            .state
            .messages
            .values()
            .filter(|m| m.mailbox == mailbox && !m.is_placeholder())
            .map(|m| (m.uid, m.id))
            .collect();
        let mut merge = MergeTarget {
            account,
            mailbox,
            by_uid,
            placeholders: super::sync::placeholders_in(&self.state, mailbox),
            threader: self.take_threader(account),
            ids: ctx.ids(),
        };
        let fetched = envelopes.len();
        for remote in envelopes {
            self.merge_remote(remote, &mut merge, ctx, &mut changes);
        }
        self.keep_threader(account, std::mem::take(&mut merge.threader));
        // Nothing older came back: there is nothing older (a guard against asking forever).
        if (complete || fetched == 0)
            && let Some(mb) = self.state.mailboxes.get_mut(&mailbox)
        {
            mb.history_complete = true;
            changes.mailboxes.insert(mailbox);
        }
        let delta = self.delta(changes);
        let mut outcome = Outcome::new(receipt.with_data(serde_json::json!({
            "mailbox": mailbox,
            "fetched": fetched,
            "complete": complete || fetched == 0,
        })));
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        if let Some(next) = self.maybe_backfill(mailbox) {
            outcome = outcome.with_effect(next);
        }
        outcome
    }
}
