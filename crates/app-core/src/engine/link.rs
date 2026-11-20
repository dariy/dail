//! Mirror mode (ADR 0017): the engine of a device linked to a dail server.

use super::Engine;
use crate::command::{Command, CommandKind};
use crate::ctx::Ctx;
use crate::diff::StateDiff;
use crate::ids::RequestId;
use crate::link::{LinkStatus, QueuedCommand};
use crate::receipt::{CommandReceipt, EntityRef, Outcome};

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

    /// `link_changed`: records the connection, turning mirror mode on.
    pub(super) fn ingest_link_changed(
        &mut self,
        status: LinkStatus,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.link.get_or_insert_with(Default::default).status = status;
        Outcome::new(receipt.affecting(EntityRef::Link))
    }

    /// `remote_synced`: applies the server's diff to the mirror (a snapshot over a cleared
    /// state) and records its revision. A diff not newer than the mirror is ignored.
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
        if snapshot {
            StateDiff::clear(&mut self.state);
        }
        diff.apply(&mut self.state);
        if snapshot || diff.touches_messages() {
            self.messages_changed();
        }
        Outcome::new(receipt.affecting(EntityRef::Link))
    }
}
