//! `app_sync::saves`: attachments written to a real folder, never over an existing file.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use app_core::{Event, MessageId};
use app_sync::saves::save_attachments;

const RAW: &[u8] = b"Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\
Content-Type: text/plain\r\n\r\nHi\r\n--b\r\n\
Content-Type: text/plain\r\nContent-Disposition: attachment; filename=\"../notes.txt\"\r\n\r\n\
new notes\r\n--b\r\n\
Content-Type: application/pdf\r\nContent-Disposition: attachment; filename=q3.pdf\r\n\
Content-Transfer-Encoding: base64\r\n\r\nJVBERi0=\r\n--b--\r\n";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-saves-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn message() -> MessageId {
    MessageId(uuid::Uuid::from_u128(1))
}

#[tokio::test]
async fn attachments_are_written_beside_existing_files() {
    let dir = scratch("write");
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    let event = save_attachments(message(), Some(RAW.to_vec()), &[], dir.to_str().unwrap()).await;
    let Event::AttachmentsSaved { files, .. } = event else {
        panic!("{event:?}")
    };
    let expected = [dir.join("notes (1).txt"), dir.join("q3.pdf")];
    let expected: Vec<String> = expected.iter().map(|p| p.display().to_string()).collect();
    assert_eq!(files, expected);
    assert_eq!(
        std::fs::read_to_string(dir.join("notes.txt")).unwrap(),
        "mine"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("notes (1).txt")).unwrap(),
        "new notes"
    );
    assert_eq!(std::fs::read(dir.join("q3.pdf")).unwrap(), b"%PDF-");
    assert!(
        !dir.parent().unwrap().join("notes.txt").exists(),
        "stays in the folder"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn failures_name_the_cause() {
    let dir = scratch("fail");
    let missing = dir.join("missing");
    let failed = |event: Event| match event {
        Event::AttachmentsSaveFailed { error, .. } => error,
        other => panic!("{other:?}"),
    };
    let error = failed(
        save_attachments(
            message(),
            Some(RAW.to_vec()),
            &[],
            missing.to_str().unwrap(),
        )
        .await,
    );
    assert!(error.contains("cannot open"), "{error}");
    let file = dir.join("file");
    std::fs::write(&file, "").unwrap();
    let error =
        failed(save_attachments(message(), Some(RAW.to_vec()), &[], file.to_str().unwrap()).await);
    assert!(error.contains("is not a folder"), "{error}");
    let error = failed(save_attachments(message(), None, &[], dir.to_str().unwrap()).await);
    assert!(error.contains("local store"), "{error}");
    let error =
        failed(save_attachments(message(), Some(RAW.to_vec()), &[0], dir.to_str().unwrap()).await);
    assert!(error.contains("not an attachment"), "{error}");
    let plain = b"Content-Type: text/plain\r\n\r\nHi\r\n".to_vec();
    let error = failed(save_attachments(message(), Some(plain), &[], dir.to_str().unwrap()).await);
    assert!(error.contains("no attachments"), "{error}");
    let _ = std::fs::remove_dir_all(&dir);
}
