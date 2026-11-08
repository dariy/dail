//! `app_sync::draft_files`: the attachments of a server draft written under the draft's folder
//! with safe names, and folders no draft names swept (MISC-44d).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::draft::{AttachmentRef, AttachmentState, Draft};
use app_core::{AccountId, AppState, DraftId};
use app_sync::draft_files::{sweep, write};

const RAW: &[u8] = b"Content-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\n\
Content-Type: text/plain\r\n\r\nHi\r\n--b\r\n\
Content-Type: text/plain\r\nContent-Disposition: attachment; filename=\"../.notes.txt\"\r\n\r\n\
new notes\r\n--b\r\n\
Content-Type: application/pdf\r\nContent-Disposition: attachment; filename=q3.pdf\r\n\
Content-Transfer-Encoding: base64\r\n\r\nJVBERi0=\r\n--b--\r\n";

#[tokio::test]
async fn attachments_are_written_and_unused_folders_swept() {
    let dir = std::env::temp_dir().join(format!("dail-draft-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let draft = DraftId(uuid::Uuid::from_u128(7));
    let carried = write(&dir, draft, RAW).await.unwrap();
    let names: Vec<&str> = carried.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["notes.txt", "q3.pdf"]);
    let folder = dir.join(draft.as_uuid().to_string());
    assert_eq!(carried[1].path, folder.join("q3.pdf").display().to_string());
    assert_eq!(carried[1].size, 5);
    assert_eq!(carried[1].content_type, "application/pdf");
    assert_eq!(std::fs::read(folder.join("q3.pdf")).unwrap(), b"%PDF-");
    assert!(
        write(
            &dir,
            DraftId(uuid::Uuid::from_u128(8)),
            b"Subject: none\r\n\r\nx"
        )
        .await
        .unwrap()
        .is_empty()
    );

    // A folder a draft names stays; any other goes.
    std::fs::create_dir_all(dir.join("stale")).unwrap();
    let mut state = AppState::default();
    let mut local: Draft = serde_json::from_value(serde_json::json!({
        "id": draft, "account": AccountId(uuid::Uuid::from_u128(1)),
        "message_id_hdr": "x@y", "created_ms": 0, "updated_ms": 0
    }))
    .unwrap();
    local.attachments = vec![AttachmentRef {
        path: carried[1].path.clone(),
        name: "q3.pdf".into(),
        state: AttachmentState::Checking,
    }];
    state.drafts.insert(draft, local);
    assert_eq!(sweep(&dir, &state), 1);
    assert!(folder.exists());
    assert_eq!(sweep(&dir, &AppState::default()), 1);
    assert!(!folder.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
