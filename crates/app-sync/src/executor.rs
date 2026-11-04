//! Effect executor: turns one `Effect` into the `Event`s that report its result.

use std::time::Duration;

use app_core::{Effect, Event};

use crate::SyncError;
use crate::backend::Backend;
use crate::clock::Clock;

/// The reason of an effect that panicked; the panic's own text goes to the log only.
pub const PANICKED: &str = "dail hit an internal error; the log has the details";

/// A panic payload's text, when it has one.
pub fn panic_text(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(no message)")
}

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
            // A panic in a backend is this effect's failure, not its lane's end (STORE-4): the
            // worker keeps running and the engine hears about it like any other failure.
            let run = std::panic::AssertUnwindSafe(backend.execute(effect.clone()));
            let result = match futures_util::FutureExt::catch_unwind(run).await {
                Ok(result) => result,
                Err(panic) => {
                    tracing::error!(effect = name, panic = %panic_text(panic.as_ref()), "effect panicked");
                    Err(SyncError::Effect {
                        effect: name,
                        reason: PANICKED.to_owned(),
                    })
                }
            };
            match result {
                Ok(events) => events,
                Err(err) => {
                    tracing::warn!(effect = name, error = %err, "effect failed");
                    vec![effect.failure_event(err.to_string())]
                }
            }
        }
    }
}
