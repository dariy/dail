//! The pending-op queue (`ARCHITECTURE.md §5.2`): one running op per account, oldest first.
//!
//! Ops are resolved into [`RemoteStep`]s only when they start, so they always use the messages'
//! current server locations.

use indexmap::IndexMap;

use crate::ctx::Ctx;
use crate::event::Effect;
use crate::ids::{AccountId, IdGen, MailboxId, MessageId, OpId};
use crate::message::Flags;
use crate::ops::{
    FlagKind, MAX_ATTEMPTS, OpState, PendingOp, RemoteOp, RemoteStep, retry_delay_ms,
};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

use super::Engine;
use super::sync::{CacheChanges, push_unique};

/// The server actions of an op, and what could not be located yet.
#[derive(Debug, Default)]
struct Resolved {
    steps: Vec<RemoteStep>,
    /// Placeholders (UID unknown) the op needs, and the mailboxes to fetch to locate them.
    unknown: Vec<MessageId>,
    unknown_in: Vec<MailboxId>,
}

/// Messages grouped by server mailbox, with their UIDs ascending.
type ByMailbox = IndexMap<MailboxId, Vec<u32>>;

impl Engine {
    /// Queues a new op for `account`. The caller starts it with [`Engine::pump_ops`].
    pub(super) fn queue_op(
        &mut self,
        account: AccountId,
        op: RemoteOp,
        ctx: &Ctx,
        ids: &mut IdGen,
        changes: &mut CacheChanges,
    ) -> OpId {
        let id = loop {
            let id: OpId = ids.next_id();
            if !self.state.outbox.contains_key(&id) {
                break id;
            }
        };
        self.state.outbox.insert(
            id,
            PendingOp {
                id,
                account,
                op,
                state: OpState::Queued,
                attempts: 0,
                last_error: None,
                created_ms: ctx.now_ms,
                next_attempt_ms: None,
                awaiting_sync: false,
            },
        );
        changes.ops.insert(id);
        id
    }

    /// Starts the oldest queued op of `account` unless one is running. An op with nothing left
    /// to do on the server (e.g. its messages are gone) completes at once. An op that needs
    /// placeholders located first fetches their mailboxes and waits (blocking the queue); if a
    /// finished sync could not locate them, they are dropped from the op and the cache. An op
    /// waiting for its next attempt holds up the queue until `now_ms` reaches it.
    pub(super) fn pump_ops(
        &mut self,
        account: AccountId,
        now_ms: i64,
        changes: &mut CacheChanges,
    ) -> Vec<Effect> {
        let Some(imap) = self.state.accounts.get(&account).map(|a| a.imap_login()) else {
            return Vec::new();
        };
        loop {
            let mut ops = self
                .state
                .outbox
                .values()
                .filter(|op| op.account == account);
            if ops.clone().any(|op| op.state == OpState::Running) {
                return Vec::new();
            }
            let Some(next) = ops.find(|op| op.state == OpState::Queued) else {
                return Vec::new();
            };
            if next.next_attempt_ms.is_some_and(|at| at > now_ms) {
                return Vec::new();
            }
            let (id, awaiting) = (next.id, next.awaiting_sync);
            let mut resolved = self.resolve(&next.op);
            if !resolved.unknown.is_empty() {
                if !awaiting {
                    if let Some(op) = self.state.outbox.get_mut(&id) {
                        op.awaiting_sync = true;
                    }
                    changes.ops.insert(id);
                    let fetches: Vec<Effect> = resolved
                        .unknown_in
                        .iter()
                        .filter_map(|mailbox| self.start_fetch(*mailbox))
                        .collect();
                    if !fetches.is_empty() {
                        return fetches;
                    }
                } else if self
                    .state
                    .status
                    .sync
                    .get(&account)
                    .is_some_and(|s| s.is_syncing())
                {
                    return Vec::new();
                }
                // The fetch is done and they are still unknown: their server copies are gone.
                for message in std::mem::take(&mut resolved.unknown) {
                    self.forget_from_op(id, message);
                    self.remove_message(message, changes);
                }
                resolved = self
                    .state
                    .outbox
                    .get(&id)
                    .map(|op| self.resolve(&op.op))
                    .unwrap_or_default();
            }
            if resolved.steps.is_empty() {
                self.state.outbox.shift_remove(&id);
                changes.removed_ops.insert(id);
                continue;
            }
            if let Some(op) = self.state.outbox.get_mut(&id) {
                op.state = OpState::Running;
                op.awaiting_sync = false;
                op.next_attempt_ms = None;
            }
            changes.ops.insert(id);
            return vec![Effect::RunRemoteOp {
                op: id,
                account,
                imap,
                steps: resolved.steps,
            }];
        }
    }

    /// Starts the next op of every account with queued ops (e.g. on a timer tick).
    pub(super) fn pump_all_ops(&mut self, now_ms: i64, changes: &mut CacheChanges) -> Vec<Effect> {
        let mut accounts: Vec<AccountId> = Vec::new();
        for op in self.state.outbox.values() {
            push_unique(&mut accounts, op.account);
        }
        accounts
            .into_iter()
            .flat_map(|account| self.pump_ops(account, now_ms, changes))
            .collect()
    }

    /// The server is reachable again (a sync step of `account` succeeded): ops waiting for
    /// their next attempt may run now.
    pub(super) fn retry_now(&mut self, account: AccountId, changes: &mut CacheChanges) {
        for op in self.state.outbox.values_mut() {
            if op.account == account && op.state == OpState::Queued && op.next_attempt_ms.is_some()
            {
                op.next_attempt_ms = None;
                changes.ops.insert(op.id);
            }
        }
    }

    /// The server actions of `op`, from the messages' current locations. Messages that are gone
    /// (or already in the target) are skipped; placeholders are reported as unknown.
    fn resolve(&self, op: &RemoteOp) -> Resolved {
        let mut resolved = Resolved::default();
        match op {
            RemoteOp::SetFlag {
                flag,
                value,
                messages,
            } => {
                // A placeholder's flags are pushed when a fetch locates it.
                let located: Vec<MessageId> = messages
                    .iter()
                    .copied()
                    .filter(|id| {
                        self.state
                            .messages
                            .get(id)
                            .is_some_and(|m| !m.is_placeholder())
                    })
                    .collect();
                for (mailbox, uids) in self.uids_by_mailbox(&located) {
                    if let Some(mailbox) = self.state.mailboxes.get(&mailbox)
                        && let Some(uid_validity) = mailbox.uid_validity
                        // Keywords stay local where the server keeps none.
                        && (!flag.is_keyword() || mailbox.custom_keywords != Some(false))
                    {
                        resolved.steps.push(RemoteStep::Store {
                            path: mailbox.path.clone(),
                            uid_validity,
                            uids,
                            flag: *flag,
                            value: *value,
                        });
                    }
                }
            }
            RemoteOp::Move { to, messages } => {
                let sources = self.sources(messages.iter().copied(), *to, &mut resolved);
                self.transfer_steps(sources, *to, false, &mut resolved);
            }
            RemoteOp::Copy { to, copies } => {
                let live = copies
                    .iter()
                    .filter(|c| self.state.messages.contains_key(&c.copy))
                    .map(|c| c.source);
                let sources = self.sources(live, *to, &mut resolved);
                self.transfer_steps(sources, *to, true, &mut resolved);
            }
            RemoteOp::FlagMailbox {
                mailbox,
                flag,
                value,
            } => {
                if let Some(mailbox) = self.state.mailboxes.get(mailbox)
                    && let Some(uid_validity) = mailbox.uid_validity
                {
                    resolved.steps.push(RemoteStep::StoreAll {
                        path: mailbox.path.clone(),
                        uid_validity,
                        flag: *flag,
                        value: *value,
                    });
                }
            }
        }
        resolved
    }

    /// Server locations of `messages` that are not in `to` yet.
    fn sources(
        &self,
        messages: impl Iterator<Item = MessageId>,
        to: MailboxId,
        resolved: &mut Resolved,
    ) -> ByMailbox {
        let mut located = Vec::new();
        for id in messages {
            let Some(envelope) = self.state.messages.get(&id) else {
                continue;
            };
            if envelope.mailbox == to {
                continue;
            }
            if envelope.is_placeholder() {
                push_unique(&mut resolved.unknown, id);
                push_unique(&mut resolved.unknown_in, envelope.mailbox);
            } else {
                located.push(id);
            }
        }
        self.uids_by_mailbox(&located)
    }

    fn transfer_steps(&self, sources: ByMailbox, to: MailboxId, copy: bool, out: &mut Resolved) {
        let Some(target) = self.state.mailboxes.get(&to) else {
            return;
        };
        // Never fetched: it may not exist on the server yet (e.g. a new Hostinger Archive).
        let create = target.uid_validity.is_none();
        for (mailbox, uids) in sources {
            let Some(source) = self.state.mailboxes.get(&mailbox) else {
                continue;
            };
            let Some(uid_validity) = source.uid_validity else {
                continue;
            };
            let (path, to) = (source.path.clone(), target.path.clone());
            out.steps.push(if copy {
                RemoteStep::Copy {
                    path,
                    uid_validity,
                    uids,
                    to,
                    create,
                }
            } else {
                RemoteStep::Move {
                    path,
                    uid_validity,
                    uids,
                    to,
                    create,
                }
            });
        }
    }

    /// The UIDs of `messages`, grouped by mailbox (in order of first appearance), ascending.
    fn uids_by_mailbox(&self, messages: &[MessageId]) -> ByMailbox {
        let mut groups: ByMailbox = IndexMap::new();
        for envelope in messages.iter().filter_map(|id| self.state.messages.get(id)) {
            groups
                .entry(envelope.mailbox)
                .or_default()
                .push(envelope.uid);
        }
        for uids in groups.values_mut() {
            uids.sort_unstable();
            uids.dedup();
        }
        groups
    }

    /// Takes `message` out of op `id`.
    fn forget_from_op(&mut self, id: OpId, message: MessageId) {
        let Some(op) = self.state.outbox.get_mut(&id) else {
            return;
        };
        match &mut op.op {
            RemoteOp::SetFlag { messages, .. } | RemoteOp::Move { messages, .. } => {
                messages.retain(|m| *m != message);
            }
            RemoteOp::Copy { copies, .. } => {
                copies.retain(|c| c.source != message && c.copy != message);
            }
            RemoteOp::FlagMailbox { .. } => {}
        }
    }

    /// An op finished on the server: record where its messages are now, drop it and start the
    /// next one.
    pub(super) fn ingest_op_succeeded(
        &mut self,
        id: OpId,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(op) = self.state.outbox.shift_remove(&id) else {
            // Discarded while it was running.
            return Outcome::new(receipt);
        };
        let mut changes = CacheChanges::default();
        changes.removed_ops.insert(id);
        let mut effects = Vec::new();
        if let Some(to) = op.op.target() {
            if let RemoteOp::Move { messages, .. } = &op.op {
                for message in messages {
                    self.landed(*message, to, &mut changes);
                }
            }
            let mut ids = ctx.ids();
            self.absorb_copies(to, ctx, &mut ids, &mut changes);
            let unknown = self
                .state
                .messages
                .values()
                .any(|m| m.mailbox == to && m.is_placeholder());
            let never_fetched = self
                .state
                .mailboxes
                .get(&to)
                .is_some_and(|m| m.uid_validity.is_none());
            if unknown || never_fetched {
                effects.extend(self.start_fetch(to));
            }
        }
        effects.extend(self.pump_ops(op.account, ctx.now_ms, &mut changes));
        self.op_outcome(receipt.affecting(EntityRef::Op(id)), changes, effects)
    }

    /// `message` was moved to `to` on the server: it is a placeholder there now (shown where it
    /// was shown before, if that differs).
    fn landed(&mut self, message: MessageId, to: MailboxId, changes: &mut CacheChanges) {
        let Some(envelope) = self.state.messages.get_mut(&message) else {
            return;
        };
        if envelope.mailbox == to || envelope.is_placeholder() {
            return;
        }
        let shown = envelope.location();
        envelope.mailbox = to;
        envelope.uid = 0;
        envelope.moved_to = (shown != to).then_some(shown);
        changes.messages.insert(message);
    }

    /// An op failed: record the error and keep the local change. The op is retried after
    /// [`retry_delay_ms`] (a timer is scheduled). An op the server rejected (`permanent`) is
    /// given up once it has failed [`MAX_ATTEMPTS`] times, and the queue moves on; an op that
    /// could not reach the server is retried until it can.
    pub(super) fn ingest_op_failed(
        &mut self,
        id: OpId,
        error: String,
        permanent: bool,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let Some(op) = self.state.outbox.get_mut(&id) else {
            return Outcome::new(receipt);
        };
        op.attempts = op.attempts.saturating_add(1);
        op.last_error = Some(error);
        let mut effects = Vec::new();
        if permanent && op.attempts >= MAX_ATTEMPTS {
            op.state = OpState::Failed;
            op.next_attempt_ms = None;
        } else {
            let at_ms = ctx.now_ms.saturating_add(retry_delay_ms(op.attempts));
            op.state = OpState::Queued;
            op.next_attempt_ms = Some(at_ms);
            effects.push(Effect::ScheduleTick { at_ms });
        }
        let account = op.account;
        let mut changes = CacheChanges::default();
        changes.ops.insert(id);
        effects.extend(self.pump_ops(account, ctx.now_ms, &mut changes));
        self.op_outcome(receipt.affecting(EntityRef::Op(id)), changes, effects)
    }

    fn op_outcome(
        &mut self,
        receipt: CommandReceipt,
        changes: CacheChanges,
        effects: Vec<Effect>,
    ) -> Outcome {
        let delta = self.delta(changes);
        let mut receipt = receipt;
        for op in &delta.ops {
            if !receipt.affected.contains(&EntityRef::Op(op.id)) {
                receipt = receipt.affecting(EntityRef::Op(op.id));
            }
        }
        let mut outcome = Outcome {
            receipt,
            effects: Vec::new(),
        };
        if !delta.is_empty() {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: crate::event::PersistDelta::Mail(delta),
            });
        }
        outcome.effects.extend(effects);
        outcome
    }

    /// Queues ops that set `message`'s local read/star state on the server, where it is
    /// `server` (used when a placeholder is located). The caller pumps the queue.
    pub(super) fn push_flag_diff(
        &mut self,
        message: MessageId,
        server: Flags,
        ctx: &Ctx,
        ids: &mut IdGen,
        changes: &mut CacheChanges,
    ) {
        let Some(envelope) = self.state.messages.get(&message) else {
            return;
        };
        let (account, local) = (envelope.account, envelope.flags);
        let keywords = self.keeps_keywords(envelope.mailbox);
        for flag in [FlagKind::Seen, FlagKind::Flagged, FlagKind::Pinned] {
            if flag.is_keyword() && !keywords {
                continue;
            }
            let value = flag.get(&local);
            if flag.get(&server) != value {
                let op = RemoteOp::SetFlag {
                    flag,
                    value,
                    messages: vec![message],
                };
                self.queue_op(account, op, ctx, ids, changes);
            }
        }
    }

    /// Whether op `id` was never sent to the server, so it can still be changed or dropped.
    fn unsent(&self, id: OpId) -> bool {
        self.state
            .outbox
            .get(&id)
            .is_some_and(|op| op.state == OpState::Queued && op.attempts == 0)
    }

    /// Ops of the outbox, newest first, that were never sent and match `pick`.
    fn unsent_ops(&self, pick: impl Fn(&RemoteOp) -> bool) -> Vec<OpId> {
        self.state
            .outbox
            .values()
            .rev()
            .filter(|op| self.unsent(op.id) && pick(&op.op))
            .map(|op| op.id)
            .collect()
    }

    /// Drops op `id` if it has nothing left to do; otherwise records it as changed.
    fn trim_op(&mut self, id: OpId, changes: &mut CacheChanges) {
        let empty = self.state.outbox.get(&id).is_some_and(|op| match &op.op {
            RemoteOp::SetFlag { messages, .. } | RemoteOp::Move { messages, .. } => {
                messages.is_empty()
            }
            RemoteOp::Copy { copies, .. } => copies.is_empty(),
            RemoteOp::FlagMailbox { .. } => false,
        });
        if empty {
            self.state.outbox.shift_remove(&id);
            changes.removed_ops.insert(id);
        } else {
            changes.ops.insert(id);
        }
    }

    /// Setting `flag` to `value` on `message` cancels an unsent op that sets it the other way.
    /// Returns whether one was cancelled (then no new op is needed).
    pub(super) fn cancel_flag(
        &mut self,
        message: MessageId,
        flag: FlagKind,
        value: bool,
        changes: &mut CacheChanges,
    ) -> bool {
        let found = self.unsent_ops(|op| {
            matches!(op, RemoteOp::SetFlag { flag: f, value: v, messages }
                if *f == flag && *v != value && messages.contains(&message))
        });
        let Some(id) = found.first().copied() else {
            return false;
        };
        self.forget_from_op(id, message);
        self.trim_op(id, changes);
        true
    }

    /// Takes `message` out of an unsent move. Returns whether it was in one: the message has
    /// then not left its server location yet.
    pub(super) fn cancel_move(&mut self, message: MessageId, changes: &mut CacheChanges) -> bool {
        let found = self.unsent_ops(
            |op| matches!(op, RemoteOp::Move { messages, .. } if messages.contains(&message)),
        );
        let Some(id) = found.first().copied() else {
            return false;
        };
        self.forget_from_op(id, message);
        self.trim_op(id, changes);
        true
    }

    /// Whether an op in the outbox (sent or not) still moves `message`.
    pub(super) fn move_pending(&self, message: MessageId) -> bool {
        self.state.outbox.values().any(
            |op| matches!(&op.op, RemoteOp::Move { messages, .. } if messages.contains(&message)),
        )
    }

    /// Moving the placeholder `copy` of an unsent copy back to where its source is shown
    /// cancels the copy: the placeholder is deleted. Returns whether that happened.
    pub(super) fn cancel_copy(
        &mut self,
        copy: MessageId,
        to: MailboxId,
        changes: &mut CacheChanges,
    ) -> bool {
        let found = self.unsent_ops(|op| {
            matches!(op, RemoteOp::Copy { copies, .. } if copies.iter().any(|c| {
                c.copy == copy
                    && self.state.messages.get(&c.source).is_some_and(|s| s.location() == to)
            }))
        });
        let Some(id) = found.first().copied() else {
            return false;
        };
        self.forget_from_op(id, copy);
        self.trim_op(id, changes);
        self.remove_message(copy, changes);
        true
    }

    /// `flags` (as reported by the server) with the changes of queued and running ops applied, so
    /// a sync does not undo a local change that has not reached the server yet.
    pub(super) fn with_pending_flags(&self, message: MessageId, flags: Flags) -> Flags {
        let mut flags = flags;
        // A mailbox that keeps no keywords: the pin lives on the device only (MISC-4).
        if let Some(local) = self.state.messages.get(&message)
            && !self.keeps_keywords(local.mailbox)
        {
            flags.pinned = local.flags.pinned;
        }
        for op in self.state.outbox.values() {
            if let RemoteOp::SetFlag {
                flag,
                value,
                messages,
            } = &op.op
                && messages.contains(&message)
            {
                flag.set(&mut flags, *value);
            }
        }
        flags
    }

    /// Whether `mailbox` keeps keywords such as `$Pinned` on the server (unknown counts as yes).
    pub(super) fn keeps_keywords(&self, mailbox: MailboxId) -> bool {
        self.state
            .mailboxes
            .get(&mailbox)
            .is_none_or(|m| m.custom_keywords != Some(false))
    }
}
