//! "Open original" (`ARCHITECTURE.md §7.1`): the sanitized page of a stored message
//! ([`app_io::original`]) written to a private temporary file for the system browser. Opening an
//! attachment (a PDF) writes a private copy the same way.

use std::path::PathBuf;

use crate::error::SyncError;

/// Directory (mode 0700 on Unix) that holds the pages and attachment copies.
fn page_dir() -> PathBuf {
    std::env::temp_dir().join("dail-originals")
}

fn fs_error(path: &std::path::Path, source: std::io::Error) -> SyncError {
    SyncError::Store(app_io::IoError::Fs {
        path: path.display().to_string(),
        source,
    })
}

/// Writes `html` to a new file (mode 0600 on Unix) in the private temp directory.
pub async fn write_page(html: String) -> Result<PathBuf, SyncError> {
    write_private(&format!("{}.html", uuid::Uuid::new_v4()), html.as_bytes()).await
}

/// Writes an attachment to a new file (mode 0600 on Unix) in the private temp directory, as
/// `<uuid>-<name>` (`name` must be safe, `app_io::mime::safe_file_name`).
pub async fn write_copy(name: &str, bytes: &[u8]) -> Result<PathBuf, SyncError> {
    write_private(&format!("{}-{name}", uuid::Uuid::new_v4()), bytes).await
}

async fn write_private(file_name: &str, bytes: &[u8]) -> Result<PathBuf, SyncError> {
    let dir = page_dir();
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(&dir).await.map_err(|e| fs_error(&dir, e))?;
    let path = dir.join(file_name);
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path).await.map_err(|e| fs_error(&path, e))?;
    // tokio writes in the background: flush before handing the path to a browser.
    tokio::io::AsyncWriteExt::write_all(&mut file, bytes)
        .await
        .map_err(|e| fs_error(&path, e))?;
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| fs_error(&path, e))?;
    Ok(path)
}
