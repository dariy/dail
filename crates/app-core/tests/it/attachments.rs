//! `attach_file` / `remove_attachment` and the attachment checks reported by the runtime.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::Mail;
use app_core::draft::MAX_ATTACHMENT_BYTES;
use app_core::{AttachmentState, Command, CoreError, DraftId, Effect, Event};

fn draft() -> (Mail, DraftId) {
    let mut m = Mail::hostinger();
    let account = m.account;
    let outcome = m
        .run(Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Q3".into(),
            body: String::new(),
        })
        .unwrap();
    let id = serde_json::from_value(outcome.receipt.data["id"].clone()).unwrap();
    (m, id)
}

fn attach(m: &mut Mail, draft: DraftId, path: &str, index: Option<u32>) -> Vec<Effect> {
    m.run(Command::AttachFile {
        draft,
        path: path.into(),
        index,
    })
    .unwrap()
    .effects
}

fn inspected(m: &mut Mail, draft: DraftId, path: &str, size: u64) {
    m.feed(Event::AttachmentInspected {
        draft,
        path: path.into(),
        size,
        content_type: "application/pdf".into(),
    });
}

fn paths(m: &Mail, draft: DraftId) -> Vec<String> {
    m.engine.state().drafts[&draft]
        .attachments
        .iter()
        .map(|a| a.path.clone())
        .collect()
}

fn state(m: &Mail, draft: DraftId, index: usize) -> AttachmentState {
    m.engine.state().drafts[&draft].attachments[index]
        .state
        .clone()
}

#[test]
fn attach_file_records_the_path_and_asks_the_runtime_to_check_it() {
    let (mut m, id) = draft();
    let outcome = m
        .run(Command::AttachFile {
            draft: id,
            path: "/home/me/q3.pdf".into(),
            index: None,
        })
        .unwrap();
    assert_eq!(outcome.receipt.undo_label.as_deref(), Some("Attach file"));
    assert!(outcome.effects.iter().any(|e| matches!(
        e,
        Effect::InspectAttachment { draft, path } if *draft == id && path == "/home/me/q3.pdf"
    )));
    let attachment = &m.engine.state().drafts[&id].attachments[0];
    assert_eq!(attachment.name, "q3.pdf");
    assert_eq!(attachment.state, AttachmentState::Checking);
    assert_eq!(
        outcome.receipt.data["attachments"][0]["state"]["state"],
        "checking"
    );

    inspected(&mut m, id, "/home/me/q3.pdf", 1000);
    assert_eq!(
        state(&m, id, 0),
        AttachmentState::Ready {
            size: 1000,
            content_type: "application/pdf".into()
        }
    );
}

#[test]
fn a_missing_file_is_marked_failed() {
    let (mut m, id) = draft();
    attach(&mut m, id, "/tmp/gone.txt", None);
    m.feed(Event::AttachmentFailed {
        draft: id,
        path: "/tmp/gone.txt".into(),
        error: "cannot read /tmp/gone.txt: No such file or directory".into(),
    });
    assert!(matches!(
        state(&m, id, 0),
        AttachmentState::Failed { error } if error.contains("No such file")
    ));
}

#[test]
fn attachments_beyond_the_size_limit_fail() {
    let (mut m, id) = draft();
    attach(&mut m, id, "/a.bin", None);
    attach(&mut m, id, "/b.bin", None);
    attach(&mut m, id, "/huge.bin", None);
    inspected(&mut m, id, "/a.bin", MAX_ATTACHMENT_BYTES - 10);
    inspected(&mut m, id, "/b.bin", 10);
    assert!(
        matches!(state(&m, id, 1), AttachmentState::Ready { .. }),
        "fits exactly"
    );
    inspected(&mut m, id, "/huge.bin", 1);
    assert!(matches!(
        state(&m, id, 2),
        AttachmentState::Failed { error }
            if error == "huge.bin is 0.0 MB; all attachments together may have at most 25.0 MB"
    ));
}

#[test]
fn late_and_unknown_results_are_ignored() {
    let (mut m, id) = draft();
    attach(&mut m, id, "/a.pdf", None);
    inspected(&mut m, id, "/a.pdf", 5);
    // A second result for an attachment that is already checked changes nothing.
    m.feed(Event::AttachmentFailed {
        draft: id,
        path: "/a.pdf".into(),
        error: "late".into(),
    });
    assert!(matches!(
        state(&m, id, 0),
        AttachmentState::Ready { size: 5, .. }
    ));
    let outcome = m.feed(Event::AttachmentInspected {
        draft: DraftId::from_uuid(uuid::Uuid::from_u128(1)),
        path: "/a.pdf".into(),
        size: 1,
        content_type: "x/y".into(),
    });
    assert!(outcome.effects.is_empty());
}

#[test]
fn attach_file_rejects_bad_paths_duplicates_and_positions() {
    let (mut m, id) = draft();
    attach(&mut m, id, "/home/me/a.pdf", None);
    let cmd = |path: &str, index| Command::AttachFile {
        draft: id,
        path: path.into(),
        index,
    };
    for path in ["a.pdf", "./a.pdf", "", "/home/me/", "/tmp/a\nb"] {
        let err = m.run(cmd(path, None)).unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "path"),
            "{path:?}: {err:?}"
        );
    }
    let err = m.run(cmd("/home/me/a.pdf", None)).unwrap_err();
    assert!(matches!(err, CoreError::AlreadyExists { ref entity, .. } if entity == "attachment"));
    let err = m.run(cmd("/b.pdf", Some(2))).unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "index"));
    let missing = DraftId::from_uuid(uuid::Uuid::from_u128(3));
    let err = m
        .run(Command::AttachFile {
            draft: missing,
            path: "/b.pdf".into(),
            index: None,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "draft"));
    assert_eq!(paths(&m, id), ["/home/me/a.pdf"]);
}

#[test]
fn windows_paths_are_accepted() {
    let (mut m, id) = draft();
    attach(&mut m, id, "C:\\Users\\me\\Q3 report.xlsx", None);
    assert_eq!(
        m.engine.state().drafts[&id].attachments[0].name,
        "Q3 report.xlsx"
    );
}

#[test]
fn remove_attachment_undo_puts_it_back_at_its_position() {
    let (mut m, id) = draft();
    attach(&mut m, id, "/a", None);
    attach(&mut m, id, "/c", None);
    attach(&mut m, id, "/b", Some(1));
    assert_eq!(paths(&m, id), ["/a", "/b", "/c"]);
    let outcome = m
        .run(Command::RemoveAttachment {
            draft: id,
            index: 1,
        })
        .unwrap();
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Remove attachment")
    );
    assert_eq!(paths(&m, id), ["/a", "/c"]);
    let undo = m.run(Command::Undo {}).unwrap();
    assert_eq!(paths(&m, id), ["/a", "/b", "/c"]);
    // The file is checked again.
    assert!(
        undo.effects
            .iter()
            .any(|e| matches!(e, Effect::InspectAttachment { path, .. } if path == "/b"))
    );
    m.run(Command::Redo {}).unwrap();
    assert_eq!(paths(&m, id), ["/a", "/c"]);

    // Undoing the attach removes it again.
    m.run(Command::Undo {}).unwrap();
    m.run(Command::Undo {}).unwrap();
    assert_eq!(paths(&m, id), ["/a", "/c"]);

    let err = m
        .run(Command::RemoveAttachment {
            draft: id,
            index: 5,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "index"));
}

#[test]
fn a_restored_draft_has_its_attachments_checked_again() {
    let (mut m, id) = draft();
    attach(&mut m, id, "/home/me/q3.pdf", None);
    inspected(&mut m, id, "/home/me/q3.pdf", 1_000);
    assert!(matches!(state(&m, id, 0), AttachmentState::Ready { .. }));
    let inspects = |effects: &[Effect]| -> Vec<String> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::InspectAttachment { draft, path } if *draft == id => Some(path.clone()),
                _ => None,
            })
            .collect()
    };

    // Undo of a discard: the file may have changed meanwhile.
    m.run(Command::DiscardDraft { id }).unwrap();
    let undo = m.run(Command::Undo {}).unwrap();
    assert_eq!(state(&m, id, 0), AttachmentState::Checking);
    assert_eq!(inspects(&undo.effects), ["/home/me/q3.pdf"]);
    inspected(&mut m, id, "/home/me/q3.pdf", 1_000);

    // A check that comes with the draft is never trusted.
    let mut forged = m.engine.state().drafts[&id].clone();
    m.run(Command::DiscardDraft { id }).unwrap();
    forged.attachments[0].state = AttachmentState::Ready {
        size: 1,
        content_type: "text/plain".into(),
    };
    let restored = m
        .run(Command::RestoreDraft {
            draft: Box::new(forged.clone()),
        })
        .unwrap();
    assert_eq!(state(&m, id, 0), AttachmentState::Checking);
    assert_eq!(inspects(&restored.effects), ["/home/me/q3.pdf"]);

    // Paths are held to the rules of `attach_file`.
    m.run(Command::DiscardDraft { id }).unwrap();
    forged.attachments[0].path = "relative.pdf".into();
    let err = m
        .run(Command::RestoreDraft {
            draft: Box::new(forged),
        })
        .unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "path"),
        "{err:?}"
    );
}
