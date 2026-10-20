//! Crash safety for the profile's files: durable atomic writes, daily backups, and recovery of a
//! cache that no longer opens.
//!
//! - [`write_atomic`]: a temporary file is written and flushed to disk, renamed over the target,
//!   and the directory is flushed (Unix), so a crash leaves the old or the new file, never a
//!   torn one. `config.toml` and the blobs use it; `state.redb` commits are durable by
//!   themselves (redb).
//! - [`backup_if_due`]: when the profile is opened (under its lock, so nothing writes the files),
//!   `config.toml` and `state.redb` are copied into `backups/<UTC time>/` if the newest backup
//!   is a day old; the newest [`KEEP`] stay. Blobs are not copied: they can be downloaded again.
//! - [`recover`]: a `state.redb` that does not open is renamed to `state.redb.broken-<time>`
//!   and the newest backup's copy is put in its place (or none, and the cache starts empty
//!   and fills again from the servers). `config.toml` is the user's to edit and is never
//!   replaced; `dail doctor` lists the backups to restore it from.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::IoError;
use crate::profile::ProfileDirs;

/// How old the newest backup may get before the next is made.
pub const BACKUP_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Backups kept.
pub const KEEP: usize = 7;
/// The files a backup holds (in the profile directory).
pub const FILES: [&str; 2] = ["config.toml", "state.redb"];

/// Writes `bytes` to `path` durably and atomically.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), IoError> {
    use std::io::Write as _;
    let tmp = sibling(path, ".tmp");
    let written = std::fs::File::create(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(err) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(IoError::fs(&tmp, err));
    }
    std::fs::rename(&tmp, path).map_err(|err| {
        let _ = std::fs::remove_file(&tmp);
        IoError::fs(path, err)
    })?;
    sync_dir(path.parent());
    Ok(())
}

/// Flushes a directory's entries (a rename) to disk. Only Unix can open a directory for it.
fn sync_dir(dir: Option<&Path>) {
    #[cfg(unix)]
    if let Some(dir) = dir
        && let Ok(handle) = std::fs::File::open(dir)
    {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// `20261020T081500Z`: a UTC time that sorts like the time.
pub fn stamp(at: SystemTime) -> String {
    let secs = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let secs = i64::try_from(secs).unwrap_or(i64::MAX);
    let (year, month, day) = app_core::date::civil_from_days(secs.div_euclid(86_400));
    let in_day = secs.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

/// One backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// Its directory.
    pub path: PathBuf,
    /// Its name: the UTC time it was made (`20261020T081500Z`).
    pub name: String,
}

/// The backups of the profile, newest first.
pub fn list(dirs: &ProfileDirs) -> Result<Vec<Backup>, IoError> {
    let dir = dirs.backups_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(IoError::fs(&dir, err)),
    };
    let mut backups: Vec<Backup> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            (name.len() == 16 && name.ends_with('Z')).then(|| Backup {
                path: e.path(),
                name,
            })
        })
        .collect();
    backups.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(backups)
}

/// Makes a backup if the newest is older than [`BACKUP_EVERY`] (by its name) and there is
/// something to back up; keeps the newest [`KEEP`]. Returns the new backup.
pub fn backup_if_due(dirs: &ProfileDirs, now: SystemTime) -> Result<Option<Backup>, IoError> {
    let backups = list(dirs)?;
    let due_after = stamp(
        now.checked_sub(BACKUP_EVERY)
            .unwrap_or(SystemTime::UNIX_EPOCH),
    );
    if backups
        .first()
        .is_some_and(|newest| newest.name > due_after)
    {
        return Ok(None);
    }
    let present: Vec<&str> = FILES
        .into_iter()
        .filter(|f| dirs.root().join(f).is_file())
        .collect();
    if present.is_empty() {
        return Ok(None);
    }
    let name = stamp(now);
    let root = dirs.backups_dir();
    let tmp = root.join(format!(".{name}.tmp"));
    std::fs::create_dir_all(&tmp).map_err(|err| IoError::fs(&tmp, err))?;
    for file in present {
        let to = tmp.join(file);
        let copied = std::fs::copy(dirs.root().join(file), &to)
            .and_then(|_| std::fs::File::open(&to).and_then(|f| f.sync_all()));
        if let Err(err) = copied {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(IoError::fs(&to, err));
        }
    }
    let path = root.join(&name);
    std::fs::rename(&tmp, &path).map_err(|err| {
        let _ = std::fs::remove_dir_all(&tmp);
        IoError::fs(&path, err)
    })?;
    sync_dir(Some(&root));
    for old in list(dirs)?.into_iter().skip(KEEP) {
        if let Err(err) = std::fs::remove_dir_all(&old.path) {
            tracing::warn!(backup = %old.path.display(), error = %err, "old backup not removed");
        }
    }
    Ok(Some(Backup { path, name }))
}

/// What [`recover`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovery {
    /// Where the file that did not open was moved.
    pub broken: PathBuf,
    /// The backup put in its place, if there was one.
    pub restored_from: Option<Backup>,
}

/// Moves `file` (in the profile directory) aside as `<file>.broken-<time>` and copies the
/// newest backup's copy in its place, if a backup has one.
pub fn recover(dirs: &ProfileDirs, file: &str, now: SystemTime) -> Result<Recovery, IoError> {
    let path = dirs.root().join(file);
    let broken = sibling(&path, &format!(".broken-{}", stamp(now)));
    std::fs::rename(&path, &broken).map_err(|err| IoError::fs(&path, err))?;
    let backup = list(dirs)?
        .into_iter()
        .find(|b| b.path.join(file).is_file());
    if let Some(backup) = &backup {
        let bytes = std::fs::read(backup.path.join(file))
            .map_err(|err| IoError::fs(&backup.path.join(file), err))?;
        write_atomic(&path, &bytes)?;
    }
    Ok(Recovery {
        broken,
        restored_from: backup,
    })
}

/// Files set aside by [`recover`] that are still in the profile directory.
pub fn broken_files(dirs: &ProfileDirs) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dirs.root()) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".broken-"))
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn stamps_sort_like_time() {
        assert_eq!(stamp(at(0)), "19700101T000000Z");
        assert_eq!(stamp(at(1_700_000_000)), "20231114T221320Z");
    }

    #[test]
    fn daily_backups_keep_the_newest_and_recovery_uses_them() {
        let root = std::env::temp_dir().join(format!("dail-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dirs = ProfileDirs::at(&root);
        assert_eq!(
            backup_if_due(&dirs, at(1_000_000)).unwrap(),
            None,
            "nothing yet"
        );
        write_atomic(&dirs.config_file(), b"[settings]\n").unwrap();
        write_atomic(&dirs.state_db(), b"v1").unwrap();
        let first = backup_if_due(&dirs, at(1_000_000)).unwrap().unwrap();
        assert_eq!(std::fs::read(first.path.join("state.redb")).unwrap(), b"v1");
        assert_eq!(
            backup_if_due(&dirs, at(1_000_000 + 3600)).unwrap(),
            None,
            "not due"
        );
        for day in 1..=9 {
            write_atomic(&dirs.state_db(), format!("v{}", day + 1).as_bytes()).unwrap();
            assert!(
                backup_if_due(&dirs, at(1_000_000 + day * 86_400))
                    .unwrap()
                    .is_some()
            );
        }
        let backups = list(&dirs).unwrap();
        assert_eq!(backups.len(), KEEP);
        assert_eq!(
            std::fs::read(backups[0].path.join("state.redb")).unwrap(),
            b"v10"
        );
        // A cache that does not open: set aside, the newest backup in its place.
        write_atomic(&dirs.state_db(), b"garbage").unwrap();
        let recovery = recover(&dirs, "state.redb", at(2_000_000)).unwrap();
        assert_eq!(std::fs::read(&recovery.broken).unwrap(), b"garbage");
        assert_eq!(recovery.restored_from.as_ref(), Some(&backups[0]));
        assert_eq!(std::fs::read(dirs.state_db()).unwrap(), b"v10");
        assert_eq!(broken_files(&dirs), [recovery.broken]);
        assert!(
            !root.join("state.redb.tmp").exists(),
            "no temporary file left"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
