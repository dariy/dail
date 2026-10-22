//! `tracing` to `<profile>/logs/android.log` (user-only) and, on Android, to logcat through
//! logd's socket. `warn` and above in release builds, `info` in debug builds. Never secrets.

use std::fs::OpenOptions;
use std::path::Path;
use std::sync::{Mutex, Once};

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{Layer, Registry, fmt};

/// The level of both outputs.
pub const LEVEL: LevelFilter = if cfg!(debug_assertions) {
    LevelFilter::INFO
} else {
    LevelFilter::WARN
};

static INIT: Once = Once::new();

/// Starts logging for the process, once; the first profile's log file wins. Failures leave
/// that output off.
pub fn init(profile_dir: &Path) {
    INIT.call_once(|| {
        let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();
        if let Some(file) = log_file(profile_dir) {
            layers.push(
                fmt::layer()
                    .with_ansi(false)
                    .with_writer(Mutex::new(file))
                    .with_filter(LEVEL)
                    .boxed(),
            );
        }
        #[cfg(target_os = "android")]
        if let Some(logcat) = logcat() {
            layers.push(logcat);
        }
        let registry = tracing_subscriber::registry().with(layers);
        // Another subscriber (a test's) may be set already.
        let _ = registry.try_init();
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
    options.open(dir.join("android.log")).ok()
}

#[cfg(target_os = "android")]
fn logcat() -> Option<Box<dyn Layer<Registry> + Send + Sync>> {
    use tracing_logcat::{LogcatMakeWriter, LogcatTag};
    let writer = LogcatMakeWriter::new(LogcatTag::Fixed("dail".to_owned())).ok()?;
    Some(
        fmt::layer()
            .event_format(fmt::format().with_level(false).without_time())
            .with_ansi(false)
            .with_writer(writer)
            .with_filter(LEVEL)
            .boxed(),
    )
}
