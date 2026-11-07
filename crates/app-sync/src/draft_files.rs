//! The attachments `edit_draft` carries from a server draft (MISC-44d): written under
//! `<dir>/<draft id>/` with their safe names (`app_io::mime::draft_attachments`), so the local
//! draft names them by path like any attached file. Folders that no draft names any more are
//! removed by [`sweep`].

use std::path::Path;

use app_core::draft::CarriedAttachment;
use app_core::{AppState, DraftId};

/// Why the attachments of a server draft were not carried into the local draft. Reported as
/// the text of `DraftReadFailed`.
#[derive(Debug, thiserror::Error)]
pub enum DraftFilesError {
    /// The backend has no `draft-files` folder, and the draft has attachments.
    #[error("ERR208: there is no folder for the draft's attachments")]
    NoFolder,
    /// Creating the folder or a file failed.
    #[error("ERR209: the draft's attachments could not be written: {0}")]
    Write(String),
}

/// Writes the attachments of `raw` into `dir/<draft>/` and describes the copies. Nothing is
/// written when the draft has none; a failure removes what was written.
pub async fn write(
    dir: &Path,
    draft: DraftId,
    raw: &[u8],
) -> Result<Vec<CarriedAttachment>, DraftFilesError> {
    let files = app_io::mime::draft_attachments(raw);
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let folder = dir.join(draft.as_uuid().to_string());
    let written = write_all(&folder, files).await;
    if written.is_err() {
        let _ = tokio::fs::remove_dir_all(&folder).await;
    }
    written.map_err(DraftFilesError::Write)
}

async fn write_all(
    folder: &Path,
    files: Vec<(String, Vec<u8>)>,
) -> Result<Vec<CarriedAttachment>, String> {
    tokio::fs::create_dir_all(folder)
        .await
        .map_err(|err| format!("cannot create {}: {err}", folder.display()))?;
    let mut carried = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        let path = crate::saves::write_new(folder, &name, &bytes).await?;
        let name = app_core::draft::file_name(&path).to_owned();
        carried.push(CarriedAttachment {
            content_type: app_io::mime::content_type_for(&name).to_owned(),
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            name,
            path,
        });
    }
    Ok(carried)
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
        if !folder.is_dir() || named.iter().any(|path| path.starts_with(&folder)) {
            continue;
        }
        match std::fs::remove_dir_all(&folder) {
            Ok(()) => removed += 1,
            Err(err) => tracing::warn!(error = %err, "an unused draft attachment folder stays"),
        }
    }
    removed
}
