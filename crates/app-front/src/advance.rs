//! Auto-advance after triage (UX-12): when the open conversation is archived, trashed or
//! moved, the GUI and the TUI open its neighbour in the list or go back to the list, as the
//! `after_triage` setting says. Plain View commands, submitted right after the triage.

use app_core::{AppState, Command, ThreadId, listing};

/// The command that follows `cmd` (about to be submitted against `state`), if `cmd` takes the
/// open conversation out of the list: `open_thread` of the next (or previous) row, or
/// `close_thread`.
pub fn after(state: &AppState, cmd: &Command) -> Option<Command> {
    let open = state.view.thread?;
    let ids: &[ThreadId] = match cmd {
        Command::ArchiveThreads { ids, .. }
        | Command::TrashThreads { ids, .. }
        | Command::MoveThreads { ids, .. } => ids,
        _ => return None,
    };
    if !ids.contains(&open) {
        return None;
    }
    let rows = listing::visible_threads(state);
    let at = rows.iter().position(|id| *id == open)?;
    let remaining = |id: &&ThreadId| !ids.contains(id);
    let next = rows[at + 1..].iter().find(remaining);
    let previous = rows[..at].iter().rev().find(remaining);
    let target = match state.settings.after_triage {
        0 => next.or(previous),
        1 => previous.or(next),
        _ => None,
    };
    Some(match target {
        Some(id) => Command::OpenThread { id: *id },
        None => Command::CloseThread {},
    })
}
