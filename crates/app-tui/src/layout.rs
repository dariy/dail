//! The layout: which panes are on screen, as a function of `ViewState` and the terminal size
//! (the choice is `app_front::panes`; this module maps columns onto its width classes).
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

use app_core::AppState;
use app_front::panes::{self, Width};
pub use app_front::panes::{Slot, composer_open};
use ratatui::layout::{Constraint, Layout, Rect};

/// The smallest usable terminal.
pub const MIN_WIDTH: u16 = 40;
/// The smallest usable terminal.
pub const MIN_HEIGHT: u16 = 12;
/// From this width: three panes.
pub const WIDE: u16 = 120;
/// From this width: two panes, and borders.
pub const MEDIUM: u16 = 80;

/// The panes of one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrangement {
    /// Each pane and where it goes, left to right.
    pub panes: Vec<(Slot, Rect)>,
    /// Whether panes have borders (from [`MEDIUM`] columns).
    pub borders: bool,
}

/// The panes for `area` (between the header and the status line); `None` if the terminal is
/// too small (`total` is the whole terminal). Which panes show is [`panes::choose`]'s rule.
pub fn arrange(total: Rect, area: Rect, state: &AppState) -> Option<Arrangement> {
    if total.width < MIN_WIDTH || total.height < MIN_HEIGHT {
        return None;
    }
    let width = if total.width >= WIDE {
        Width::Wide
    } else if total.width >= MEDIUM {
        Width::Medium
    } else {
        Width::Narrow
    };
    let slots = panes::choose(width, state);
    let constraints: &[Constraint] = match slots.len() {
        3 => &[
            Constraint::Length(26),
            Constraint::Percentage(40),
            Constraint::Fill(1),
        ],
        2 => &[Constraint::Percentage(42), Constraint::Fill(1)],
        _ => &[Constraint::Fill(1)],
    };
    let rects = Layout::horizontal(constraints.iter().copied()).split(area);
    Some(Arrangement {
        panes: slots.into_iter().zip(rects.iter().copied()).collect(),
        borders: total.width >= MEDIUM,
    })
}
