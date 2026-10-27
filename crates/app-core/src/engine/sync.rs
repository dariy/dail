//! Sync events: mailbox listings, fetched envelopes and sync failures (`ARCHITECTURE.md §5.2`).

use std::collections::BTreeMap;

use indexmap::IndexSet;

use crate::ctx::Ctx;
use crate::error::CoreError;
use crate::event::{Effect, MailDelta, PersistDelta};
use crate::ids::{AccountId, ApprovalId, DraftId, IdGen, MailboxId, MessageId, OpId, ThreadId};
use crate::mailbox::{Mailbox, MailboxRole, RemoteMailbox, assign_roles};
use crate::message::{BodyState, Flags, RemoteEnvelope, Thread, UidFlags};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::{AppState, SyncIssue};
use crate::threading::{Placement, ThreadKey, Threader};

use super::{Engine, not_found};

/// The result of an `envelopes_fetched` event, before it becomes a receipt.
#[derive(Debug, Clone)]
pub(super) struct FetchedEnvelopes {
    pub account: AccountId,
    pub mailbox: MailboxId,
    pub uid_validity: u32,
    pub uid_next: Option<u32>,
    pub total: u32,
    pub unread: u32,
    pub envelopes: Vec<RemoteEnvelope>,
}

/// The result of a `flags_changed` event.
#[derive(Debug, Clone)]
pub(super) struct ChangedFlags {
    pub account: AccountId,
    pub mailbox: MailboxId,
    pub uid_validity: u32,
    pub from_uid: u32,
    pub to_uid: u32,
    pub flags: Vec<UidFlags>,
    pub existing: Option<Vec<u32>>,
    pub highest_modseq: Option<u64>,
}

/// How many unread INBOX bodies are prefetched (`ARCHITECTURE.md §5.2`).
const PREFETCH_BODIES: usize = 20;

/// Collects cache changes while an input is applied, then turns them into a [`MailDelta`].
#[derive(Debug, Default)]
pub(super) struct CacheChanges {
    // Sets that keep insertion order: a 100k-message fetch must not search a list per message.
    pub mailboxes: IndexSet<MailboxId>,
    pub removed_mailboxes: IndexSet<MailboxId>,
    pub messages: IndexSet<MessageId>,
    pub removed_messages: IndexSet<MessageId>,
    pub threads: IndexSet<ThreadId>,
    pub ops: IndexSet<OpId>,
    pub removed_ops: IndexSet<OpId>,
    pub drafts: IndexSet<DraftId>,
    pub removed_drafts: IndexSet<DraftId>,
    pub approvals: IndexSet<ApprovalId>,
    pub removed_approvals: IndexSet<ApprovalId>,
}

/// Where [`Engine::merge_remote`] puts fetched envelopes, and the lookups it keeps up to date.
pub(super) struct MergeTarget {
    pub account: AccountId,
    pub mailbox: MailboxId,
    /// Cached messages of `mailbox` by UID (placeholders excluded).
    pub by_uid: BTreeMap<u32, MessageId>,
    /// The placeholders in `mailbox` (moved or copied here by ops, UID not known yet): few, so
    /// fetched envelopes are matched against them instead of against every cached message.
    pub placeholders: Vec<MessageId>,
    pub threader: Threader,
    pub ids: IdGen,
}

/// The placeholders cached in `mailbox`.
pub(super) fn placeholders_in(state: &AppState, mailbox: MailboxId) -> Vec<MessageId> {
    state
        .messages
        .values()
        .filter(|m| m.mailbox == mailbox && m.is_placeholder())
        .map(|m| m.id)
        .collect()
}

/// What [`Engine::merge_remote`] did with one envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Merged {
    /// The UID was cached; `updated` if its flags changed.
    Known { updated: bool },
    /// A placeholder took the UID.
    Located,
    /// A new message.
    Added,
}

pub(super) fn push_unique<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

impl Engine {
    /// Reconciles the cached mailboxes of `account` with a server listing: new mailboxes get
    /// IDs, known ones keep theirs (and their sync position), vanished ones are dropped together
    /// with their messages. Then every mailbox is fetched.
    pub(super) fn ingest_mailboxes_listed(
        &mut self,
        account: AccountId,
        listed: Vec<RemoteMailbox>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(preset) = self.state.accounts.get(&account).map(|a| a.preset) else {
            // The account was removed while the listing was in flight.
            return Outcome::new(receipt);
        };
        let roles = assign_roles(preset, &listed);
        let mut ids = ctx.ids();
        let mut changes = CacheChanges::default();
        let mut kept: Vec<MailboxId> = Vec::new();
        let mut receipt = receipt.affecting(EntityRef::Account(account));

        for (remote, role) in listed.into_iter().zip(roles) {
            if !remote.selectable {
                continue;
            }
            let existing = self
                .state
                .mailboxes
                .values()
                .find(|m| m.account == account && m.path == remote.path)
                .cloned();
            if let Some(known) = &existing
                && kept.contains(&known.id)
            {
                // The server listed the same path twice.
                continue;
            }
            let mailbox = match existing.clone() {
                Some(known) => Mailbox {
                    delimiter: remote.delimiter,
                    role,
                    ..known
                },
                None => Mailbox {
                    id: self.fresh_mailbox_id(&mut ids),
                    account,
                    path: remote.path,
                    delimiter: remote.delimiter,
                    role,
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
            };
            kept.push(mailbox.id);
            if existing.as_ref() != Some(&mailbox) {
                receipt = receipt.affecting(EntityRef::Mailbox(mailbox.id));
                changes.mailboxes.insert(mailbox.id);
                self.state.mailboxes.insert(mailbox.id, mailbox);
            }
        }

        // Targets of pending moves stay even if the server does not list them (yet): a new
        // archive folder is created by the op that moves into it.
        let targets: Vec<MailboxId> = self
            .state
            .outbox
            .values()
            .filter_map(|op| op.op.target())
            .chain(self.state.messages.values().filter_map(|m| m.moved_to))
            .collect();
        let vanished: Vec<MailboxId> = self
            .state
            .mailboxes
            .values()
            .filter(|m| m.account == account && !kept.contains(&m.id))
            .filter(|m| !targets.contains(&m.id))
            .map(|m| m.id)
            .collect();
        for id in vanished {
            self.drop_mailbox(id, &mut changes);
            receipt = receipt.affecting(EntityRef::Mailbox(id));
        }

        if let Some(status) = self.state.status.sync.get_mut(&account) {
            status.failures = 0;
            if status.last_error.is_some() {
                status.last_error = None;
                receipt = receipt.affecting(EntityRef::Status);
            }
        }

        self.retry_now(account, &mut changes);
        let op_effects = self.pump_ops(account, ctx.now_ms, &mut changes);
        let delta = self.delta(changes);
        let receipt = receipt.with_data(serde_json::json!({
            "account": account,
            "mailboxes": kept.len(),
            "added_or_changed": delta.mailboxes.len(),
            "removed": delta.removed_mailboxes.len(),
        }));
        let mut outcome = Outcome::new(receipt);
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        for id in kept {
            if let Some(fetch) = self.start_fetch(id) {
                outcome = outcome.with_effect(fetch);
            }
        }
        outcome.effects.extend(op_effects);
        // After the fetches are counted, so the round does not look finished in between.
        self.end_sync_effect(account, ctx.now_ms);
        outcome
    }

    /// `sync_now`: lists the mailboxes of `account` (or every account) and starts polling.
    /// Accounts that are still syncing are skipped.
    pub(super) fn sync_now(
        &mut self,
        account: Option<AccountId>,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let targets: Vec<AccountId> = match account {
            Some(id) if self.state.accounts.contains_key(&id) => vec![id],
            Some(id) => {
                return Err(not_found("account", id));
            }
            None => self.state.accounts.keys().copied().collect(),
        };
        let mut receipt = receipt.affecting(EntityRef::Status);
        let mut effects = Vec::new();
        let mut started = Vec::new();
        let mut already = Vec::new();
        for id in targets {
            receipt = receipt.affecting(EntityRef::Account(id));
            if self
                .state
                .status
                .sync
                .get(&id)
                .is_some_and(|s| s.is_syncing())
            {
                already.push(id);
            } else if let Some(listing) = self.start_listing(id) {
                effects.push(listing);
                started.push(id);
                self.state.status.sync.entry(id).or_default().last_full_ms = Some(ctx.now_ms);
            }
        }
        if let Some(tick) = self.start_polling(ctx.now_ms) {
            effects.push(tick);
        }
        let receipt = receipt.with_data(serde_json::json!({
            "started": started,
            "already_syncing": already,
            "next_poll_ms": self.state.status.next_poll_ms,
        }));
        Ok(Outcome { receipt, effects })
    }

    /// A timer fired: start pending ops whose next attempt is due, and poll.
    pub(super) fn ingest_tick(&mut self, now_ms: i64, receipt: CommandReceipt) -> Outcome {
        let mut changes = CacheChanges::default();
        let op_effects = self.pump_all_ops(now_ms, &mut changes);
        let delta = self.delta(changes);
        let mut outcome = self.poll(now_ms, receipt);
        if !delta.is_empty() {
            for op in &delta.ops {
                outcome.receipt = outcome.receipt.affecting(EntityRef::Op(op.id));
            }
            outcome.effects.push(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(op_effects);
        outcome.effects.extend(self.pump_sends(now_ms));
        outcome.effects.extend(self.pump_draft_saves(now_ms, false));
        outcome.effects.extend(self.retry_watches(now_ms));
        outcome
    }

    /// If an INBOX poll is due, fetch the INBOX of every idle account (or list its mailboxes
    /// if no INBOX is known yet) and schedule the next poll.
    fn poll(&mut self, now_ms: i64, receipt: CommandReceipt) -> Outcome {
        let Some(due) = self.state.status.next_poll_ms else {
            return Outcome::new(receipt);
        };
        if now_ms < due {
            return Outcome::new(receipt);
        }
        let mut receipt = receipt.affecting(EntityRef::Status);
        if self.state.accounts.is_empty() {
            self.state.status.next_poll_ms = None;
            return Outcome::new(receipt);
        }
        let mut effects = Vec::new();
        let accounts: Vec<AccountId> = self.state.accounts.keys().copied().collect();
        let poll_ms = i64::try_from(self.state.settings.poll_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        let full_ms = i64::try_from(self.state.settings.full_sync_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        for account in accounts {
            let status = self
                .state
                .status
                .sync
                .get(&account)
                .cloned()
                .unwrap_or_default();
            if status.is_syncing() || backing_off(&status, poll_ms, now_ms) {
                continue;
            }
            // A full round now and then (UX-6): new folders, Sent, Archive stay fresh.
            if full_ms > 0
                && status
                    .last_full_ms
                    .is_none_or(|at| now_ms.saturating_sub(at) >= full_ms)
                && let Some(listing) = self.start_listing(account)
            {
                self.state
                    .status
                    .sync
                    .entry(account)
                    .or_default()
                    .last_full_ms = Some(now_ms);
                receipt = receipt.affecting(EntityRef::Account(account));
                effects.push(listing);
                continue;
            }
            let inbox = self
                .state
                .mailboxes
                .values()
                .find(|m| m.account == account && m.role == Some(MailboxRole::Inbox))
                .map(|m| m.id);
            let mut fetch = Vec::new();
            if !self.is_watched(account) {
                match inbox {
                    Some(inbox) => fetch.extend(self.start_fetch(inbox)),
                    None => fetch.extend(self.start_listing(account)),
                }
            }
            // The open mailbox is polled too.
            let open = self
                .state
                .view
                .mailbox
                .filter(|id| Some(*id) != inbox)
                .filter(|id| {
                    self.state
                        .mailboxes
                        .get(id)
                        .is_some_and(|m| m.account == account)
                });
            if let Some(open) = open {
                fetch.extend(self.refresh(open));
            }
            if !fetch.is_empty() {
                receipt = receipt.affecting(EntityRef::Account(account));
                effects.extend(fetch);
            }
        }
        self.state.status.next_poll_ms = None;
        if let Some(tick) = self.start_polling(now_ms) {
            effects.push(tick);
        }
        Outcome { receipt, effects }
    }

    /// Schedules the next poll unless one is scheduled already (or there are no accounts).
    fn start_polling(&mut self, now_ms: i64) -> Option<Effect> {
        if self.state.status.next_poll_ms.is_some() || self.state.accounts.is_empty() {
            return None;
        }
        let interval = i64::try_from(self.state.settings.poll_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        let at_ms = now_ms.saturating_add(interval);
        self.state.status.next_poll_ms = Some(at_ms);
        Some(Effect::ScheduleTick { at_ms })
    }

    /// The `ListMailboxes` effect for `account`, counted as in flight.
    pub(super) fn start_listing(&mut self, account: AccountId) -> Option<Effect> {
        let imap = self.state.accounts.get(&account)?.imap_login();
        self.state.status.sync.entry(account).or_default().pending += 1;
        Some(Effect::ListMailboxes { account, imap })
    }

    /// The `FetchEnvelopes` effect for `mailbox`, counted as in flight.
    pub(super) fn start_fetch(&mut self, mailbox: MailboxId) -> Option<Effect> {
        let effect = self.fetch_envelopes_effect(mailbox)?;
        self.state.status.fetching.insert(mailbox);
        if let Effect::FetchEnvelopes { account, .. } = &effect {
            self.state.status.sync.entry(*account).or_default().pending += 1;
        }
        Some(effect)
    }

    /// `start_fetch` unless a fetch of `mailbox` is in flight already (UX-6's refreshes).
    pub(super) fn refresh(&mut self, mailbox: MailboxId) -> Option<Effect> {
        if self.state.status.fetching.contains(&mailbox) {
            return None;
        }
        self.start_fetch(mailbox)
    }

    /// One sync effect of `account` completed (or failed). When none are left, the round is
    /// done.
    fn end_sync_effect(&mut self, account: AccountId, now_ms: i64) {
        if let Some(status) = self.state.status.sync.get_mut(&account)
            && status.pending > 0
        {
            status.pending -= 1;
            if status.pending == 0 {
                status.last_sync_ms = Some(now_ms);
            }
        }
    }

    /// The effect that fetches what is new in `mailbox`.
    pub(super) fn fetch_envelopes_effect(&self, mailbox: MailboxId) -> Option<Effect> {
        let mailbox = self.state.mailboxes.get(&mailbox)?;
        let account = self.state.accounts.get(&mailbox.account)?;
        let limit = u32::try_from(self.state.settings.initial_window).unwrap_or(u32::MAX);
        Some(Effect::FetchEnvelopes {
            account: account.id,
            mailbox: mailbox.id,
            path: mailbox.path.clone(),
            imap: account.imap_login(),
            uid_validity: mailbox.uid_validity,
            since_uid: mailbox.uid_validity.and(mailbox.uid_next),
            limit,
        })
    }

    /// Adds fetched envelopes to the cache and threads them. A changed UIDVALIDITY first drops
    /// every cached message of the mailbox. Known UIDs only get their flags updated.
    pub(super) fn ingest_envelopes_fetched(
        &mut self,
        fetched: FetchedEnvelopes,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.end_sync_effect(fetched.account, ctx.now_ms);
        self.state.status.fetching.remove(&fetched.mailbox);
        if let Some(status) = self.state.status.sync.get_mut(&fetched.account) {
            status.failures = 0;
        }
        let Some(mut mailbox) = self.state.mailboxes.get(&fetched.mailbox).cloned() else {
            return Outcome::new(receipt);
        };
        if mailbox.account != fetched.account {
            return Outcome::new(receipt);
        }
        let account = fetched.account;
        let mut changes = CacheChanges::default();
        let mut receipt = receipt.affecting(EntityRef::Mailbox(mailbox.id));

        let reset = mailbox
            .uid_validity
            .is_some_and(|cached| cached != fetched.uid_validity);
        let in_mailbox: Vec<(u32, MessageId)> = self
            .state
            .messages
            .values()
            .filter(|m| m.mailbox == mailbox.id && !m.is_placeholder())
            .map(|m| (m.uid, m.id))
            .collect();
        // The cached UID range to resync after this fetch (not after a reset or a first fetch).
        let resync_range = if reset {
            None
        } else {
            let uids = in_mailbox.iter().map(|(uid, _)| *uid);
            uids.clone().min().zip(uids.max())
        };
        let mut by_uid: BTreeMap<u32, MessageId> = BTreeMap::new();
        if reset {
            for (_, id) in &in_mailbox {
                self.remove_message(*id, &mut changes);
            }
        } else {
            by_uid.extend(in_mailbox);
        }

        let mut target = MergeTarget {
            account,
            mailbox: mailbox.id,
            placeholders: placeholders_in(&self.state, mailbox.id),
            by_uid,
            threader: self.take_threader(account),
            ids: ctx.ids(),
        };
        let mut added = 0_usize;
        let mut updated = 0_usize;
        let mut located = 0_usize;
        let mut max_uid: Option<u32> = None;
        for remote in fetched.envelopes {
            max_uid = max_uid.max(Some(remote.uid));
            match self.merge_remote(remote, &mut target, ctx, &mut changes).1 {
                Merged::Known { updated: true } => updated += 1,
                Merged::Known { updated: false } => {}
                Merged::Located => located += 1,
                Merged::Added => added += 1,
            }
        }
        self.keep_threader(account, std::mem::take(&mut target.threader));

        let uid_next = match (fetched.uid_next, max_uid.map(|uid| uid.saturating_add(1))) {
            (Some(server), Some(seen)) => Some(server.max(seen)),
            (server, seen) => server
                .or(seen)
                .or(if reset { None } else { mailbox.uid_next }),
        };
        let before = mailbox.clone();
        if reset {
            // Mod-sequences and history of the old UIDVALIDITY mean nothing now.
            mailbox.highest_modseq = None;
            mailbox.history_complete = false;
        }
        mailbox.uid_validity = Some(fetched.uid_validity);
        mailbox.uid_next = uid_next;
        mailbox.total = fetched.total;
        mailbox.unread = fetched.unread;
        mailbox.last_fetch_ms = Some(ctx.now_ms);
        if mailbox != before {
            self.state.mailboxes.insert(mailbox.id, mailbox.clone());
            changes.mailboxes.insert(mailbox.id);
        }

        if let Some(status) = self.state.status.sync.get_mut(&account)
            && status
                .last_error
                .as_ref()
                .is_some_and(|issue| issue.mailbox == Some(mailbox.id))
        {
            status.last_error = None;
            receipt = receipt.affecting(EntityRef::Status);
        }

        self.retry_now(account, &mut changes);
        let op_effects = self.pump_ops(account, ctx.now_ms, &mut changes);
        let delta = self.delta(changes);
        for thread in &delta.threads {
            receipt = receipt.affecting(EntityRef::Thread(thread.id));
        }
        let receipt = receipt.with_data(serde_json::json!({
            "mailbox": mailbox.id,
            "added": added,
            "located": located,
            "flags_updated": updated,
            "uid_validity_reset": reset,
        }));
        let mut outcome = Outcome::new(receipt);
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        if let Some((from_uid, to_uid)) = resync_range
            && let Some(resync) = self.start_resync(mailbox.id, from_uid, to_uid)
        {
            outcome = outcome.with_effect(resync);
        }
        if mailbox.role == Some(MailboxRole::Inbox) {
            for fetch in self.prefetch_bodies(mailbox.id) {
                outcome = outcome.with_effect(fetch);
            }
        }
        if let Some(watch) = self.watch_after_fetch(account, mailbox.role, ctx.now_ms) {
            outcome = outcome.with_effect(watch);
        }
        if let Some(older) = self.maybe_backfill(mailbox.id) {
            outcome = outcome.with_effect(older);
        }
        outcome.effects.extend(op_effects);
        outcome
    }

    /// Merges one fetched envelope into `target.mailbox`: a known UID gets its flags updated, a
    /// placeholder of a pending move or copy is located (it keeps its ID and local flags), and
    /// anything else becomes a new, threaded message. Returns the message's ID.
    pub(super) fn merge_remote(
        &mut self,
        remote: RemoteEnvelope,
        target: &mut MergeTarget,
        ctx: &Ctx,
        changes: &mut CacheChanges,
    ) -> (MessageId, Merged) {
        if let Some(id) = target.by_uid.get(&remote.uid).copied() {
            let flags = self.with_pending_flags(id, remote.flags);
            if let Some(known) = self.state.messages.get_mut(&id)
                && known.flags != flags
            {
                known.flags = flags;
                changes.messages.insert(id);
                changes.threads.insert(known.thread);
                return (id, Merged::Known { updated: true });
            }
            return (id, Merged::Known { updated: false });
        }
        if let Some(id) = self.placeholder_for(&mut target.placeholders, &remote) {
            // A message moved or copied here by an op: it keeps its ID and local flags.
            if let Some(envelope) = self.state.messages.get_mut(&id) {
                envelope.uid = remote.uid;
            }
            target.by_uid.insert(remote.uid, id);
            changes.messages.insert(id);
            self.push_flag_diff(id, remote.flags, ctx, &mut target.ids, changes);
            return (id, Merged::Located);
        }
        let placement = target.threader.place(&ThreadKey::from(&remote));
        let thread = match placement {
            Placement::Join(thread) => thread,
            Placement::Merge { into, from } => {
                for old in from {
                    self.merge_thread(old, into, changes);
                    target.threader.merge(old, into);
                }
                into
            }
            Placement::New => {
                let id = self.fresh_thread_id(&mut target.ids);
                self.state.threads.insert(
                    id,
                    Thread {
                        id,
                        account: target.account,
                        messages: Vec::new(),
                        subject: remote.subject.clone(),
                        last_date_ms: remote.date_ms,
                    },
                );
                id
            }
        };
        target.threader.add(&ThreadKey::from(&remote), thread);
        let id = self.fresh_message_id(&mut target.ids);
        target.by_uid.insert(remote.uid, id);
        let envelope = remote.into_envelope(id, target.account, target.mailbox, thread);
        self.state.messages.insert(id, envelope);
        if let Some(entry) = self.state.threads.get_mut(&thread) {
            entry.messages.push(id);
        }
        changes.messages.insert(id);
        changes.threads.insert(thread);
        (id, Merged::Added)
    }

    /// The placeholder in `mailbox` that `remote` is the server copy of: same `Message-ID`, or
    /// (without one) same subject, date and size.
    /// The placeholder `remote` locates, taken out of `placeholders`.
    fn placeholder_for(
        &self,
        placeholders: &mut Vec<MessageId>,
        remote: &RemoteEnvelope,
    ) -> Option<MessageId> {
        let at = placeholders
            .iter()
            .filter_map(|id| self.state.messages.get(id))
            .position(|m| match &remote.message_id_hdr {
                Some(header) => m.message_id_hdr.as_ref() == Some(header),
                None => {
                    m.message_id_hdr.is_none()
                        && m.subject == remote.subject
                        && m.date_ms == remote.date_ms
                        && m.size == remote.size
                }
            })?;
        Some(placeholders.remove(at))
    }

    /// Applies a flag resync: updates flags of cached UIDs in the range and drops the ones the
    /// server no longer has (expunged). Ignored if UIDVALIDITY changed meanwhile.
    pub(super) fn ingest_flags_changed(
        &mut self,
        changed: ChangedFlags,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.end_sync_effect(changed.account, ctx.now_ms);
        let Some(mailbox) = self.state.mailboxes.get(&changed.mailbox) else {
            return Outcome::new(receipt);
        };
        if mailbox.account != changed.account || mailbox.uid_validity != Some(changed.uid_validity)
        {
            return Outcome::new(receipt);
        }
        let mailbox_id = mailbox.id;
        let current: BTreeMap<u32, Flags> = changed
            .flags
            .iter()
            .map(|entry| (entry.uid, entry.flags))
            .collect();
        let cached: Vec<(MessageId, u32)> = self
            .state
            .messages
            .values()
            .filter(|m| {
                m.mailbox == mailbox_id
                    && !m.is_placeholder()
                    && m.uid >= changed.from_uid
                    && m.uid <= changed.to_uid
            })
            .map(|m| (m.id, m.uid))
            .collect();
        let existing: Option<std::collections::BTreeSet<u32>> = changed
            .existing
            .as_ref()
            .map(|uids| uids.iter().copied().collect());
        let mut changes = CacheChanges::default();
        let mut updated = 0_usize;
        let mut expunged = 0_usize;
        if mailbox.highest_modseq != changed.highest_modseq {
            let mut mailbox = mailbox.clone();
            mailbox.highest_modseq = changed.highest_modseq;
            self.state.mailboxes.insert(mailbox_id, mailbox);
            changes.mailboxes.insert(mailbox_id);
        }
        for (id, uid) in cached {
            match current.get(&uid) {
                // CONDSTORE: unchanged messages are not listed, but they exist.
                None if existing.as_ref().is_some_and(|e| e.contains(&uid)) => {}
                Some(flags) => {
                    let flags = self.with_pending_flags(id, *flags);
                    if let Some(envelope) = self.state.messages.get_mut(&id)
                        && envelope.flags != flags
                    {
                        envelope.flags = flags;
                        changes.messages.insert(id);
                        changes.threads.insert(envelope.thread);
                        updated += 1;
                    }
                }
                None => {
                    self.remove_message(id, &mut changes);
                    expunged += 1;
                }
            }
        }
        let delta = self.delta(changes);
        let mut receipt = receipt.affecting(EntityRef::Mailbox(mailbox_id));
        for thread in &delta.threads {
            receipt = receipt.affecting(EntityRef::Thread(thread.id));
        }
        let receipt = receipt.with_data(serde_json::json!({
            "mailbox": mailbox_id,
            "flags_updated": updated,
            "expunged": expunged,
        }));
        let mut outcome = Outcome::new(receipt);
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome
    }

    /// Records a downloaded (or failed) body.
    pub(super) fn ingest_body(
        &mut self,
        message: MessageId,
        body: BodyState,
        parsed: Option<(bool, Option<String>)>,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.status.bodies_in_flight.remove(&message);
        let Some(envelope) = self.state.messages.get_mut(&message) else {
            return Outcome::new(receipt.affecting(EntityRef::Status));
        };
        envelope.body = body;
        if let Some((has_attachments, snippet)) = parsed {
            envelope.has_attachments = has_attachments;
            envelope.snippet = snippet.as_deref().and_then(crate::message::snippet);
        }
        let thread = envelope.thread;
        let mut changes = CacheChanges::default();
        changes.messages.insert(message);
        changes.threads.insert(thread);
        let delta = self.delta(changes);
        Outcome::new(
            receipt
                .affecting(EntityRef::Message(message))
                .affecting(EntityRef::Status),
        )
        .with_effect(Effect::PersistState {
            delta: PersistDelta::Mail(delta),
        })
    }

    /// `FetchBody` effects for the newest unread INBOX messages without a body, up to
    /// [`PREFETCH_BODIES`] (counting bodies already in flight).
    fn prefetch_bodies(&mut self, mailbox: MailboxId) -> Vec<Effect> {
        let mut candidates: Vec<(i64, MessageId)> = self
            .state
            .messages
            .values()
            .filter(|m| m.mailbox == mailbox && !m.flags.seen && !m.is_placeholder())
            .map(|m| (m.date_ms, m.id))
            .collect();
        candidates.sort_by(|a, b| b.cmp(a));
        let mut effects = Vec::new();
        for (_, id) in candidates.into_iter().take(PREFETCH_BODIES) {
            let wanted = self
                .state
                .messages
                .get(&id)
                .is_some_and(|m| m.body == BodyState::NotFetched);
            if wanted && let Some(fetch) = self.start_body_fetch(id) {
                effects.push(fetch);
            }
        }
        effects
    }

    /// The `FetchBody` effect for `message`, unless it is already in flight.
    pub(super) fn start_body_fetch(&mut self, message: MessageId) -> Option<Effect> {
        if self.state.status.bodies_in_flight.contains(&message) {
            return None;
        }
        let effect = self.body_fetch_effect(message)?;
        self.state.status.bodies_in_flight.insert(message);
        Some(effect)
    }

    /// The `FetchBody` effect for `message` (not recorded as in flight).
    pub(super) fn body_fetch_effect(&self, message: MessageId) -> Option<Effect> {
        let envelope = self.state.messages.get(&message)?;
        if envelope.is_placeholder() {
            return None;
        }
        let mailbox = self.state.mailboxes.get(&envelope.mailbox)?;
        let account = self.state.accounts.get(&envelope.account)?;
        Some(Effect::FetchBody {
            account: account.id,
            message,
            path: mailbox.path.clone(),
            imap: account.imap_login(),
            uid_validity: mailbox.uid_validity?,
            uid: envelope.uid,
        })
    }

    /// The `ResyncFlags` effect for a UID range of `mailbox`, counted as in flight.
    fn start_resync(&mut self, mailbox: MailboxId, from_uid: u32, to_uid: u32) -> Option<Effect> {
        let mailbox = self.state.mailboxes.get(&mailbox)?;
        let account = self.state.accounts.get(&mailbox.account)?;
        let effect = Effect::ResyncFlags {
            account: account.id,
            mailbox: mailbox.id,
            path: mailbox.path.clone(),
            imap: account.imap_login(),
            uid_validity: mailbox.uid_validity?,
            from_uid,
            to_uid,
            changed_since: mailbox.highest_modseq,
        };
        self.state
            .status
            .sync
            .entry(account.id)
            .or_default()
            .pending += 1;
        Some(effect)
    }

    /// Records a failed sync step in the account's sync status.
    pub(super) fn ingest_sync_failed(
        &mut self,
        account: AccountId,
        mailbox: Option<MailboxId>,
        error: String,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        if !self.state.accounts.contains_key(&account) {
            return Outcome::new(receipt);
        }
        self.end_sync_effect(account, ctx.now_ms);
        if let Some(mailbox) = mailbox {
            self.state.status.fetching.remove(&mailbox);
        }
        let status = self.state.status.sync.entry(account).or_default();
        status.failures = status.failures.saturating_add(1);
        status.last_error = Some(SyncIssue {
            error,
            mailbox,
            at_ms: ctx.now_ms,
        });
        let mut changes = CacheChanges::default();
        let effects = self.pump_ops(account, ctx.now_ms, &mut changes);
        let delta = self.delta(changes);
        let mut outcome = Outcome::new(
            receipt
                .affecting(EntityRef::Account(account))
                .affecting(EntityRef::Status),
        );
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(effects);
        outcome
    }

    /// Drops everything cached for `account` (on account removal). Returns the store delta.
    pub(super) fn forget_account_cache(&mut self, account: AccountId) -> MailDelta {
        let mut changes = CacheChanges::default();
        let mailboxes: Vec<MailboxId> = self
            .state
            .mailboxes
            .values()
            .filter(|m| m.account == account)
            .map(|m| m.id)
            .collect();
        for id in mailboxes {
            self.drop_mailbox(id, &mut changes);
        }
        let ops: Vec<OpId> = self
            .state
            .outbox
            .values()
            .filter(|op| op.account == account)
            .map(|op| op.id)
            .collect();
        for id in ops {
            self.state.outbox.shift_remove(&id);
            changes.removed_ops.insert(id);
        }
        let drafts: Vec<DraftId> = self
            .state
            .drafts
            .values()
            .filter(|d| d.account == account)
            .map(|d| d.id)
            .collect();
        for id in drafts {
            self.state.drafts.shift_remove(&id);
            changes.removed_drafts.insert(id);
        }
        let approvals: Vec<ApprovalId> = self
            .state
            .approvals
            .values()
            .filter(|a| a.account == account)
            .map(|a| a.id)
            .collect();
        for id in approvals {
            self.state.approvals.shift_remove(&id);
            changes.removed_approvals.insert(id);
        }
        self.state.status.sync.shift_remove(&account);
        self.delta(changes)
    }

    /// Removes a mailbox and its messages.
    pub(super) fn drop_mailbox(&mut self, id: MailboxId, changes: &mut CacheChanges) {
        let messages: Vec<MessageId> = self
            .state
            .messages
            .values()
            .filter(|m| m.mailbox == id)
            .map(|m| m.id)
            .collect();
        for message in messages {
            self.remove_message(message, changes);
        }
        for envelope in self.state.messages.values_mut() {
            if envelope.moved_to == Some(id) {
                envelope.moved_to = None;
                changes.messages.insert(envelope.id);
            }
        }
        self.state.mailboxes.shift_remove(&id);
        changes.removed_mailboxes.insert(id);
    }

    /// Removes one cached envelope and takes it out of its thread.
    pub(super) fn remove_message(&mut self, id: MessageId, changes: &mut CacheChanges) {
        self.messages_changed();
        let Some(envelope) = self.state.messages.shift_remove(&id) else {
            return;
        };
        self.state.status.bodies_in_flight.remove(&id);
        if let Some(thread) = self.state.threads.get_mut(&envelope.thread) {
            thread.messages.retain(|m| *m != id);
        }
        changes.threads.insert(envelope.thread);
        changes.removed_messages.insert(id);
    }

    /// Moves every message of thread `from` into thread `into` and deletes `from`.
    fn merge_thread(&mut self, from: ThreadId, into: ThreadId, changes: &mut CacheChanges) {
        let Some(old) = self.state.threads.get(&from).cloned() else {
            return;
        };
        for id in &old.messages {
            if let Some(envelope) = self.state.messages.get_mut(id) {
                envelope.thread = into;
                changes.messages.insert(*id);
            }
        }
        if let Some(target) = self.state.threads.get_mut(&into) {
            target.messages.extend(old.messages);
        }
        // Emptied: `delta` removes it.
        if let Some(entry) = self.state.threads.get_mut(&from) {
            entry.messages.clear();
        }
        changes.threads.insert(from);
        changes.threads.insert(into);
    }

    /// Recomputes derived thread fields, deletes emptied threads and builds the store delta.
    pub(super) fn delta(&mut self, changes: CacheChanges) -> MailDelta {
        let approvals = changes
            .approvals
            .iter()
            .filter(|id| !changes.removed_approvals.contains(*id))
            .filter_map(|id| self.state.approvals.get(id).cloned())
            .collect();
        let drafts = changes
            .drafts
            .iter()
            .filter(|id| !changes.removed_drafts.contains(*id))
            .filter_map(|id| self.state.drafts.get(id).cloned())
            .collect();
        let ops = changes
            .ops
            .iter()
            .filter(|id| !changes.removed_ops.contains(*id))
            .filter_map(|id| self.state.outbox.get(id).cloned())
            .collect();
        let mut delta = MailDelta {
            approvals,
            drafts,
            ops,
            removed_mailboxes: changes.removed_mailboxes.into_iter().collect(),
            removed_messages: changes.removed_messages.into_iter().collect(),
            removed_ops: changes.removed_ops.into_iter().collect(),
            removed_drafts: changes.removed_drafts.into_iter().collect(),
            removed_approvals: changes.removed_approvals.into_iter().collect(),
            ..MailDelta::default()
        };
        for id in changes.threads {
            if self.refresh_thread(id) {
                if let Some(thread) = self.state.threads.get(&id) {
                    delta.threads.push(thread.clone());
                }
            } else {
                delta.removed_threads.push(id);
            }
        }
        delta.mailboxes = changes
            .mailboxes
            .iter()
            .filter_map(|id| self.state.mailboxes.get(id).cloned())
            .collect();
        delta.messages = changes
            .messages
            .iter()
            .filter_map(|id| self.state.messages.get(id).cloned())
            .collect();
        let AppState {
            view,
            mailboxes,
            threads,
            ..
        } = &mut self.state;
        view.prune(
            |id| mailboxes.contains_key(id),
            |id| threads.contains_key(id),
        );
        delta
    }

    /// Sorts a thread's messages by date, refreshes its subject and date, and deletes it if it
    /// is empty. Returns whether the thread still exists.
    fn refresh_thread(&mut self, id: ThreadId) -> bool {
        let Some(thread) = self.state.threads.get(&id) else {
            return false;
        };
        let mut members: Vec<(i64, MessageId)> = thread
            .messages
            .iter()
            .filter_map(|m| self.state.messages.get(m).map(|e| (e.date_ms, e.id)))
            .collect();
        if members.is_empty() {
            self.state.threads.shift_remove(&id);
            return false;
        }
        members.sort();
        members.dedup();
        let subject = members
            .first()
            .and_then(|(_, m)| self.state.messages.get(m))
            .map(|e| e.subject.clone())
            .unwrap_or_default();
        let last_date_ms = members.iter().map(|(date, _)| *date).max().unwrap_or(0);
        if let Some(thread) = self.state.threads.get_mut(&id) {
            thread.messages = members.into_iter().map(|(_, m)| m).collect();
            thread.subject = subject;
            thread.last_date_ms = last_date_ms;
        }
        true
    }

    pub(super) fn fresh_mailbox_id(&self, ids: &mut IdGen) -> MailboxId {
        loop {
            let id: MailboxId = ids.next_id();
            if !self.state.mailboxes.contains_key(&id) {
                return id;
            }
        }
    }

    pub(super) fn fresh_message_id(&self, ids: &mut IdGen) -> MessageId {
        loop {
            let id: MessageId = ids.next_id();
            if !self.state.messages.contains_key(&id) {
                return id;
            }
        }
    }

    fn fresh_thread_id(&self, ids: &mut IdGen) -> ThreadId {
        loop {
            let id: ThreadId = ids.next_id();
            if !self.state.threads.contains_key(&id) {
                return id;
            }
        }
    }
}

/// Whether a poll of the account waits after failures: `min(poll × 2^(failures − 1), 15 min)`
/// after the last one, so the regular poll after one failure still runs and the wait doubles
/// from there (UX-6). Any success resets the count.
fn backing_off(status: &crate::state::SyncStatus, poll_ms: i64, now_ms: i64) -> bool {
    const MAX_BACKOFF_MS: i64 = 15 * 60 * 1000;
    // After one failure the regular poll runs as usual.
    if status.failures < 2 {
        return false;
    }
    let Some(issue) = &status.last_error else {
        return false;
    };
    let factor = 1_i64 << (status.failures - 1).min(16);
    let wait = poll_ms.saturating_mul(factor).min(MAX_BACKOFF_MS);
    now_ms < issue.at_ms.saturating_add(wait)
}
