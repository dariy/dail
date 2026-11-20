//! The link of a device to a dail server (ADR 0017): where each command runs, and the state of
//! a mirror (`AppState::link`).
//!
//! A device with a remote profile runs its own engine over a *mirror* of the server's state.
//! While [`crate::AppState::link`] is set the engine is in **mirror mode**:
//!
//! - every command still applies locally at once (optimistic), but the engine emits only the
//!   effects that stay on the device ([`crate::Effect::is_device_local`]): the server is the only
//!   one that talks to mail servers, runs rules, polls and sends;
//! - a [`Placement::Server`] or [`Placement::ServerWithUpload`] command that is not a query and
//!   succeeds while the link is not [`LinkStatus::Online`] is appended to [`LinkState::queued`],
//!   to be replayed in order on reconnect. While the link is online it is not queued: the runtime
//!   forwards it to the server itself (SRV-5);
//! - the server's changes arrive as [`crate::Event::RemoteSynced`], whose
//!   [`StateDiff`](crate::diff::StateDiff) overwrites the optimistic result.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::ctx::Origin;
use crate::ids::RequestId;

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
}

impl LinkState {
    /// Whether commands for the server are forwarded now rather than queued.
    pub fn is_online(&self) -> bool {
        self.status == LinkStatus::Online
    }
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

/// A queued command the server refused when it was replayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OfflineRejection {
    /// The refused submission.
    pub request_id: RequestId,
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
