//! Mirror mode (ADR 0017): the engine of a device linked to a dail server, and the device's
//! link commands (`connect_server`, `trust_server`, `disconnect_server`, `retry_offline`,
//! `dismiss_offline_rejection`) with the runtime's `server_offered` and `link_failed`.

use serde_json::json;

use super::certificates::sha256_hex;
use super::{Engine, invalid, not_found};
use crate::account::{Secret, SecretRef};
use crate::command::{Command, CommandKind};
use crate::ctx::Ctx;
use crate::diff::StateDiff;
use crate::error::CoreError;
use crate::event::{Effect, MailDelta, PersistDelta};
use crate::history::History;
use crate::ids::{IdGen, RequestId};
use crate::link::{LinkStatus, QueuedCommand, ServerEndpoint, is_pairing_code};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::synced::LocalSettings;

/// The trimmed host of `connect_server`, or `error_002_invalid_argument`.
fn server_host(host: &str) -> Result<String, CoreError> {
    let host = host.trim();
    let valid = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '[' | ']'));
    if valid {
        Ok(host.to_owned())
    } else {
        Err(invalid("host", "host must be a DNS name or IP address"))
    }
}

impl Engine {
    /// The copy of `cmd` to queue for the server if it succeeds: in mirror mode, while the link
    /// is not online, for a command the server runs that is not a query (queries are answered
    /// from the mirror). While online nothing is queued here: the runtime forwards the command.
    pub(super) fn to_queue(&self, cmd: &Command) -> Option<Command> {
        let link = self.state.link.as_ref()?;
        let queue =
            !link.is_online() && cmd.placement().on_server() && cmd.kind() != CommandKind::Query;
        queue.then(|| cmd.clone())
    }

    /// Mirror mode's last step of every input: drops the effects that only the server runs and
    /// queues `queued`, the command just applied, for the server.
    pub(super) fn mirror(&mut self, outcome: &mut Outcome, queued: Option<Command>, ctx: &Ctx) {
        let Some(link) = self.state.link.as_mut() else {
            return;
        };
        outcome
            .effects
            .retain(crate::event::Effect::is_device_local);
        if let Some(command) = queued {
            link.queued.push(QueuedCommand {
                request_id: RequestId::from_uuid(uuid::Uuid::from_u128(ctx.id_seed)),
                seed: ctx.id_seed,
                origin: ctx.origin,
                command,
                queued_at_ms: ctx.now_ms,
            });
            outcome.receipt.affected.push(EntityRef::Link);
        }
    }

    /// `link_changed`: records the connection, turning mirror mode on. Online clears the last
    /// error.
    pub(super) fn ingest_link_changed(
        &mut self,
        status: LinkStatus,
        receipt: CommandReceipt,
    ) -> Outcome {
        let link = self.state.link.get_or_insert_with(Default::default);
        link.status = status;
        if status == LinkStatus::Online {
            link.error = None;
        }
        Outcome::new(receipt.affecting(EntityRef::Link))
    }

    /// `server_paired`: the server accepted the code; the fingerprint being paired with is
    /// pinned. Any other fingerprint (or no pairing in progress) is ignored.
    pub(super) fn ingest_server_paired(
        &mut self,
        fingerprint: &str,
        receipt: CommandReceipt,
    ) -> Outcome {
        let fingerprint = sha256_hex("fingerprint", fingerprint).ok();
        let server = self
            .state
            .link
            .as_mut()
            .and_then(|link| link.server.as_mut())
            .filter(|server| server.pairing.is_some() && server.pairing == fingerprint);
        match server {
            Some(server) => {
                server.fingerprint = server.pairing.take();
                // The remote profile (`[remote]` in `config.toml`): what the device needs to
                // connect again after a restart.
                let pinned = ServerEndpoint {
                    offered: None,
                    pairing: None,
                    ..server.clone()
                };
                Outcome::new(receipt.affecting(EntityRef::Link)).with_effect(Effect::PersistState {
                    delta: PersistDelta::Remote(Some(pinned)),
                })
            }
            None => Outcome::new(receipt.with_data(json!({ "ignored": true }))),
        }
    }

    /// `link_failed`: the link goes offline with the error, ending a pairing in progress (the
    /// offer stays, so `trust_server` can be retried). Without a link (after
    /// `disconnect_server`, whose only remaining work is deleting the device token) the error
    /// is the keyring's.
    pub(super) fn ingest_link_failed(&mut self, error: String, receipt: CommandReceipt) -> Outcome {
        match self.state.link.as_mut() {
            Some(link) => {
                link.status = LinkStatus::Offline;
                link.error = Some(error);
                if let Some(server) = link.server.as_mut() {
                    server.pairing = None;
                }
                Outcome::new(receipt.affecting(EntityRef::Link))
            }
            None => {
                self.state.status.keyring_error = Some(error);
                Outcome::new(receipt.affecting(EntityRef::Status))
            }
        }
    }

    /// `server_offered`: records the fingerprint the server of `connect_server` presented, for
    /// the user to compare. An offer from another server (or without a link) is ignored.
    pub(super) fn ingest_server_offered(
        &mut self,
        host: &str,
        port: u16,
        fingerprint: &str,
        receipt: CommandReceipt,
    ) -> Outcome {
        let ignored = || Outcome::new(receipt.clone().with_data(json!({ "ignored": true })));
        let Ok(fingerprint) = sha256_hex("fingerprint", fingerprint) else {
            return ignored();
        };
        let Some(link) = self.state.link.as_mut() else {
            return ignored();
        };
        let Some(server) = link
            .server
            .as_mut()
            .filter(|s| s.port == port && s.host.eq_ignore_ascii_case(host))
        else {
            return ignored();
        };
        server.offered = Some(fingerprint);
        link.error = None;
        Outcome::new(receipt.affecting(EntityRef::Link))
    }

    /// `connect_server`: a link to `host:port`, still untrusted, and a probe of its
    /// certificate.
    pub(super) fn connect_server(
        &mut self,
        host: &str,
        port: u16,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !self.state.accounts.is_empty() {
            return Err(CoreError::ProfileHasAccounts {
                accounts: self.state.accounts.len(),
            });
        }
        let host = server_host(host)?;
        if port == 0 {
            return Err(invalid("port", "port must be between 1 and 65535"));
        }
        if let Some(server) = self
            .state
            .link
            .as_ref()
            .and_then(|link| link.server.as_ref())
            .filter(|server| server.is_pinned())
        {
            return Err(CoreError::AlreadyExists {
                entity: "server link".to_owned(),
                key: format!("{}:{}", server.host, server.port),
            });
        }
        let link = self.state.link.get_or_insert_with(Default::default);
        link.status = LinkStatus::Connecting;
        link.error = None;
        link.server = Some(ServerEndpoint {
            host: host.clone(),
            port,
            ..ServerEndpoint::default()
        });
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::Link)
                .with_data(json!({ "host": host, "port": port, "undoable": false })),
        )
        .with_effect(Effect::ProbeServer { host, port }))
    }

    /// `trust_server`: pairs with the code, accepting only the offered certificate. The
    /// fingerprint is pinned when the server accepts the code (`server_paired`); until then it
    /// is `pairing`, and a failed pairing can be retried with a new code.
    pub(super) fn trust_server(
        &mut self,
        fingerprint: &str,
        code: &Secret,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let server = self
            .state
            .link
            .as_ref()
            .and_then(|link| link.server.as_ref())
            .ok_or(CoreError::NotLinked)?;
        if server.is_pinned() {
            return Err(CoreError::AlreadyExists {
                entity: "server link".to_owned(),
                key: format!("{}:{}", server.host, server.port),
            });
        }
        let Some(offered) = server.offered.as_deref() else {
            return Err(invalid(
                "fingerprint",
                "the server has not shown a certificate yet; wait for it after connect_server",
            ));
        };
        let fingerprint = sha256_hex("fingerprint", fingerprint)?;
        if fingerprint != offered {
            return Err(invalid(
                "fingerprint",
                "does not match the certificate the server presented; do not pair",
            ));
        }
        let code = code.expose().trim().to_ascii_uppercase();
        if !is_pairing_code(&code) {
            return Err(invalid(
                "code",
                "expected the 8-character pairing code from start_pairing (A-Z, 2-7)",
            ));
        }
        let Some(server) = self.state.link.as_mut().and_then(|link| {
            link.status = LinkStatus::Connecting;
            link.server.as_mut()
        }) else {
            return Err(CoreError::NotLinked);
        };
        // A retry keeps the keyring entry of the first attempt.
        let token = server
            .token
            .clone()
            .unwrap_or_else(|| SecretRef::for_server(uuid::Uuid::from_u128(ctx.id_seed)));
        server.pairing = Some(fingerprint.clone());
        server.token = Some(token.clone());
        let (host, port) = (server.host.clone(), server.port);
        Ok(
            Outcome::new(receipt.affecting(EntityRef::Link).with_data(json!({
                "host": host,
                "port": port,
                "fingerprint": fingerprint,
                "undoable": false,
            })))
            .with_effect(Effect::PairWithServer {
                host,
                port,
                fingerprint,
                code: Secret::new(code),
                secret: token,
            }),
        )
    }

    /// `disconnect_server`: forgets the link, its queue and the mirrored state (the profile is
    /// a plain one again), and the history of commands applied to the mirror.
    pub(super) fn disconnect_server(
        &mut self,
        confirm: bool,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        if !confirm {
            return Err(CoreError::ConfirmationRequired {
                command: "disconnect_server".to_owned(),
            });
        }
        let link = self.state.link.take().ok_or(CoreError::NotLinked)?;
        let wipe = self.mirror_wipe();
        let settings_before = self.state.settings.clone();
        let synced_before = self.state.synced.clone();
        StateDiff::clear(&mut self.state);
        self.messages_changed();
        self.history = History::default();
        let server = link.server.as_ref();
        let data = json!({
            "disconnected": server.map(|s| format!("{}:{}", s.host, s.port)),
            "lost": link.queued.len(),
            "undoable": false,
        });
        let mut outcome = Outcome::new(receipt.affecting(EntityRef::Link).with_data(data));
        // The mirror's optimistic changes went to the local store (`persist_state` is
        // device-local): remove what was cleared, so it does not come back after a restart.
        for delta in wipe {
            outcome = outcome.with_effect(Effect::PersistState { delta });
        }
        if self.state.settings != settings_before {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Settings(self.state.settings.clone()),
            });
        }
        if self.state.synced != synced_before {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Synced(LocalSettings::of(&self.state.synced)),
            });
        }
        if server.is_some_and(ServerEndpoint::is_pinned) {
            outcome = outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Remote(None),
            });
        }
        Ok(outcome.with_effect(Effect::DisconnectServer {
            secret: server.and_then(|s| s.token.clone()),
        }))
    }

    /// The store changes that remove the mirrored mail and accounts (`disconnect_server`).
    fn mirror_wipe(&self) -> Vec<PersistDelta> {
        let state = &self.state;
        let mut deltas: Vec<PersistDelta> = state
            .accounts
            .keys()
            .map(|id| PersistDelta::AccountRemoved(*id))
            .collect();
        let mail = MailDelta {
            removed_mailboxes: state.mailboxes.keys().copied().collect(),
            removed_messages: state.messages.keys().copied().collect(),
            removed_threads: state.threads.keys().copied().collect(),
            removed_ops: state.outbox.keys().copied().collect(),
            removed_drafts: state.drafts.keys().copied().collect(),
            removed_approvals: state.approvals.keys().copied().collect(),
            ..MailDelta::default()
        };
        if !mail.is_empty() {
            deltas.push(PersistDelta::Mail(mail));
        }
        deltas
    }

    /// `retry_offline`: every refused command goes back to the end of the queue, with its seed
    /// and origin and a new request ID (the server answers a known request ID from memory, so
    /// the old one would only bring back the refusal).
    pub(super) fn retry_offline(
        &mut self,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let link = self.state.link.as_mut().ok_or(CoreError::NotLinked)?;
        let mut ids = IdGen::new(ctx.id_seed);
        let mut retried = Vec::new();
        for rejection in std::mem::take(&mut link.rejected) {
            let request_id: RequestId = ids.next_id();
            retried.push(request_id);
            link.queued.push(QueuedCommand {
                request_id,
                seed: rejection.seed,
                origin: rejection.origin,
                command: rejection.command,
                queued_at_ms: ctx.now_ms,
            });
        }
        Ok(Outcome::new(receipt.affecting(EntityRef::Link).with_data(
            json!({ "retried": retried, "undoable": false }),
        )))
    }

    /// `dismiss_offline_rejection`: drops one refused command.
    pub(super) fn dismiss_offline_rejection(
        &mut self,
        id: RequestId,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let link = self.state.link.as_mut().ok_or(CoreError::NotLinked)?;
        let index = link
            .rejected
            .iter()
            .position(|r| r.request_id == id)
            .ok_or_else(|| not_found("offline rejection", id))?;
        let rejection = link.rejected.remove(index);
        Ok(Outcome::new(receipt.affecting(EntityRef::Link).with_data(
            json!({
                "dismissed": id,
                "command": rejection.command.name(),
                "undoable": false,
            }),
        )))
    }

    /// `remote_synced`: applies the server's diff to the mirror (a snapshot over a cleared
    /// state) and records its revision. A diff not newer than the mirror is ignored. The mirror's
    /// mail as it is now goes to the local store (`persist_state`, device-local), so a change
    /// applied optimistically and then overwritten (or cleared by a snapshot) does not come back
    /// after a restart.
    pub(super) fn ingest_remote_synced(
        &mut self,
        revision: u64,
        diff: &StateDiff,
        snapshot: bool,
        receipt: CommandReceipt,
    ) -> Outcome {
        let link = self.state.link.get_or_insert_with(Default::default);
        if !snapshot && revision <= link.revision {
            return Outcome::new(receipt.with_data(serde_json::json!({ "stale": true })));
        }
        link.revision = revision;
        let mail = if snapshot {
            let old = MirroredMail::take(&mut self.state);
            StateDiff::clear(&mut self.state);
            diff.apply(&mut self.state);
            old.delta_to(&self.state)
        } else {
            diff.apply(&mut self.state);
            mail_delta(diff)
        };
        if snapshot || diff.touches_messages() {
            self.messages_changed();
        }
        let outcome = Outcome::new(receipt.affecting(EntityRef::Link));
        if mail.is_empty() {
            outcome
        } else {
            outcome.with_effect(Effect::PersistState {
                delta: PersistDelta::Mail(mail),
            })
        }
    }
}

/// The mail of a diff, as the store keeps it.
fn mail_delta(diff: &StateDiff) -> MailDelta {
    MailDelta {
        mailboxes: diff.mailboxes.upsert.values().cloned().collect(),
        removed_mailboxes: diff.mailboxes.remove.clone(),
        messages: diff.messages.upsert.values().cloned().collect(),
        removed_messages: diff.messages.remove.clone(),
        threads: diff.threads.upsert.values().cloned().collect(),
        removed_threads: diff.threads.remove.clone(),
        ops: diff.outbox.upsert.values().cloned().collect(),
        removed_ops: diff.outbox.remove.clone(),
        drafts: diff.drafts.upsert.values().cloned().collect(),
        removed_drafts: diff.drafts.remove.clone(),
        approvals: diff.approvals.upsert.values().cloned().collect(),
        removed_approvals: diff.approvals.remove.clone(),
    }
}

/// The mirror's mail before a snapshot, to persist only what the snapshot changed.
struct MirroredMail {
    mailboxes: indexmap::IndexMap<crate::ids::MailboxId, crate::mailbox::Mailbox>,
    messages: indexmap::IndexMap<crate::ids::MessageId, crate::message::Envelope>,
    threads: indexmap::IndexMap<crate::ids::ThreadId, crate::message::Thread>,
    outbox: indexmap::IndexMap<crate::ids::OpId, crate::ops::PendingOp>,
    drafts: indexmap::IndexMap<crate::ids::DraftId, crate::draft::Draft>,
    approvals: indexmap::IndexMap<crate::ids::ApprovalId, crate::approval::ApprovalRequest>,
}

/// Upserts of what is new or changed in `new`, removals of what is gone from it.
fn changed<K: Copy + std::hash::Hash + Eq, V: Clone + PartialEq>(
    old: &indexmap::IndexMap<K, V>,
    new: &indexmap::IndexMap<K, V>,
) -> (Vec<V>, Vec<K>) {
    let upsert = new
        .iter()
        .filter(|(key, value)| old.get(*key) != Some(*value))
        .map(|(_, value)| value.clone())
        .collect();
    let removed = old
        .keys()
        .filter(|key| !new.contains_key(*key))
        .copied()
        .collect();
    (upsert, removed)
}

impl MirroredMail {
    fn take(state: &mut crate::state::AppState) -> Self {
        Self {
            mailboxes: std::mem::take(&mut state.mailboxes),
            messages: std::mem::take(&mut state.messages),
            threads: std::mem::take(&mut state.threads),
            outbox: std::mem::take(&mut state.outbox),
            drafts: std::mem::take(&mut state.drafts),
            approvals: std::mem::take(&mut state.approvals),
        }
    }

    fn delta_to(&self, state: &crate::state::AppState) -> MailDelta {
        let (mailboxes, removed_mailboxes) = changed(&self.mailboxes, &state.mailboxes);
        let (messages, removed_messages) = changed(&self.messages, &state.messages);
        let (threads, removed_threads) = changed(&self.threads, &state.threads);
        let (ops, removed_ops) = changed(&self.outbox, &state.outbox);
        let (drafts, removed_drafts) = changed(&self.drafts, &state.drafts);
        let (approvals, removed_approvals) = changed(&self.approvals, &state.approvals);
        MailDelta {
            mailboxes,
            removed_mailboxes,
            messages,
            removed_messages,
            threads,
            removed_threads,
            ops,
            removed_ops,
            drafts,
            removed_drafts,
            approvals,
            removed_approvals,
        }
    }
}
