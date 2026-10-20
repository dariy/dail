//! `RedbStore`: the mail cache in `state.redb` (`ARCHITECTURE.md §6`).
//!
//! Tables map 128-bit IDs to JSON records: `mailboxes`, `messages` (envelopes), `threads`,
//! `outbox` (pending ops), `drafts`, `approvals` (agents' send requests); `meta` holds `schema_version`. Each [`MailDelta`] is applied in one ACID write transaction.

use std::path::{Path, PathBuf};

use app_core::{AppState, ApprovalRequest, Draft, Envelope, MailDelta, Mailbox, PendingOp, Thread};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::IoError;
use super::migrations;

const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const MAILBOXES: TableDefinition<u128, &[u8]> = TableDefinition::new("mailboxes");
const MESSAGES: TableDefinition<u128, &[u8]> = TableDefinition::new("messages");
const THREADS: TableDefinition<u128, &[u8]> = TableDefinition::new("threads");
const OUTBOX: TableDefinition<u128, &[u8]> = TableDefinition::new("outbox");
const DRAFTS: TableDefinition<u128, &[u8]> = TableDefinition::new("drafts");
const APPROVALS: TableDefinition<u128, &[u8]> = TableDefinition::new("approvals");
const SCHEMA_VERSION: &str = "schema_version";

/// The mail cache of one profile.
pub struct RedbStore {
    path: PathBuf,
    // Transactions share it; `compact` needs it alone.
    db: parking_lot::RwLock<Database>,
}

impl std::fmt::Debug for RedbStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl RedbStore {
    /// Opens (or creates) the database at `path` and migrates it to the current schema.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, IoError> {
        let path = path.into();
        let db = Database::create(&path).map_err(|err| IoError::db(&path, err))?;
        let store = Self {
            path,
            db: parking_lot::RwLock::new(db),
        };
        store.migrate()?;
        Ok(store)
    }

    /// The database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks the file's checksums and structure (redb `check_integrity`; repairs what it
    /// can). `false`: it had to repair something.
    pub fn check_integrity(&self) -> Result<bool, IoError> {
        self.db.write().check_integrity().map_err(|e| self.err(e))
    }

    /// Gives the free pages back to the file system (redb `compact`).
    pub fn compact(&self) -> Result<(), IoError> {
        self.db.write().compact().map_err(|e| self.err(e))?;
        Ok(())
    }

    fn err(&self, err: impl std::fmt::Display) -> IoError {
        IoError::db(&self.path, err)
    }

    fn migrate(&self) -> Result<(), IoError> {
        let txn = self.db.read().begin_write().map_err(|e| self.err(e))?;
        let found = {
            let meta = txn.open_table(META).map_err(|e| self.err(e))?;
            meta.get(SCHEMA_VERSION)
                .map_err(|e| self.err(e))?
                .map(|v| v.value())
        };
        match found {
            Some(version) if version > migrations::CURRENT => {
                return Err(IoError::SchemaTooNew {
                    path: self.path.display().to_string(),
                    found: version,
                    supported: migrations::CURRENT,
                });
            }
            Some(version) => {
                for migration in migrations::pending(version) {
                    migration(&txn).map_err(|e| self.err(e))?;
                }
            }
            None => {}
        }
        // Creates missing tables (fresh databases; `drafts` and `approvals` are additive within
        // version 1) and
        // stamps the version.
        for table in [MAILBOXES, MESSAGES, THREADS, OUTBOX, DRAFTS, APPROVALS] {
            txn.open_table(table).map_err(|e| self.err(e))?;
        }
        txn.open_table(META)
            .map_err(|e| self.err(e))?
            .insert(SCHEMA_VERSION, migrations::CURRENT)
            .map_err(|e| self.err(e))?;
        txn.commit().map_err(|e| self.err(e))
    }

    /// The schema version recorded in the file.
    pub fn schema_version(&self) -> Result<u64, IoError> {
        let txn = self.db.read().begin_read().map_err(|e| self.err(e))?;
        let meta = txn.open_table(META).map_err(|e| self.err(e))?;
        Ok(meta
            .get(SCHEMA_VERSION)
            .map_err(|e| self.err(e))?
            .map_or(0, |v| v.value()))
    }

    /// Fills the mail collections of `state` from the cache. Unreadable records are skipped.
    pub fn load_into(&self, state: &mut AppState) -> Result<(), IoError> {
        let txn = self.db.read().begin_read().map_err(|e| self.err(e))?;
        for mailbox in self.read_all::<Mailbox>(&txn, MAILBOXES)? {
            state.mailboxes.insert(mailbox.id, mailbox);
        }
        for envelope in self.read_all::<Envelope>(&txn, MESSAGES)? {
            state.messages.insert(envelope.id, envelope);
        }
        for thread in self.read_all::<Thread>(&txn, THREADS)? {
            state.threads.insert(thread.id, thread);
        }
        let mut ops = self.read_all::<PendingOp>(&txn, OUTBOX)?;
        // Keys are random-looking IDs; the queue order is the creation order.
        ops.sort_by_key(|op| op.created_ms);
        for op in ops {
            state.outbox.insert(op.id, op);
        }
        let mut drafts = self.read_all::<Draft>(&txn, DRAFTS)?;
        drafts.sort_by_key(|draft| draft.created_ms);
        for draft in drafts {
            state.drafts.insert(draft.id, draft);
        }
        let mut approvals = self.read_all::<ApprovalRequest>(&txn, APPROVALS)?;
        approvals.sort_by_key(|approval| approval.requested_ms);
        for approval in approvals {
            state.approvals.insert(approval.id, approval);
        }
        Ok(())
    }

    fn read_all<T: DeserializeOwned>(
        &self,
        txn: &redb::ReadTransaction,
        table: TableDefinition<u128, &[u8]>,
    ) -> Result<Vec<T>, IoError> {
        let name = table.to_string();
        let table = txn.open_table(table).map_err(|e| self.err(e))?;
        let mut out = Vec::new();
        for row in table.iter().map_err(|e| self.err(e))? {
            let (key, value) = row.map_err(|e| self.err(e))?;
            match serde_json::from_slice(value.value()) {
                Ok(record) => out.push(record),
                Err(err) => tracing::warn!(
                    table = %name,
                    key = key.value(),
                    error = %err,
                    "skipping unreadable cache record"
                ),
            }
        }
        Ok(out)
    }

    /// Applies one delta in a single transaction: removals first, then upserts.
    pub fn apply_mail(&self, delta: &MailDelta) -> Result<(), IoError> {
        let txn = self.db.read().begin_write().map_err(|e| self.err(e))?;
        {
            let mut mailboxes = txn.open_table(MAILBOXES).map_err(|e| self.err(e))?;
            let mut messages = txn.open_table(MESSAGES).map_err(|e| self.err(e))?;
            let mut threads = txn.open_table(THREADS).map_err(|e| self.err(e))?;
            let mut outbox = txn.open_table(OUTBOX).map_err(|e| self.err(e))?;
            let mut drafts = txn.open_table(DRAFTS).map_err(|e| self.err(e))?;
            let mut approvals = txn.open_table(APPROVALS).map_err(|e| self.err(e))?;
            for id in &delta.removed_approvals {
                approvals
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for approval in &delta.approvals {
                let bytes = self.encode(approval)?;
                approvals
                    .insert(approval.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_drafts {
                drafts
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for draft in &delta.drafts {
                let bytes = self.encode(draft)?;
                drafts
                    .insert(draft.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_ops {
                outbox
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for op in &delta.ops {
                let bytes = self.encode(op)?;
                outbox
                    .insert(op.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_mailboxes {
                mailboxes
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_messages {
                messages
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for id in &delta.removed_threads {
                threads
                    .remove(id.as_uuid().as_u128())
                    .map_err(|e| self.err(e))?;
            }
            for mailbox in &delta.mailboxes {
                let bytes = self.encode(mailbox)?;
                mailboxes
                    .insert(mailbox.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for envelope in &delta.messages {
                let bytes = self.encode(envelope)?;
                messages
                    .insert(envelope.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
            for thread in &delta.threads {
                let bytes = self.encode(thread)?;
                threads
                    .insert(thread.id.as_uuid().as_u128(), bytes.as_slice())
                    .map_err(|e| self.err(e))?;
            }
        }
        txn.commit().map_err(|e| self.err(e))
    }

    fn encode(&self, record: &impl Serialize) -> Result<Vec<u8>, IoError> {
        serde_json::to_vec(record).map_err(|e| self.err(e))
    }
}
