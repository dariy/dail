//! `tracing` to `<profile>/logs/ios.log` (user-only), which `blob` can export: `os_log` would need
//! C (ADR 0010). `warn` and above in release builds, `info` in debug builds. Never secrets.

use std::fs::OpenOptions;
use std::path::Path;
use std::sync::{Mutex, Once};

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{Layer as _, fmt};

/// The level of the log file.
pub const LEVEL: LevelFilter = if cfg!(debug_assertions) {
    LevelFilter::INFO
} else {
    LevelFilter::WARN
};

static INIT: Once = Once::new();

/// Starts logging for the process, once; the first profile's log file wins. A failure leaves the
/// logs off.
pub fn init(profile_dir: &Path) {
    INIT.call_once(|| {
        let Some(file) = log_file(profile_dir) else {
            return;
        };
        let layer = fmt::layer()
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .with_filter(LEVEL);
        // Another subscriber (a test's) may be set already.
        let _ = tracing_subscriber::registry().with(layer).try_init();
    });
}

fn log_file(profile_dir: &Path) -> Option<std::fs::File> {
    let dir = app_io::ProfileDirs::at(profile_dir).logs_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(dir.join("ios.log")).ok()
}
