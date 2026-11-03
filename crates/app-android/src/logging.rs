//! `tracing` to `<profile>/logs/android.log` (user-only) and, on Android, to logcat through
//! logd's socket. `warn` and above in release builds, `info` in debug builds. Never secrets.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
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
        log_panics();
    });
}

/// Logs every panic, on any thread, to the outputs above before the default hook runs (STORE-4):
/// a panic on a runtime thread would otherwise leave no trace in `android.log`.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        tracing::error!(
            thread = thread.name().unwrap_or("unnamed"),
            location = info.location().map(ToString::to_string).unwrap_or_default(),
            panic = app_sync::executor::panic_text(info.payload()),
            "panic"
        );
        previous(info);
    }));
}

fn log_file(profile_dir: &Path) -> Option<Rotating> {
    let dir = app_io::ProfileDirs::at(profile_dir).logs_dir();
    std::fs::create_dir_all(&dir).ok()?;
    Rotating::open(dir.join("android.log"), ROTATE_BYTES).ok()
}

/// `android.log` is renamed to `android.log.1` (replacing the last one) once it passes this,
/// so the logs take at most about twice it (STORE-10).
pub const ROTATE_BYTES: u64 = 1024 * 1024;

/// An append-only log file that rotates once it passes `limit` bytes.
#[derive(Debug)]
pub struct Rotating {
    path: PathBuf,
    file: std::fs::File,
    written: u64,
    limit: u64,
}

impl Rotating {
    /// Opens `path` for appending (user-only on Unix), rotating it first if it is already full.
    pub fn open(path: PathBuf, limit: u64) -> std::io::Result<Self> {
        let file = open_append(&path)?;
        let written = file.metadata()?.len();
        let mut log = Self {
            path,
            file,
            written,
            limit,
        };
        if log.written >= limit {
            log.rotate()?;
        }
        Ok(log)
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let mut old = self.path.clone().into_os_string();
        old.push(".1");
        std::fs::rename(&self.path, &old)?;
        self.file = open_append(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl std::io::Write for Rotating {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.file.write(buf)?;
        self.written += n as u64;
        // `tracing` writes one whole event per call, so a rotation never splits a line.
        // A failed rotation keeps writing to the full file rather than lose the log (and cannot
        // log itself: this is the log).
        if self.written >= self.limit && self.rotate().is_err() {
            self.written = 0;
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
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
