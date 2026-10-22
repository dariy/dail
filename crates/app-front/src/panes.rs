//! Which panes are on screen, as a function of `ViewState` and the window's width class. The
//! TUI maps terminal columns onto a [`Width`] and lays the panes out in `Rect`s; the mobile
//! frames map window classes onto it.
//!
//! - [`Width::Wide`]: the mailboxes (unless `toggle_sidebar` collapsed them), the list, and the
//!   reader or the composer.
//! - [`Width::Medium`]: the list and the reader (or composer); the mailboxes take the list's
//!   place while the sidebar has the focus.
//! - [`Width::Narrow`]: one pane: the mailboxes while the sidebar has the focus, else the
//!   composer, else the open thread, else the list.

use app_core::{AppState, Pane};

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

/// How wide the window is, in the classes the pane choice knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// Room for three panes.
    Wide,
    /// Room for two.
    Medium,
    /// One pane.
    Narrow,
}

/// Whether the composer is shown: `view.composer` names a draft that exists.
pub fn composer_open(state: &AppState) -> bool {
    state
        .view
        .composer
        .is_some_and(|id| state.drafts.contains_key(&id))
}

/// The panes on screen, left to right.
pub fn choose(width: Width, state: &AppState) -> Vec<Slot> {
    arrange(width, state, state.view.focus == Pane::Sidebar)
}

/// The panes under a modal drawer: [`choose`] as if the sidebar did not have the focus (the
/// mobile frames draw the mailboxes over them on compact and medium windows).
pub fn behind_drawer(width: Width, state: &AppState) -> Vec<Slot> {
    arrange(width, state, false)
}

fn arrange(width: Width, state: &AppState, sidebar: bool) -> Vec<Slot> {
    let view = &state.view;
    let detail = if composer_open(state) {
        Slot::Composer
    } else {
        Slot::Reader
    };
    match width {
        Width::Wide if !view.sidebar_collapsed => vec![Slot::Mailboxes, Slot::List, detail],
        Width::Wide | Width::Medium => {
            let first = if sidebar { Slot::Mailboxes } else { Slot::List };
            vec![first, detail]
        }
        Width::Narrow => {
            let only = if sidebar {
                Slot::Mailboxes
            } else if detail == Slot::Composer {
                Slot::Composer
            } else if view.thread.is_some_and(|t| state.threads.contains_key(&t)) {
                Slot::Reader
            } else {
                Slot::List
            };
            vec![only]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_follow_the_width_and_the_focus() {
        let mut state = AppState::default();
        assert_eq!(
            choose(Width::Wide, &state),
            [Slot::Mailboxes, Slot::List, Slot::Reader]
        );
        assert_eq!(choose(Width::Medium, &state), [Slot::List, Slot::Reader]);
        assert_eq!(choose(Width::Narrow, &state), [Slot::List]);
        state.view.sidebar_collapsed = true;
        assert_eq!(choose(Width::Wide, &state), [Slot::List, Slot::Reader]);
        state.view.focus = Pane::Sidebar;
        assert_eq!(
            choose(Width::Medium, &state),
            [Slot::Mailboxes, Slot::Reader]
        );
        assert_eq!(choose(Width::Narrow, &state), [Slot::Mailboxes]);
        assert_eq!(behind_drawer(Width::Narrow, &state), [Slot::List]);
        assert_eq!(
            behind_drawer(Width::Medium, &state),
            [Slot::List, Slot::Reader]
        );
    }
}
