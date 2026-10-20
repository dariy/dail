//! Time and ID-seed sources injected into `Ctx`. Tests use [`VirtualClock`] and [`CounterSeeds`].

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Source of `Ctx::now_ms`.
pub trait Clock: Send + Sync + 'static {
    /// Current time in Unix milliseconds.
    fn now_ms(&self) -> i64;
}

/// Wall-clock time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            })
    }
}

/// A clock that follows tokio's time source from a fixed start. Under a paused runtime
/// (`#[tokio::test(start_paused = true)]`, `tokio::time::advance`) it is fully virtual, and timers
/// scheduled by the executor fire in step with it.
#[derive(Debug, Clone, Copy)]
pub struct VirtualClock {
    start_ms: i64,
    origin: tokio::time::Instant,
}

impl VirtualClock {
    /// Starts at `start_ms` now. Must be called inside a tokio runtime.
    pub fn starting_at(start_ms: i64) -> Self {
        Self {
            start_ms,
            origin: tokio::time::Instant::now(),
        }
    }
}

impl Clock for VirtualClock {
    fn now_ms(&self) -> i64 {
        let elapsed = i64::try_from(self.origin.elapsed().as_millis()).unwrap_or(i64::MAX);
        self.start_ms.saturating_add(elapsed)
    }
}

/// Source of `Ctx::id_seed`.
pub trait SeedSource: Send + Sync + 'static {
    /// A fresh seed for one engine call.
    fn next_seed(&self) -> u128;
}

/// Random seeds (UUID v4).
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomSeeds;

impl SeedSource for RandomSeeds {
    fn next_seed(&self) -> u128 {
        uuid::Uuid::new_v4().as_u128()
    }
}

/// Deterministic seeds `start << 64`, `(start + 1) << 64`, … for tests and offline runs.
#[derive(Debug, Default)]
pub struct CounterSeeds {
    next: Mutex<u64>,
}

impl CounterSeeds {
    /// Starts counting at `start`.
    pub fn starting_at(start: u64) -> Self {
        Self {
            next: Mutex::new(start),
        }
    }
}

impl SeedSource for CounterSeeds {
    fn next_seed(&self) -> u128 {
        let mut next = match self.next.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let seed = u128::from(*next) << 64;
        *next = next.wrapping_add(1);
        seed
    }
}
