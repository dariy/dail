//! `StateDiff`: the change between two `AppState`s, as a dail server sends it to its mirrors
//! (ADR 0017, `Event::RemoteSynced`).
//!
//! A diff covers the shared state only. Device-local fields are never part of it: `view` (each
//! device shows its own), `update` (each device updates its own dail), `link` (the device's
//! connection), the in-memory `revision`, and the device's own fields of `settings` and `status`
//! (`Settings::keep_device_local`, `StatusState::keep_device_local`). For everything else
//! `StateDiff::diff(a, b).apply(a)` gives `b`, entry order included.

use std::hash::Hash;

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::approval::ApprovalRequest;
use crate::draft::Draft;
use crate::ids::{AccountId, ApprovalId, DraftId, MailboxId, MessageId, OpId, ThreadId};
use crate::mailbox::Mailbox;
use crate::message::{Envelope, Thread};
use crate::ops::PendingOp;
use crate::state::{AppState, Settings, StatusState};
use crate::synced::SyncedState;

/// The change of one keyed collection: entries added or changed, keys removed, and the new key
/// order when inserting and removing alone would not give it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(bound(
    serialize = "K: Serialize + Hash + Eq, V: Serialize",
    deserialize = "K: Deserialize<'de> + Hash + Eq, V: Deserialize<'de>"
))]
pub struct MapDiff<K: Hash + Eq, V> {
    /// Entries that are new or changed, in their new order. A new key goes to the end.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub upsert: IndexMap<K, V>,
    /// Keys that are gone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<K>,
    /// Every key in its new order, if the order differs from the one the removals and upserts
    /// give.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<K>>,
}

impl<K: Hash + Eq, V> Default for MapDiff<K, V> {
    fn default() -> Self {
        Self {
            upsert: IndexMap::new(),
            remove: Vec::new(),
            order: None,
        }
    }
}

impl<K: Hash + Eq + Clone, V: PartialEq + Clone> MapDiff<K, V> {
    /// The change from `old` to `new`.
    pub fn diff(old: &IndexMap<K, V>, new: &IndexMap<K, V>) -> Self {
        let remove: Vec<K> = old
            .keys()
            .filter(|key| !new.contains_key(*key))
            .cloned()
            .collect();
        let upsert: IndexMap<K, V> = new
            .iter()
            .filter(|(key, value)| old.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        // The order `apply` gives without `order`: the kept keys in their old order, then the
        // new keys in their new order.
        let kept = old.keys().filter(|key| new.contains_key(*key));
        let added = new.keys().filter(|key| !old.contains_key(*key));
        let order = if kept.chain(added).eq(new.keys()) {
            None
        } else {
            Some(new.keys().cloned().collect())
        };
        Self {
            upsert,
            remove,
            order,
        }
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.upsert.is_empty() && self.remove.is_empty() && self.order.is_none()
    }

    /// Applies the change to `map`.
    pub fn apply(&self, map: &mut IndexMap<K, V>) {
        for key in &self.remove {
            map.shift_remove(key);
        }
        for (key, value) in &self.upsert {
            map.insert(key.clone(), value.clone());
        }
        if let Some(order) = &self.order {
            let mut reordered = IndexMap::with_capacity(map.len());
            for key in order {
                if let Some(value) = map.shift_remove(key) {
                    reordered.insert(key.clone(), value);
                }
            }
            // Keys the order does not name (none, for a diff of this map) stay at the end.
            reordered.extend(map.drain(..));
            *map = reordered;
        }
    }
}

/// The change between two states, without the device-local fields (`view`, `update`, `link`).
/// Collections change entry by entry; the other fields are replaced whole when they changed.
/// `settings` and `status` count as changed only when their server fields did, and `apply` keeps
/// the device's own fields of both (`Settings::keep_device_local`,
/// `StatusState::keep_device_local`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateDiff {
    /// `AppState::accounts`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub accounts: MapDiff<AccountId, Account>,
    /// `AppState::mailboxes`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub mailboxes: MapDiff<MailboxId, Mailbox>,
    /// `AppState::messages`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub messages: MapDiff<MessageId, Envelope>,
    /// `AppState::threads`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub threads: MapDiff<ThreadId, Thread>,
    /// `AppState::drafts`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub drafts: MapDiff<DraftId, Draft>,
    /// `AppState::outbox`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub outbox: MapDiff<OpId, PendingOp>,
    /// `AppState::approvals`.
    #[serde(default, skip_serializing_if = "MapDiff::is_empty")]
    pub approvals: MapDiff<ApprovalId, ApprovalRequest>,
    /// `AppState::settings`, if its server fields changed; its device fields are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Settings>,
    /// `AppState::status`, if its server fields changed; its device fields are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusState>,
    /// `AppState::synced`, if it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced: Option<SyncedState>,
}

/// `Some(new)` if it differs from `old`.
fn changed<T: PartialEq + Clone>(old: &T, new: &T) -> Option<T> {
    (old != new).then(|| new.clone())
}

impl StateDiff {
    /// The change from `old` to `new`.
    pub fn diff(old: &AppState, new: &AppState) -> Self {
        // Every field is named, so that a new one has to be placed here or excluded on purpose.
        let AppState {
            accounts,
            mailboxes,
            messages,
            threads,
            drafts,
            outbox,
            approvals,
            view: _,
            settings,
            status,
            update: _,
            synced,
            link: _,
            revision: _,
        } = new;
        Self {
            accounts: MapDiff::diff(&old.accounts, accounts),
            mailboxes: MapDiff::diff(&old.mailboxes, mailboxes),
            messages: MapDiff::diff(&old.messages, messages),
            threads: MapDiff::diff(&old.threads, threads),
            drafts: MapDiff::diff(&old.drafts, drafts),
            outbox: MapDiff::diff(&old.outbox, outbox),
            approvals: MapDiff::diff(&old.approvals, approvals),
            settings: {
                let mut settings = settings.clone();
                settings.keep_device_local(&old.settings);
                changed(&old.settings, &settings)
            },
            status: {
                let mut status = status.clone();
                status.keep_device_local(&old.status);
                changed(&old.status, &status)
            },
            synced: changed(&old.synced, synced),
        }
    }

    /// The whole shared state of `state`, as a diff from an empty state (a snapshot).
    pub fn snapshot(state: &AppState) -> Self {
        Self::diff(&AppState::default(), state)
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
            && self.mailboxes.is_empty()
            && self.messages.is_empty()
            && self.threads.is_empty()
            && self.drafts.is_empty()
            && self.outbox.is_empty()
            && self.approvals.is_empty()
            && self.settings.is_none()
            && self.status.is_none()
            && self.synced.is_none()
    }

    /// Whether the diff changes `messages` (the engine's threading indexes then start over).
    pub fn touches_messages(&self) -> bool {
        !self.messages.is_empty() || !self.threads.is_empty()
    }

    /// Applies the change to `state`. `view`, `update` and `link` are left alone.
    pub fn apply(&self, state: &mut AppState) {
        self.accounts.apply(&mut state.accounts);
        self.mailboxes.apply(&mut state.mailboxes);
        self.messages.apply(&mut state.messages);
        self.threads.apply(&mut state.threads);
        self.drafts.apply(&mut state.drafts);
        self.outbox.apply(&mut state.outbox);
        self.approvals.apply(&mut state.approvals);
        if let Some(settings) = &self.settings {
            let mut settings = settings.clone();
            settings.keep_device_local(&state.settings);
            state.settings = settings;
        }
        if let Some(status) = &self.status {
            let mut status = status.clone();
            status.keep_device_local(&state.status);
            state.status = status;
        }
        if let Some(synced) = &self.synced {
            state.synced = synced.clone();
        }
    }

    /// Clears the shared state of `state` (every field a diff covers, but the device's own
    /// settings and status), before a snapshot is applied over it.
    pub fn clear(state: &mut AppState) {
        let empty = AppState::default();
        state.accounts = empty.accounts;
        state.mailboxes = empty.mailboxes;
        state.messages = empty.messages;
        state.threads = empty.threads;
        state.drafts = empty.drafts;
        state.outbox = empty.outbox;
        state.approvals = empty.approvals;
        let mut settings = empty.settings;
        settings.keep_device_local(&state.settings);
        state.settings = settings;
        let mut status = empty.status;
        status.keep_device_local(&state.status);
        state.status = status;
        state.synced = empty.synced;
    }
}
