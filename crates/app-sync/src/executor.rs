//! Effect executor: turns one `Effect` into the `Event`s that report its result.

use std::time::Duration;

use app_core::{Effect, Event};

use crate::backend::Backend;
use crate::clock::Clock;

/// Runs one effect. Never fails: a failed effect becomes its failure event
/// (`Effect::failure_event`), so that failures surface in state.
pub async fn run_effect(backend: &dyn Backend, clock: &dyn Clock, effect: Effect) -> Vec<Event> {
    match effect {
        Effect::ScheduleTick { at_ms } => {
            let delay = at_ms.saturating_sub(clock.now_ms());
            if let Ok(delay) = u64::try_from(delay) {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            vec![Event::Tick {
                now_ms: clock.now_ms(),
            }]
        }
        effect => {
            let name = effect.name();
            match backend.execute(effect.clone()).await {
                Ok(events) => events,
                Err(err) => {
                    tracing::warn!(effect = name, error = %err, "effect failed");
                    vec![effect.failure_event(err.to_string())]
                }
            }
        }
    }
}
