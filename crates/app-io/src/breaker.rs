//! The crash-loop breaker (STORE-7). A message is marked before its body is parsed and unmarked
//! after. A mark left over at the next start means the process died while parsing that message
//! (a stack overflow or an abort cannot be caught): the message is then poisoned, its body is
//! never parsed again, and the engine hears it as a failed body. Without this, one hostile
//! message could crash the app at every sync.
//!
//! On disk, in the profile's `breaker/`: `parsing-<id>` for each body being parsed, and
//! `poisoned` with one poisoned ID per line.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const PARSING: &str = "parsing-";
const POISONED: &str = "poisoned";

/// What a poisoned message's body reports instead of being parsed.
pub const POISONED_REASON: &str =
    "dail stopped while reading this message, so it is not read again";

/// The marks of one profile.
#[derive(Debug)]
pub struct Breaker {
    dir: PathBuf,
    poisoned: Mutex<BTreeSet<String>>,
}

impl Breaker {
    /// Opens the breaker in `dir`, turning marks left by a dead process into poisoned IDs.
    /// Failures are logged and leave the breaker working in memory: it must never stop a start.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        if let Err(err) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %err, "breaker: no directory");
        }
        let mut poisoned: BTreeSet<String> = std::fs::read_to_string(dir.join(POISONED))
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        let mut found = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(id) = name.strip_prefix(PARSING) {
                    found.push(id.to_owned());
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        for id in found {
            tracing::error!(message = %id, "the last run died parsing this message; it is poisoned");
            poisoned.insert(id);
        }
        let breaker = Self {
            dir,
            poisoned: Mutex::new(poisoned),
        };
        breaker.save();
        breaker
    }

    /// Whether `id`'s body must not be parsed.
    pub fn is_poisoned(&self, id: &str) -> bool {
        self.poisoned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(id)
    }

    /// Marks `id` as being parsed until the guard drops. `id` must be a file-name-safe ID (a
    /// UUID).
    pub fn parsing(&self, id: &str) -> Guard {
        let path = self.dir.join(format!("{PARSING}{id}"));
        // The mark must be on disk before the parse starts, or a crash leaves no trace.
        let written = std::fs::File::create(&path).and_then(|file| file.sync_all());
        if let Err(err) = written {
            tracing::warn!(error = %err, "breaker: mark not written");
        }
        Guard { path }
    }

    fn save(&self) {
        let poisoned = self
            .poisoned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let path = self.dir.join(POISONED);
        let written = std::fs::File::create(&path).and_then(|mut file| {
            for id in poisoned.iter() {
                writeln!(file, "{id}")?;
            }
            file.sync_all()
        });
        if let Err(err) = written {
            tracing::warn!(error = %err, "breaker: poisoned list not written");
        }
    }

    /// The breaker's directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Removes its mark when dropped: the parse ended (or unwound, which is caught elsewhere).
#[derive(Debug)]
pub struct Guard {
    path: PathBuf,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
