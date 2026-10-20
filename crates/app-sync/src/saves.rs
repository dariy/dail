//! `Effect::SaveAttachments` on the local disk: the attachments of a stored message written into
//! a folder, each under its safe file name (`app_io::mime::attachment_files`). An existing file
//! is never overwritten: the copy becomes `name (1).ext`, `name (2).ext`, …

use std::path::Path;

use app_core::{Event, MessageId};
use tokio::io::AsyncWriteExt;

/// Alternative names tried before giving up on a file.
const MAX_ALTERNATIVES: u32 = 999;

/// Writes the attachments of `raw` (all of them if `parts` is empty) into `dir` and reports the
/// outcome as an event. `raw` is `None` when the body is not in the store.
pub async fn save_attachments(
    message: MessageId,
    raw: Option<Vec<u8>>,
    parts: &[usize],
    dir: &str,
) -> Event {
    let mut files = Vec::new();
    let result = write_all(raw, parts, dir, &mut files).await;
    match result {
        Ok(()) => Event::AttachmentsSaved { message, files },
        Err(error) => Event::AttachmentsSaveFailed {
            message,
            files,
            error,
        },
    }
}

async fn write_all(
    raw: Option<Vec<u8>>,
    parts: &[usize],
    dir: &str,
    written: &mut Vec<String>,
) -> Result<(), String> {
    let raw = raw.ok_or_else(|| "the message is not in the local store any more".to_owned())?;
    let attachments = app_io::mime::attachment_files(&raw, parts)?;
    if attachments.is_empty() {
        return Err("the message has no attachments".to_owned());
    }
    let meta = tokio::fs::metadata(dir)
        .await
        .map_err(|err| format!("cannot open {dir}: {err}"))?;
    if !meta.is_dir() {
        return Err(format!("{dir} is not a folder"));
    }
    for (name, bytes) in attachments {
        written.push(write_new(Path::new(dir), &name, &bytes).await?);
    }
    Ok(())
}

/// Creates `dir/name` (or the first free `name (n).ext`) and writes `bytes` into it.
async fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> Result<String, String> {
    for n in 0..=MAX_ALTERNATIVES {
        let candidate = if n == 0 {
            name.to_owned()
        } else {
            app_io::mime::numbered_file_name(name, n)
        };
        let path = dir.join(&candidate);
        let shown = path.display().to_string();
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(mut file) => {
                file.write_all(bytes)
                    .await
                    .map_err(|err| format!("cannot write {shown}: {err}"))?;
                file.flush()
                    .await
                    .map_err(|err| format!("cannot write {shown}: {err}"))?;
                return Ok(shown);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(format!("cannot create {shown}: {err}")),
        }
    }
    Err(format!("{name}: every alternative name is taken"))
}
