//! `save_attachments`: writing a downloaded message's attachments into a local folder; and
//! `compact_store`, which hands the runtime the bodies still in use. The
//! engine checks the request and records it in `status.saves`; the runtime writes the files
//! (`Effect::SaveAttachments`) and reports `AttachmentsSaved` / `AttachmentsSaveFailed`.

use crate::ctx::Ctx;
use crate::draft::is_absolute_path;
use crate::error::CoreError;
use crate::event::Effect;
use crate::ids::MessageId;
use crate::message::BodyState;
use crate::receipt::{CommandReceipt, EntityRef, Outcome};
use crate::state::{AttachmentSave, SaveState};

use super::{Engine, invalid, not_found};

/// Saves kept in `status.saves`; the oldest are forgotten.
pub const MAX_SAVES: usize = 16;
/// The most parts one save may name.
const MAX_PARTS: usize = 1000;

impl Engine {
    pub(super) fn save_attachments(
        &mut self,
        message: MessageId,
        parts: Vec<usize>,
        dir: String,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Result<Outcome, CoreError> {
        let envelope = self
            .state
            .messages
            .get(&message)
            .ok_or_else(|| not_found("message", message))?;
        let BodyState::Fetched { blob } = &envelope.body else {
            return Err(invalid(
                "message",
                "the body is not downloaded yet (open_thread downloads it)",
            ));
        };
        if !is_absolute_path(&dir) || dir.chars().any(char::is_control) {
            return Err(invalid("dir", "expected the absolute path of a folder"));
        }
        if parts.len() > MAX_PARTS {
            return Err(invalid("parts", format!("at most {MAX_PARTS} parts")));
        }
        let effect = Effect::SaveAttachments {
            message,
            blob: blob.clone(),
            parts: parts.clone(),
            dir: dir.clone(),
        };
        self.record_save(message, &dir, SaveState::Saving, ctx);
        Ok(Outcome::new(
            receipt
                .affecting(EntityRef::Message(message))
                .affecting(EntityRef::Status)
                .with_data(serde_json::json!({ "message": message, "parts": parts, "dir": dir })),
        )
        .with_effect(effect))
    }

    pub(super) fn ingest_saved(
        &mut self,
        message: MessageId,
        state: SaveState,
        ctx: &Ctx,
        receipt: CommandReceipt,
    ) -> Outcome {
        let dir = self
            .state
            .status
            .saves
            .iter()
            .find(|s| s.message == message)
            .map(|s| s.dir.clone())
            .unwrap_or_default();
        self.record_save(message, &dir, state, ctx);
        Outcome::new(
            receipt
                .affecting(EntityRef::Message(message))
                .affecting(EntityRef::Status),
        )
    }

    /// Puts the message's save last in `status.saves`, replacing an earlier one.
    fn record_save(&mut self, message: MessageId, dir: &str, state: SaveState, ctx: &Ctx) {
        let saves = &mut self.state.status.saves;
        saves.retain(|s| s.message != message);
        saves.push(AttachmentSave {
            message,
            dir: dir.to_owned(),
            state,
            updated_ms: ctx.now_ms,
        });
        if saves.len() > MAX_SAVES {
            let excess = saves.len() - MAX_SAVES;
            saves.drain(..excess);
        }
    }
}

impl Engine {
    /// `compact_store`: hands the runtime every body a cached message refers to.
    pub(super) fn compact_store(&mut self, ctx: &Ctx, receipt: CommandReceipt) -> Outcome {
        let keep: std::collections::BTreeSet<String> = self
            .state
            .messages
            .values()
            .filter_map(|m| match &m.body {
                BodyState::Fetched { blob } => Some(blob.clone()),
                _ => None,
            })
            .collect();
        self.state.status.compaction = Some(crate::state::Compaction::Running {
            started_ms: ctx.now_ms,
        });
        let kept = keep.len();
        Outcome::new(
            receipt
                .affecting(EntityRef::Status)
                .with_data(serde_json::json!({ "bodies_kept": kept })),
        )
        .with_effect(Effect::CompactStore {
            keep: keep.into_iter().collect(),
        })
    }

    pub(super) fn ingest_compacted(
        &mut self,
        compaction: crate::state::Compaction,
        receipt: CommandReceipt,
    ) -> Outcome {
        self.state.status.compaction = Some(compaction);
        Outcome::new(receipt.affecting(EntityRef::Status))
    }
}
