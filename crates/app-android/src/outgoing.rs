//! Files picked or shared for sending (ADR 0009): each is copied into `<outgoing>/<random>/`
//! under a safe name before `attach_file` names the copy, because a content URI's permission
//! ends with the activity and the engine reads attachments by path. Copies that no draft names
//! are removed when the host opens.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use app_core::AppState;
use app_core::draft::MAX_ATTACHMENT_BYTES;
use app_sync::{RandomSeeds, SeedSource};

/// Why a file was not copied.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The copy would pass the 25 MiB that a draft may carry in all.
    #[error("“{0}” is larger than 25 MiB and cannot be attached")]
    TooLarge(String),
    /// Reading the source or writing the copy failed.
    #[error("“{name}” could not be copied: {source}")]
    Io {
        /// The file's name.
        name: String,
        /// Why.
        source: io::Error,
    },
}

/// Copies `source` (an open file such as Kotlin's `/proc/self/fd/<n>`) into a new folder of
/// `dir` as `safe_file_name(name)`; returns the copy's path. Nothing is left behind on failure.
pub fn import(dir: &Path, name: &str, source: &Path) -> Result<PathBuf, ImportError> {
    let file_name = app_io::mime::safe_file_name(Some(name), 1);
    let folder = dir.join(format!("{:032x}", RandomSeeds.next_seed()));
    let copied = copy(&folder, &file_name, source);
    if copied.is_err() {
        let _ = std::fs::remove_dir_all(&folder);
    }
    copied
}

fn copy(folder: &Path, file_name: &str, source: &Path) -> Result<PathBuf, ImportError> {
    let io = |source| ImportError::Io {
        name: file_name.to_owned(),
        source,
    };
    std::fs::create_dir_all(folder).map_err(io)?;
    let path = folder.join(file_name);
    let mut from = File::open(source)
        .map_err(io)?
        .take(MAX_ATTACHMENT_BYTES + 1);
    let mut to = File::create_new(&path).map_err(io)?;
    let size = io::copy(&mut from, &mut to).map_err(io)?;
    if size > MAX_ATTACHMENT_BYTES {
        return Err(ImportError::TooLarge(file_name.to_owned()));
    }
    to.flush().map_err(io)?;
    Ok(path)
}

/// Removes the folders of `dir` that no draft's attachment lies in. Returns how many it removed.
pub fn sweep(dir: &Path, state: &AppState) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let named: Vec<&Path> = state
        .drafts
        .values()
        .flat_map(|d| &d.attachments)
        .map(|a| Path::new(&a.path))
        .collect();
    let mut removed = 0;
    for entry in entries.flatten() {
        let folder = entry.path();
        if named.iter().any(|path| path.starts_with(&folder)) {
            continue;
        }
        let gone = if folder.is_dir() {
            std::fs::remove_dir_all(&folder)
        } else {
            std::fs::remove_file(&folder)
        };
        match gone {
            Ok(()) => removed += 1,
            Err(err) => tracing::warn!(error = %err, "an unused outgoing copy stays"),
        }
    }
    removed
}
