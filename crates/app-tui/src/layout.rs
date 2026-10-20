//! The layout: which panes are on screen, as a function of `ViewState` and the terminal size.
//!
//! - ≥ 120 columns: the mailboxes (unless `toggle_sidebar` collapsed them), the list, and the
//!   reader or the composer.
//! - 80–119: the list and the reader (or composer); the mailboxes take the list's place while
//!   the sidebar has the focus.
//! - < 80: one pane: the mailboxes while the sidebar has the focus, else the composer, else the
//!   open thread, else the list.
//! - Below 40×12 nothing but a "terminal too small" notice.
//!
//! Borders only from 80 columns. A resize never dispatches a command.

use app_core::{AppState, Pane};
use ratatui::layout::{Constraint, Layout, Rect};

/// The smallest usable terminal.
pub const MIN_WIDTH: u16 = 40;
/// The smallest usable terminal.
pub const MIN_HEIGHT: u16 = 12;
/// From this width: three panes.
pub const WIDE: u16 = 120;
/// From this width: two panes, and borders.
pub const MEDIUM: u16 = 80;

/// What a pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Accounts and their mailboxes.
    Mailboxes,
    /// The thread list.
    List,
    /// The open thread.
    Reader,
    /// The draft in `view.composer`.
    Composer,
}

impl Slot {
    /// The engine's pane for this slot (for the focus).
    pub const fn pane(self) -> Pane {
        match self {
            Self::Mailboxes => Pane::Sidebar,
            Self::List => Pane::List,
            Self::Reader => Pane::Reader,
            Self::Composer => Pane::Composer,
        }
    }
}

/// The panes of one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrangement {
    /// Each pane and where it goes, left to right.
    pub panes: Vec<(Slot, Rect)>,
    /// Whether panes have borders (from [`MEDIUM`] columns).
    pub borders: bool,
}

/// Whether the composer is shown: `view.composer` names a draft that exists.
pub fn composer_open(state: &AppState) -> bool {
    state
        .view
        .composer
        .is_some_and(|id| state.drafts.contains_key(&id))
}

/// The panes for `area` (between the header and the status line); `None` if the terminal is
/// too small (`total` is the whole terminal).
pub fn arrange(total: Rect, area: Rect, state: &AppState) -> Option<Arrangement> {
    if total.width < MIN_WIDTH || total.height < MIN_HEIGHT {
        return None;
    }
    let view = &state.view;
    let detail = if composer_open(state) {
        Slot::Composer
    } else {
        Slot::Reader
    };
    let sidebar = view.focus == Pane::Sidebar;
    let split = |slots: &[(Slot, Constraint)]| -> Vec<(Slot, Rect)> {
        let rects = Layout::horizontal(slots.iter().map(|(_, c)| *c)).split(area);
        slots
            .iter()
            .map(|(s, _)| *s)
            .zip(rects.iter().copied())
            .collect()
    };
    let panes = if total.width >= WIDE && !view.sidebar_collapsed {
        split(&[
            (Slot::Mailboxes, Constraint::Length(26)),
            (Slot::List, Constraint::Percentage(40)),
            (detail, Constraint::Fill(1)),
        ])
    } else if total.width >= MEDIUM {
        let first = if sidebar { Slot::Mailboxes } else { Slot::List };
        split(&[
            (first, Constraint::Percentage(42)),
            (detail, Constraint::Fill(1)),
        ])
    } else {
        let only = if sidebar {
            Slot::Mailboxes
        } else if detail == Slot::Composer {
            Slot::Composer
        } else if view.thread.is_some_and(|t| state.threads.contains_key(&t)) {
            Slot::Reader
        } else {
            Slot::List
        };
        vec![(only, area)]
    };
    Some(Arrangement {
        panes,
        borders: total.width >= MEDIUM,
    })
}
