//! `save_attachments`: checked by the engine, written by the runtime, tracked in
//! `status.saves`. Only the user may save.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{Mail, ctx, mail};
use app_core::{BodyState, Command, CoreError, Effect, Event, MessageId, Origin, SaveState};

/// INBOX with two messages; the first one's body is downloaded.
fn downloaded() -> (Mail, MessageId, MessageId) {
    let mut m = Mail::hostinger();
    m.fetch(
        "INBOX",
        vec![
            mail(1, "a@x", None, "Report", true),
            mail(2, "b@x", None, "Lunch", true),
        ],
    );
    let (report, lunch) = (m.message("INBOX", 1), m.message("INBOX", 2));
    m.feed(Event::BodyFetched {
        message: report,
        blob: "ab".repeat(32),
        has_attachments: true,
        snippet: None,
    });
    (m, report, lunch)
}

fn save_to(message: MessageId, dir: &str) -> Command {
    Command::SaveAttachments {
        message,
        parts: vec![],
        dir: dir.to_owned(),
    }
}

#[test]
fn a_save_is_written_by_the_runtime_and_tracked() {
    let (mut m, report, _) = downloaded();
    let outcome = m.run(save_to(report, "/home/me/Downloads")).unwrap();
    assert_eq!(
        outcome.effects,
        [Effect::SaveAttachments {
            message: report,
            blob: "ab".repeat(32),
            parts: vec![],
            dir: "/home/me/Downloads".into(),
        }]
    );
    assert!(outcome.receipt.undo_label.is_none(), "not undoable");
    let saves = &m.engine.state().status.saves;
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].state, SaveState::Saving);

    m.feed(Event::AttachmentsSaved {
        message: report,
        files: vec!["/home/me/Downloads/q3.xlsx".into()],
    });
    let save = &m.engine.state().status.saves[0];
    assert_eq!(save.dir, "/home/me/Downloads");
    assert_eq!(
        save.state,
        SaveState::Saved {
            files: vec!["/home/me/Downloads/q3.xlsx".into()]
        }
    );

    // A failure (e.g. the effect could not run) is recorded too.
    m.run(save_to(report, "/media/usb")).unwrap();
    let failed = Effect::SaveAttachments {
        message: report,
        blob: String::new(),
        parts: vec![],
        dir: String::new(),
    }
    .failure_event("disk full".into());
    m.feed(failed);
    let save = &m.engine.state().status.saves[0];
    assert_eq!(save.dir, "/media/usb");
    assert!(
        matches!(&save.state, SaveState::Failed { error, files } if error == "disk full" && files.is_empty())
    );
}

#[test]
fn invalid_saves_are_rejected() {
    let (mut m, report, lunch) = downloaded();
    let unknown = MessageId(uuid::Uuid::from_u128(99));
    assert!(matches!(
        m.run(save_to(unknown, "/tmp")),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        m.engine.state().messages[&lunch].body,
        BodyState::NotFetched
    ));
    let not_fetched = m.run(save_to(lunch, "/tmp")).unwrap_err();
    assert!(matches!(&not_fetched, CoreError::InvalidArgument { field, .. } if field == "message"));
    for dir in ["Downloads", "", "/tmp/a\nb"] {
        let err = m.run(save_to(report, dir)).unwrap_err();
        assert!(
            matches!(&err, CoreError::InvalidArgument { field, .. } if field == "dir"),
            "{dir:?}"
        );
    }
    let many = Command::SaveAttachments {
        message: report,
        parts: (0..1001).collect(),
        dir: "/tmp".into(),
    };
    assert!(matches!(
        m.run(many),
        Err(CoreError::InvalidArgument { .. })
    ));
    assert!(m.engine.state().status.saves.is_empty());
}

#[test]
fn agents_cannot_write_files() {
    let (mut m, report, _) = downloaded();
    let agent = app_core::Ctx {
        origin: Origin::Mcp,
        ..ctx(50)
    };
    let err = m
        .engine
        .dispatch(save_to(report, "/home/me/.config/autostart"), &agent)
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }), "{err:?}");
    assert!(m.engine.state().status.saves.is_empty());
}

#[test]
fn a_new_save_replaces_the_messages_last_one() {
    let (mut m, report, _) = downloaded();
    for n in 0..20 {
        m.run(save_to(report, &format!("/tmp/{n}"))).unwrap();
    }
    let saves = &m.engine.state().status.saves;
    assert_eq!(saves.len(), 1, "one per message");
    assert_eq!(saves[0].dir, "/tmp/19");
}

#[test]
fn compaction_keeps_the_bodies_in_use() {
    let (mut m, _, _) = downloaded();
    let outcome = m.run(Command::CompactStore {}).unwrap();
    assert_eq!(
        outcome.effects,
        [Effect::CompactStore {
            keep: vec!["ab".repeat(32)]
        }]
    );
    assert!(matches!(
        m.engine.state().status.compaction,
        Some(app_core::Compaction::Running { .. })
    ));
    m.feed(Event::StoreCompacted {
        removed_blobs: 3,
        freed_bytes: 4096,
    });
    assert!(matches!(
        m.engine.state().status.compaction,
        Some(app_core::Compaction::Done {
            removed_blobs: 3,
            freed_bytes: 4096,
            ..
        })
    ));
    m.feed(Effect::CompactStore { keep: vec![] }.failure_event("disk gone".into()));
    assert!(matches!(
        &m.engine.state().status.compaction,
        Some(app_core::Compaction::Failed { error, .. }) if error == "disk gone"
    ));
}
