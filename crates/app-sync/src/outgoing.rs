//! Rendering outgoing messages for `SaveDraft` and `SmtpSend`: the backend reads the quoted
//! original (a blob) and the attached files, [`render`] puts them together with
//! `app_io::compose`.

use app_core::OutgoingMessage;
use app_io::compose::{Bcc, FileData};

/// Renders `message`. `original` is the raw quoted message (if its blob was found); `files` are
/// the contents of `message.attachments`, in order (a file that could not be read fails the
/// whole render with its error).
pub fn render(
    message: &OutgoingMessage,
    original: Option<&[u8]>,
    files: Vec<Result<Vec<u8>, String>>,
    bcc: Bcc,
) -> Result<Vec<u8>, String> {
    let text = original.map(|raw| app_io::mime::parse_message(raw).text);
    let mut data = Vec::with_capacity(files.len());
    for (attachment, bytes) in message.attachments.iter().zip(files) {
        let content_type = match &attachment.state {
            app_core::AttachmentState::Ready { content_type, .. } => content_type.clone(),
            app_core::AttachmentState::Checking | app_core::AttachmentState::Failed { .. } => {
                app_io::mime::content_type_for(&attachment.name).to_owned()
            }
        };
        data.push(FileData {
            name: attachment.name.clone(),
            content_type,
            bytes: bytes?,
        });
    }
    app_io::compose::render(message, text.as_deref(), &data, bcc).map_err(|err| err.to_string())
}

/// Reads an attached file from the local disk.
pub async fn read_file(path: &str) -> Result<Vec<u8>, String> {
    tokio::fs::read(path)
        .await
        .map_err(|err| format!("cannot read {path}: {err}"))
}
