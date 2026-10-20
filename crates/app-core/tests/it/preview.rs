//! Dry runs: `Engine::preview` reports the receipt, the effects by name and the changed fields,
//! and changes nothing.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, ctx, mail};
use app_core::{Command, CoreError, EntityRef, Origin, ProviderPreset};

fn inbox_mail() -> Mail {
    let mut m = Mail::hostinger();
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Plans", false)]);
    m
}

#[test]
fn a_preview_reports_what_would_change_and_changes_nothing() {
    let m = inbox_mail();
    let message = m.message("INBOX", 1);
    let thread = m.thread_of(message);
    let before = m.engine.state().clone();
    let undo = m.engine.history().undo_len();
    let preview = m
        .engine
        .preview(
            Command::MarkRead {
                ids: vec![thread],
                messages: Vec::new(),
            },
            &ctx(9),
        )
        .unwrap();
    assert_eq!(m.engine.state(), &before, "nothing changed");
    assert_eq!(m.engine.history().undo_len(), undo, "nothing recorded");
    assert!(preview.dry_run);
    assert_eq!(preview.receipt.name, "mark_read");
    assert!(
        preview.would_run.contains(&"run_remote_op".to_owned()),
        "{:?}",
        preview.would_run
    );
    let change = preview
        .changes
        .iter()
        .find(|c| c.entity == EntityRef::Message(message))
        .unwrap_or_else(|| panic!("{:?}", preview.changes));
    assert_eq!(change.untrusted_content.before["flags"]["seen"], false);
    assert_eq!(change.untrusted_content.after["flags"]["seen"], true);
    assert!(
        change.untrusted_content.before.get("subject").is_none(),
        "only the fields that differ"
    );
    // The JSON keeps the receipt's fields at the top, the values inside `untrusted_content`.
    let json = serde_json::to_value(&preview).unwrap();
    assert_eq!(json["name"], "mark_read");
    assert_eq!(json["dry_run"], true);
    assert!(json["changes"][0]["untrusted_content"]["after"].is_object());
}

#[test]
fn a_new_entity_has_no_before_and_rejections_are_the_same() {
    let m = inbox_mail();
    let preview = m
        .engine
        .preview(
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "new@gmail.com".into(),
                display_name: None,
                password: None,
                imap: None,
                smtp: None,
            },
            &ctx(9),
        )
        .unwrap();
    let created = &preview.changes[0];
    assert!(created.untrusted_content.before.is_null());
    assert_eq!(created.untrusted_content.after["email"], "new@gmail.com");
    assert_eq!(m.engine.state().accounts.len(), 1, "still one account");
    // An agent may not preview what it may not do.
    let mut agent = ctx(9);
    agent.origin = Origin::Mcp;
    let err = m
        .engine
        .preview(
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "x@gmail.com".into(),
                display_name: None,
                password: None,
                imap: None,
                smtp: None,
            },
            &agent,
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }), "{err:?}");
}
