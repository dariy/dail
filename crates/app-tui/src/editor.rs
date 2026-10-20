//! `$VISUAL` / `$EDITOR` for a draft's body. The text goes through a private temporary file: a
//! fresh directory only the user can enter (0700 on Unix) holding one file only the user can
//! read (0600), both removed afterwards. The runner leaves the alternate screen and stops reading
//! input while the editor runs.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The editor the user chose: `$VISUAL`, else `$EDITOR` (`None` if neither is set).
pub fn configured() -> Option<String> {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|v| v.to_string_lossy().trim().to_owned())
        .find(|v| !v.is_empty())
}

/// A private temporary file, removed with its directory on drop.
#[derive(Debug)]
pub struct PrivateFile {
    dir: PathBuf,
    path: PathBuf,
}

impl PrivateFile {
    /// Writes `text` to a new file in a new private directory under the system's temp dir.
    pub fn create(text: &str) -> io::Result<Self> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("dail-edit-{}-{stamp}", std::process::id()));
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt as _;
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        #[cfg(not(unix))]
        let builder = std::fs::DirBuilder::new();
        // `create`, not `create_all`: an existing directory (someone else's) is an error.
        builder.create(&dir)?;
        let file = Self {
            path: dir.join("draft.txt"),
            dir,
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut out = options.open(&file.path)?;
        out.write_all(text.as_bytes())?;
        out.sync_all()?;
        Ok(file)
    }

    /// The file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Its current content.
    pub fn read(&self) -> io::Result<String> {
        std::fs::read_to_string(&self.path)
    }
}

impl Drop for PrivateFile {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.dir) {
            tracing::warn!(error = %err, dir = %self.dir.display(), "could not remove the editor's file");
        }
    }
}

/// How editing went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edited {
    /// The editor exited successfully; the file's new text.
    Text(String),
    /// The editor failed (non-zero exit, or it could not start); the text is unchanged.
    Failed(String),
}

/// Runs `command` (`$VISUAL`/`$EDITOR`: a program and its arguments, split at whitespace) on a
/// private file holding `text`, waits for it, and returns the file's text.
pub fn edit(command: &str, text: &str) -> io::Result<Edited> {
    let file = PrivateFile::create(text)?;
    let mut words = command.split_whitespace();
    let Some(program) = words.next() else {
        return Ok(Edited::Failed("no editor".to_owned()));
    };
    let status = Command::new(program).args(words).arg(file.path()).status();
    Ok(match status {
        Ok(status) if status.success() => Edited::Text(file.read()?),
        Ok(status) => Edited::Failed(format!("{program} exited with {status}")),
        Err(err) => Edited::Failed(format!("{program}: {err}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_private_and_removed_afterwards() -> io::Result<()> {
        let file = PrivateFile::create("Hi Bob")?;
        let dir = file.dir.clone();
        assert_eq!(file.read()?, "Hi Bob");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(std::fs::metadata(&dir)?.permissions().mode() & 0o777, 0o700);
            assert_eq!(
                std::fs::metadata(file.path())?.permissions().mode() & 0o777,
                0o600
            );
        }
        drop(file);
        assert!(!dir.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn the_editors_text_comes_back_and_a_failure_is_reported() -> io::Result<()> {
        // `sed -i` stands in for an editor that changes the file in place.
        assert_eq!(
            edit("sed -i s/Hi/Hello/", "Hi Bob\n")?,
            Edited::Text("Hello Bob\n".to_owned())
        );
        assert!(matches!(edit("false", "x")?, Edited::Failed(_)));
        assert!(matches!(
            edit("/nonexistent/editor", "x")?,
            Edited::Failed(_)
        ));
        Ok(())
    }
}
