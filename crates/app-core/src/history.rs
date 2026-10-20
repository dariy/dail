//! Undo / redo history of `Mutation` commands.

use std::collections::VecDeque;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::Command;

/// One undoable step: the command as dispatched and the commands that revert it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HistoryEntry {
    /// Human label, e.g. "Set poll_secs".
    pub label: String,
    /// The command that was applied; re-applied on redo.
    pub forward: Command,
    /// Commands that revert `forward`, applied in order on undo.
    pub inverse: Vec<Command>,
    /// `Ctx::id_seed` of the original dispatch. Redo reuses it, so re-created entities get the
    /// same IDs as before the undo.
    pub id_seed: u128,
}

/// Bounded undo and redo stacks. Lives in the `Engine`, not in `AppState`: history is per session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

impl History {
    /// Records a new step, clears the redo stack and drops the oldest entries beyond `cap`.
    pub fn record(&mut self, entry: HistoryEntry, cap: usize) {
        self.redo.clear();
        self.push_undo(entry, cap);
    }

    /// Pushes onto the undo stack without touching the redo stack (used by redo).
    pub fn push_undo(&mut self, entry: HistoryEntry, cap: usize) {
        self.undo.push_back(entry);
        while self.undo.len() > cap.max(1) {
            self.undo.pop_front();
        }
    }

    /// Takes the most recent undoable step.
    pub fn pop_undo(&mut self) -> Option<HistoryEntry> {
        self.undo.pop_back()
    }

    /// Pushes onto the redo stack.
    pub fn push_redo(&mut self, entry: HistoryEntry) {
        self.redo.push(entry);
    }

    /// Takes the most recently undone step.
    pub fn pop_redo(&mut self) -> Option<HistoryEntry> {
        self.redo.pop()
    }

    /// Drops every undo and redo step for which `stale` holds (e.g. steps about a draft that has
    /// been sent: they can't be reverted any more).
    pub fn forget(&mut self, stale: impl Fn(&HistoryEntry) -> bool) {
        self.undo.retain(|entry| !stale(entry));
        self.redo.retain(|entry| !stale(entry));
    }

    /// Label of the step that `undo` would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|entry| entry.label.as_str())
    }

    /// Label of the step that `redo` would re-apply.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }

    /// Number of undoable steps.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Number of redoable steps.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}
