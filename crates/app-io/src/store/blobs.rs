//! Content-addressed blobs: `blobs/ab/cdef….eml`, named by the SHA-256 of the content.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::IoError;

/// The content address of `bytes`: lower-case hex SHA-256.
pub fn blob_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut id = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

fn is_blob_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A directory of content-addressed blobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobDir {
    root: PathBuf,
}

impl BlobDir {
    /// Blobs under `root` (created on the first write).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the blob `id` lives. Rejects anything but a hex SHA-256, so an ID can never name a
    /// path outside the directory.
    pub fn path(&self, id: &str) -> Result<PathBuf, IoError> {
        if !is_blob_id(id) {
            return Err(IoError::InvalidBlobId(id.to_owned()));
        }
        let (dir, file) = id.split_at(2);
        Ok(self.root.join(dir).join(format!("{file}.eml")))
    }

    /// Writes `bytes` (atomically, via a temporary file) unless already present.
    pub fn put(&self, bytes: &[u8]) -> Result<String, IoError> {
        let id = blob_id(bytes);
        let path = self.path(&id)?;
        if path.exists() {
            return Ok(id);
        }
        let Some(dir) = path.parent() else {
            return Err(IoError::InvalidBlobId(id));
        };
        std::fs::create_dir_all(dir).map_err(|err| IoError::fs(dir, err))?;
        // `<id>.eml.tmp` while written: `collect` removes leftovers.
        super::backup::write_atomic(&path, bytes)?;
        Ok(id)
    }

    /// Deletes the blobs (and leftover temporary files) not in `keep` that were last written
    /// before `written_before`.
    pub fn collect(
        &self,
        keep: &std::collections::BTreeSet<String>,
        written_before: std::time::SystemTime,
    ) -> Result<super::Compacted, IoError> {
        let mut done = super::Compacted::default();
        let dirs = match std::fs::read_dir(&self.root) {
            Ok(dirs) => dirs,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(done),
            Err(err) => return Err(IoError::fs(&self.root, err)),
        };
        for dir in dirs.flatten() {
            let prefix = dir.file_name().to_string_lossy().into_owned();
            let Ok(files) = std::fs::read_dir(dir.path()) else {
                continue;
            };
            for file in files.flatten() {
                let name = file.file_name().to_string_lossy().into_owned();
                let id = name
                    .strip_suffix(".eml")
                    .map(|stem| format!("{prefix}{stem}"));
                let stale = name.ends_with(".eml.tmp");
                let unused = id
                    .as_deref()
                    .is_some_and(|id| is_blob_id(id) && !keep.contains(id));
                if !(stale || unused) {
                    continue;
                }
                let Ok(meta) = file.metadata() else {
                    continue;
                };
                if meta.modified().is_ok_and(|at| at < written_before) {
                    let path = file.path();
                    std::fs::remove_file(&path).map_err(|err| IoError::fs(&path, err))?;
                    if unused {
                        done.removed_blobs += 1;
                    }
                    done.freed_bytes += meta.len();
                }
            }
        }
        Ok(done)
    }

    /// Reads a blob; `None` if missing.
    pub fn get(&self, id: &str) -> Result<Option<Vec<u8>>, IoError> {
        let path = self.path(id)?;
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(IoError::fs(&path, err)),
        }
    }
}
