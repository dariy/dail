//! Push for the INBOX (`ARCHITECTURE.md §5.2`): after an INBOX sync the engine asks the runtime
//! to watch the mailbox (`Effect::WatchMailbox`, IMAP IDLE). A reported change fetches the INBOX
//! and watches again; a quiet timeout watches again; a failure retries after
//! [`WATCH_RETRY_MS`]; a server without IDLE stays on polling. While an account is watched,
//! the INBOX poll skips it.

use crate::ctx::Ctx;
use crate::event::Effect;
use crate::ids::{AccountId, MailboxId};
use crate::mailbox::MailboxRole;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::WatchState;

use super::Engine;

/// How long a failed watch waits before it starts again, in milliseconds.
pub const WATCH_RETRY_MS: i64 = 60_000;

impl Engine {
    /// The `WatchMailbox` effect for `account`'s INBOX, if it is not watched yet (or its retry
    /// is due at `now_ms`). Records it as watching.
    pub(super) fn start_watch(&mut self, account: AccountId, now_ms: i64) -> Option<Effect> {
        let status = self.state.status.sync.get(&account);
        let due = match status.map(|s| &s.watch) {
            None | Some(WatchState::Off) => true,
            Some(WatchState::Retrying { at_ms, .. }) => *at_ms <= now_ms,
            Some(WatchState::Watching | WatchState::Unsupported) => false,
        };
        if !due {
            return None;
        }
        let effect = self.watch_effect(account)?;
        self.state.status.sync.entry(account).or_default().watch = WatchState::Watching;
        Some(effect)
    }

    fn watch_effect(&self, account: AccountId) -> Option<Effect> {
        let login = self.state.accounts.get(&account)?.imap_login();
        let inbox = self
            .state
            .mailboxes
            .values()
            .find(|m| m.account == account && m.role == Some(MailboxRole::Inbox))?;
        Some(Effect::WatchMailbox {
            account,
            mailbox: inbox.id,
            path: inbox.path.clone(),
            imap: login,
        })
    }

    /// Whether `account`'s INBOX is being watched (the poll skips it).
    pub(super) fn is_watched(&self, account: AccountId) -> bool {
        self.state
            .status
            .sync
            .get(&account)
            .is_some_and(|s| s.watch == WatchState::Watching)
    }

    /// Watches that are due again at `now_ms` (after a failure).
    pub(super) fn retry_watches(&mut self, now_ms: i64) -> Vec<Effect> {
        let due: Vec<AccountId> = self
            .state
            .status
            .sync
            .iter()
            .filter(
                |(_, s)| matches!(s.watch, WatchState::Retrying { at_ms, .. } if at_ms <= now_ms),
            )
            .map(|(id, _)| *id)
            .collect();
        due.into_iter()
            .filter_map(|account| self.start_watch(account, now_ms))
            .collect()
    }

    pub(super) fn ingest_watched(
        &mut self,
        account: AccountId,
        mailbox: MailboxId,
        changed: bool,
        receipt: CommandReceipt,
    ) -> Outcome {
        let receipt = receipt.affecting(EntityRef::Account(account));
        if !self.is_watched(account) || !self.state.mailboxes.contains_key(&mailbox) {
            // A removed account or mailbox, or a watch that was replaced: let it end.
            return Outcome::new(receipt);
        }
        let mut outcome = Outcome::new(receipt.affecting(EntityRef::Status));
        // Even mid-round: the round's INBOX fetch may have run before the change. A second
        // incremental fetch costs little.
        if changed && let Some(fetch) = self.start_fetch(mailbox) {
            outcome = outcome.with_effect(fetch);
        }
        // Watch again at once, so that nothing that happens during the fetch is missed.
        if let Some(watch) = self.watch_effect(account) {
            outcome = outcome.with_effect(watch);
        }
        outcome
    }

    pub(super) fn ingest_watch_failed(
        &mut self,
        account: AccountId,
        mailbox: MailboxId,
        error: String,
        unsupported: bool,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let receipt = receipt.affecting(EntityRef::Account(account));
        if !self.is_watched(account) || !self.state.mailboxes.contains_key(&mailbox) {
            return Outcome::new(receipt);
        }
        let mut outcome = Outcome::new(receipt.affecting(EntityRef::Status));
        let watch = if unsupported {
            WatchState::Unsupported
        } else {
            let at_ms = ctx.now_ms.saturating_add(WATCH_RETRY_MS);
            outcome = outcome.with_effect(Effect::ScheduleTick { at_ms });
            WatchState::Retrying { at_ms, error }
        };
        self.state.status.sync.entry(account).or_default().watch = watch;
        outcome
    }

    /// After a successful INBOX fetch: start watching `account` if it isn't yet.
    pub(super) fn watch_after_fetch(
        &mut self,
        account: AccountId,
        role: Option<MailboxRole>,
        now_ms: i64,
    ) -> Option<Effect> {
        if role != Some(MailboxRole::Inbox) {
            return None;
        }
        match self.state.status.sync.get(&account).map(|s| &s.watch) {
            None | Some(WatchState::Off) => self.start_watch(account, now_ms),
            _ => None,
        }
    }
}
