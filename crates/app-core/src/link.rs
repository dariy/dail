//! The link of a device to a dail server (ADR 0017): where each command runs, and the state of
//! a mirror (`AppState::link`).
//!
//! A device with a remote profile runs its own engine over a *mirror* of the server's state.
//! While [`crate::AppState::link`] is set the engine is in **mirror mode**:
//!
//! - every command still applies locally at once (optimistic), but the engine emits only the
//!   effects that stay on the device ([`crate::Effect::is_device_local`]): the server is the only
//!   one that talks to mail servers, runs rules, polls and sends;
//! - while the link is not [`LinkStatus::Online`], a command's [`OfflinePolicy`] decides: a
//!   [`OfflinePolicy::Queue`] command that succeeds is appended to [`LinkState::queued`], to be
//!   replayed in order on reconnect; a [`OfflinePolicy::Refuse`] command is refused
//!   (`error_014_refused_offline`) and changes nothing. While the link is online nothing is
//!   queued here: the runtime forwards the command itself (SRV-5);
//! - the runtime reports each replayed command's fate as [`crate::Event::QueuedDelivered`] or
//!   [`crate::Event::QueuedRejected`] (moved to [`LinkState::rejected`]), and a forwarded command
//!   whose answer was lost with the connection as [`crate::Event::Requeued`];
//! - every change of the queue or the rejections goes to the device's store
//!   ([`crate::PersistDelta::OfflineQueue`]), so they survive a restart;
//! - the server's changes arrive as [`crate::Event::RemoteSynced`], whose
//!   [`StateDiff`](crate::diff::StateDiff) overwrites the optimistic result.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::ctx::Origin;
use crate::ids::{DeviceId, RequestId};

/// Where a command runs when the profile is linked to a dail server (ADR 0017). Without a link
/// every command runs here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// On the server, which owns the accounts and the mail: mutations, sends, destructive and
    /// account commands, rules and queries over mail. The device applies it to its mirror at
    /// once and forwards (or queues) it.
    Server,
    /// On the device only: what the user looks at (every `View` command), the device's own
    /// appearance and store, and updates of the device's own dail.
    Client,
    /// On the server, after the device uploads the local file the command names
    /// (`attach_file`).
    ServerWithUpload,
    /// On the device, with a body the device fetches from the server (`save_attachments`
    /// writes files into a local folder).
    ClientWithBlob,
}

impl Placement {
    /// Whether the server runs the command.
    pub const fn on_server(self) -> bool {
        match self {
            Self::Server | Self::ServerWithUpload => true,
            Self::Client | Self::ClientWithBlob => false,
        }
    }
}

/// What a command does while a mirror is not connected to its server (ADR 0017, SRV-5b):
/// [`crate::registry::CommandName::offline_policy`], refined by [`crate::Command::offline_policy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OfflinePolicy {
    /// Applied to the mirror at once and queued for the server (`link.queued`), which runs it
    /// on reconnect: mutations of mail, drafts, folders and rules, undo/redo, and sends (the
    /// server starts their undo window when they arrive).
    Queue,
    /// Refused while offline (`error_014_refused_offline`), nothing applied or queued: what is
    /// irreversible (destructive commands), what manages accounts, certificates, devices or
    /// approvals on the server, and what only makes sense now (`sync_now`, `pull_settings`,
    /// `push_settings`).
    Refuse,
    /// Not affected by the link: commands that run on the device, and queries, answered from
    /// the mirror.
    Local,
}

/// The persisted part of a link (`PersistDelta::OfflineQueue`): what waits for the server and
/// what it refused. The rest of `link` comes from `[remote]` or the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OfflineQueue {
    /// `LinkState::queued`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<QueuedCommand>,
    /// `LinkState::rejected`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected: Vec<OfflineRejection>,
}

impl OfflineQueue {
    /// The queue and rejections of `link`.
    pub fn of(link: &LinkState) -> Self {
        Self {
            queued: link.queued.clone(),
            rejected: link.rejected.clone(),
        }
    }

    /// Whether nothing waits and nothing was refused.
    pub fn is_empty(&self) -> bool {
        self.queued.is_empty() && self.rejected.is_empty()
    }

    /// Puts the queue and rejections into `state.link`; without a link (the profile is no
    /// longer remote) there is nothing to restore.
    pub fn apply_to(&self, state: &mut crate::AppState) {
        if let Some(link) = state.link.as_mut() {
            link.queued.clone_from(&self.queued);
            link.rejected.clone_from(&self.rejected);
        }
    }
}

/// The connection of a mirror to its server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    /// Dialing the server, or waiting for its first snapshot.
    #[default]
    Connecting,
    /// Connected: commands for the server are forwarded as they are submitted.
    Online,
    /// Not connected: commands for the server are queued (`LinkState::queued`).
    Offline,
}

/// The state of a device linked to a dail server: present only in mirror mode. Device-local:
/// never part of a [`crate::diff::StateDiff`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkState {
    /// The connection.
    pub status: LinkStatus,
    /// The server revision the mirror is at (the last `remote_synced`); 0 before the first.
    pub revision: u64,
    /// Commands for the server submitted while not online, oldest first; replayed in order on
    /// reconnect.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<QueuedCommand>,
    /// Queued commands the server refused on replay, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejected: Vec<OfflineRejection>,
    /// The server this device connects to (`connect_server`), with the certificate fingerprint
    /// it offered and the one the user trusted (`trust_server`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerEndpoint>,
    /// Why the last probe, pairing or connection failed (`Event::LinkFailed`); cleared when the
    /// server answers again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl LinkState {
    /// Whether commands for the server are forwarded now rather than queued.
    pub fn is_online(&self) -> bool {
        self.status == LinkStatus::Online
    }
}

/// The dail server a device connects to (`connect_server`, ADR 0017).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServerEndpoint {
    /// Host name or IP address.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// SHA-256 fingerprint of the certificate the server presented on the last probe
    /// (lower-case hex), for the user to compare with the one the server shows. Not trusted
    /// until `trust_server` pins it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offered: Option<String>,
    /// The fingerprint `trust_server` is pairing with (lower-case hex), until the server
    /// accepts the code (`Event::ServerPaired` pins it) or the pairing fails (`link_failed`
    /// clears it, so `trust_server` can be run again with a new code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairing: Option<String>,
    /// The pinned fingerprint (lower-case hex), set once pairing succeeded
    /// (`Event::ServerPaired`). The device accepts only this certificate from the server; web
    /// roots are never consulted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Where the device token lives in the OS keyring (set by `trust_server`). The token
    /// itself never enters the state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<crate::account::SecretRef>,
}

impl ServerEndpoint {
    /// Whether the device is paired: the server accepted the code and its certificate is
    /// pinned.
    pub const fn is_pinned(&self) -> bool {
        self.fingerprint.is_some()
    }
}

/// What a paired device may do on the server (ADR 0017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRole {
    /// A device of the user: it may submit as the GUI, the TUI, the CLI or a batch, so it can
    /// approve agents' sends.
    User,
    /// An agent: it may submit only as MCP, under the agent policy and approvals.
    Agent,
}

impl DeviceRole {
    /// The role's `snake_case` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }
}

/// A device paired with this dail server (ADR 0017). Its token is never here: the server
/// keeps only the token's SHA-256, in `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Device {
    /// The device.
    pub id: DeviceId,
    /// The name given when pairing (`start_pairing`), e.g. `Phone`.
    pub name: String,
    /// What it may do.
    pub role: DeviceRole,
    /// When it was paired, in Unix milliseconds.
    pub paired_at_ms: i64,
    /// When it last connected, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_ms: Option<i64>,
}

/// Characters of a pairing code (`start_pairing`): RFC 4648 base32, 5 bits each.
pub const PAIRING_CODE_LEN: usize = 8;

/// How long a pairing code is valid, in milliseconds (10 minutes).
pub const PAIRING_TTL_MS: i64 = 10 * 60 * 1000;

/// The RFC 4648 base32 alphabet.
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// The single-use pairing code of a `start_pairing` dispatched with `seed`: 40 bits of the seed
/// (its 40-bit chunks folded with XOR, so that a random seed and a counter in the high bits both
/// give distinct codes) as 8 base32 characters. The core has no RNG: the runtime's seeds are
/// random, so the code is too. Whoever knows the seed knows the code, which is why the audit log
/// leaves out the seed of `start_pairing` ([`crate::ReplayRecord::redacted_line`]).
pub fn pairing_code(seed: u128) -> String {
    const MASK: u128 = (1 << 40) - 1;
    let bits = (seed ^ (seed >> 40) ^ (seed >> 80) ^ (seed >> 120)) & MASK;
    (0..PAIRING_CODE_LEN)
        .rev()
        .map(|i| {
            let index = (bits >> (i * 5)) & 0x1f;
            BASE32
                .get(usize::try_from(index).unwrap_or_default())
                .map_or('A', |b| char::from(*b))
        })
        .collect()
}

/// Whether `code` is a pairing code: 8 base32 characters (upper-case, `2`–`7`).
pub fn is_pairing_code(code: &str) -> bool {
    code.len() == PAIRING_CODE_LEN && code.bytes().all(|b| BASE32.contains(&b))
}

/// A command for the server, applied to the mirror while offline and waiting to be sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueuedCommand {
    /// Identifies the submission, so that a resend is harmless: the call's `id_seed` as a UUID.
    pub request_id: RequestId,
    /// The `Ctx::id_seed` the command was dispatched with here. The server dispatches it with the
    /// same seed, so the IDs it creates (a draft) are the ones the device already uses. Written
    /// as a decimal string: a JSON number can't hold every `u128`.
    #[serde(with = "seed_text")]
    #[schemars(with = "String")]
    pub seed: u128,
    /// Where the command came from.
    pub origin: Origin,
    /// The command.
    pub command: Command,
    /// When it was queued, in Unix milliseconds.
    pub queued_at_ms: i64,
}

/// A queued command the server refused when it was replayed. `retry_offline` queues it again
/// with its seed and origin, under a new request ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OfflineRejection {
    /// The refused submission.
    pub request_id: RequestId,
    /// The `Ctx::id_seed` it was queued with ([`QueuedCommand::seed`]), as a decimal string.
    #[serde(with = "seed_text")]
    #[schemars(with = "String")]
    pub seed: u128,
    /// Where the command came from: a retry keeps it, so that an agent's command never comes
    /// back as the user's.
    pub origin: Origin,
    /// The command.
    pub command: Command,
    /// The error code (`ERR…`).
    pub code: String,
    /// The error message.
    pub error: String,
    /// When the server refused it, in Unix milliseconds.
    pub at_ms: i64,
}

/// A `u128` as a decimal string (`serde_json::Value` holds at most 64-bit numbers); a number is
/// read too.
mod seed_text {
    use std::fmt;

    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(seed: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(seed)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<u128, D::Error> {
        struct Seed;

        impl Visitor<'_> for Seed {
            type Value = u128;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a u128 as a decimal string")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<u128, E> {
                value.parse().map_err(E::custom)
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<u128, E> {
                Ok(u128::from(value))
            }

            fn visit_u128<E: de::Error>(self, value: u128) -> Result<u128, E> {
                Ok(value)
            }
        }

        deserializer.deserialize_any(Seed)
    }
}
