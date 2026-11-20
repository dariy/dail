//! Body pre-load around the selection (MISC-29): after a `View` navigation, the bodies of the
//! threads next to the selection (more of them in the direction of travel) or the first rows
//! of a newly opened list are fetched in the background, so that moving through the list does
//! not wait on the server. Bounded, none while metered, none over [`PREFETCH_MAX_BYTES`].

use crate::event::Effect;
use crate::ids::ThreadId;
use crate::message::BodyState;

use super::{Engine, PREFETCH_MAX_BYTES};

/// Threads pre-loaded ahead of the selection, in the direction of travel.
pub(super) const PRELOAD_AHEAD: usize = 3;
/// Threads pre-loaded behind it.
pub(super) const PRELOAD_BEHIND: usize = 1;
/// Messages one navigation pre-loads at most.
pub(super) const PRELOAD_MESSAGES: usize = 8;

impl Engine {
    /// Background `FetchBody` effects for the current selection and around it. `previous` is the
    /// selection before the navigation: travel is downwards unless the new row is above it.
    /// Without a selection, the first [`PRELOAD_AHEAD`] rows of the list.
    pub(super) fn preload_around(&mut self, previous: Option<ThreadId>) -> Vec<Effect> {
        if self.state.status.metered {
            return Vec::new();
        }
        let rows = crate::listing::visible_threads(&self.state);
        let current = self
            .state
            .view
            .thread
            .or_else(|| self.state.view.selected.first().copied());
        let threads: Vec<ThreadId> = match current.and_then(|c| rows.iter().position(|r| *r == c)) {
            None => rows.iter().take(PRELOAD_AHEAD).copied().collect(),
            Some(at) => {
                let was = previous.and_then(|p| rows.iter().position(|r| *r == p));
                let upwards = was.is_some_and(|was| was > at);
                let below = rows.iter().skip(at + 1);
                let above = rows[..at].iter().rev();
                let (ahead, behind): (Vec<_>, Vec<_>) = if upwards {
                    (
                        above.take(PRELOAD_AHEAD).collect(),
                        below.take(PRELOAD_BEHIND).collect(),
                    )
                } else {
                    (
                        below.take(PRELOAD_AHEAD).collect(),
                        above.take(PRELOAD_BEHIND).collect(),
                    )
                };
                // The selected row itself, then the nearest rows first, whichever side.
                let mut order = vec![rows[at]];
                let mut behind = behind.into_iter();
                for (i, t) in ahead.into_iter().enumerate() {
                    order.push(*t);
                    if i == 0
                        && let Some(b) = behind.next()
                    {
                        order.push(*b);
                    }
                }
                order.extend(behind.copied());
                order
            }
        };
        let mut effects = Vec::new();
        for thread in threads {
            let messages = self
                .state
                .threads
                .get(&thread)
                .map(|t| t.messages.clone())
                .unwrap_or_default();
            // Newest first: that is the message the reader shows open.
            for message in messages.into_iter().rev() {
                if effects.len() >= PRELOAD_MESSAGES {
                    return effects;
                }
                let wanted = self.state.messages.get(&message).is_some_and(|m| {
                    m.body == BodyState::NotFetched && m.size <= PREFETCH_MAX_BYTES
                });
                if wanted && let Some(fetch) = self.start_background_fetch(message) {
                    effects.push(fetch);
                }
            }
        }
        effects
    }
}
