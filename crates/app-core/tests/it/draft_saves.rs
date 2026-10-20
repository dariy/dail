//! Saving drafts to the server: idle saves, retries, saves on close, server copies of discarded
//! drafts.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{self, Mail, T0, names, of_kind};
use app_core::draft::{DRAFT_SAVE_IDLE_MS, DRAFT_SAVE_RETRY_MS};
use app_core::{
    AppState, Command, Ctx, DraftId, Effect, Engine, Event, MailboxRole, Outcome, ProviderPreset,
};

/// A Hostinger account with INBOX and Drafts.
fn mailboxes() -> Mail {
    Mail::new(
        ProviderPreset::Hostinger,
        &[("INBOX", None), ("Drafts", Some(MailboxRole::Drafts))],
    )
}

fn at(m: &Mail, now_ms: i64) -> Ctx {
    Ctx {
        now_ms,
        ..support::ctx(m.seed + 100)
    }
}

fn dispatch(m: &mut Mail, cmd: Command, now_ms: i64) -> Outcome {
    let ctx = at(m, now_ms);
    m.engine.dispatch(cmd, &ctx).unwrap()
}

fn ingest(m: &mut Mail, event: Event, now_ms: i64) -> Outcome {
    let ctx = at(m, now_ms);
    m.engine.ingest(event, &ctx).unwrap()
}

fn tick(m: &mut Mail, now_ms: i64) -> Outcome {
    ingest(m, Event::Tick { now_ms }, now_ms)
}

/// A draft created at `T0`.
fn create(m: &mut Mail) -> (DraftId, Outcome) {
    let account = m.account;
    let outcome = dispatch(
        m,
        Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Lunch?".into(),
            body: "Noon?".into(),
        },
        T0,
    );
    let id = serde_json::from_value(outcome.receipt.data["id"].clone()).unwrap();
    (id, outcome)
}

fn saves(outcome: &Outcome) -> Vec<&Effect> {
    of_kind(&outcome.effects, "save_draft")
}

fn state(m: &Mail) -> &AppState {
    m.engine.state()
}

#[test]
fn a_draft_is_saved_once_it_has_been_idle() {
    let mut m = mailboxes();
    let (id, created) = create(&mut m);
    assert!(created.effects.contains(&Effect::ScheduleTick {
        at_ms: T0 + DRAFT_SAVE_IDLE_MS
    }));
    assert!(saves(&tick(&mut m, T0 + DRAFT_SAVE_IDLE_MS - 1)).is_empty());

    let due = tick(&mut m, T0 + DRAFT_SAVE_IDLE_MS);
    let [
        Effect::SaveDraft {
            draft,
            path,
            revision,
            message,
            ..
        },
    ] = saves(&due)[..]
    else {
        panic!("{:?}", due.effects);
    };
    assert_eq!((*draft, path.as_str(), *revision), (id, "Drafts", T0));
    assert_eq!(message.from.email, "me@example.com");
    assert_eq!(message.from.name, None, "display name = address");
    assert_eq!(message.subject, "Lunch?");
    assert_eq!(message.message_id, state(&m).drafts[&id].message_id_hdr);
    assert!(state(&m).status.drafts_saving.contains(&id));
    // Not twice while it is running.
    assert!(saves(&tick(&mut m, T0 + 2 * DRAFT_SAVE_IDLE_MS)).is_empty());

    let saved = ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision: T0,
        },
        T0 + 6_000,
    );
    assert_eq!(names(&saved.effects), ["persist_state"]);
    let draft = &state(&m).drafts[&id];
    assert_eq!(draft.saved_ms, Some(T0));
    assert!(!draft.needs_save());
    assert!(state(&m).status.drafts_saving.is_empty());
    assert!(
        saves(&tick(&mut m, T0 + 60_000)).is_empty(),
        "nothing changed"
    );
}

#[test]
fn an_edit_during_a_save_is_saved_afterwards() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    tick(&mut m, T0 + DRAFT_SAVE_IDLE_MS);
    dispatch(
        &mut m,
        Command::UpdateDraft {
            id,
            to: None,
            cc: None,
            bcc: None,
            subject: Some("Lunch at one?".into()),
            body: None,
        },
        T0 + 5_500,
    );
    // The older revision lands while the new one is not idle yet.
    let saved = ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision: T0,
        },
        T0 + 6_000,
    );
    assert!(saves(&saved).is_empty());
    assert!(state(&m).drafts[&id].needs_save());
    let later = tick(&mut m, T0 + 5_500 + DRAFT_SAVE_IDLE_MS);
    assert!(matches!(
        saves(&later)[..],
        [Effect::SaveDraft { revision, message, .. }]
            if *revision == T0 + 5_500 && message.subject == "Lunch at one?"
    ));
}

#[test]
fn a_failed_save_is_shown_and_retried() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    tick(&mut m, T0 + DRAFT_SAVE_IDLE_MS);
    let failed = ingest(
        &mut m,
        Event::DraftSaveFailed {
            draft: id,
            error: "IMAP APPEND to Drafts failed: NO quota".into(),
        },
        T0 + 6_000,
    );
    assert!(failed.effects.contains(&Effect::ScheduleTick {
        at_ms: T0 + 6_000 + DRAFT_SAVE_RETRY_MS
    }));
    let draft = &state(&m).drafts[&id];
    assert!(draft.save_error.as_deref().unwrap().contains("quota"));
    assert_eq!(draft.saved_ms, None);
    let retry = tick(&mut m, T0 + 6_000 + DRAFT_SAVE_RETRY_MS);
    assert_eq!(saves(&retry).len(), 1);
    ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision: T0,
        },
        T0 + 70_000,
    );
    assert_eq!(state(&m).drafts[&id].save_error, None);
}

#[test]
fn closing_saves_changed_drafts_at_once() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    let closing = ingest(&mut m, Event::Closing {}, T0 + 1);
    assert!(matches!(
        saves(&closing)[..],
        [Effect::SaveDraft { draft, .. }] if *draft == id
    ));
    // Nothing to do when everything is saved.
    ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision: T0,
        },
        T0 + 2,
    );
    assert!(ingest(&mut m, Event::Closing {}, T0 + 3).effects.is_empty());
}

#[test]
fn drafts_wait_for_attachment_checks_and_need_a_drafts_mailbox() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    dispatch(
        &mut m,
        Command::AttachFile {
            draft: id,
            path: "/q3.pdf".into(),
            index: None,
        },
        T0,
    );
    assert!(saves(&ingest(&mut m, Event::Closing {}, T0 + 1)).is_empty());
    ingest(
        &mut m,
        Event::AttachmentInspected {
            draft: id,
            path: "/q3.pdf".into(),
            size: 3,
            content_type: "application/pdf".into(),
        },
        T0 + 2,
    );
    let closing = ingest(&mut m, Event::Closing {}, T0 + 3);
    assert!(matches!(
        saves(&closing)[..],
        [Effect::SaveDraft { message, .. }] if message.attachments.len() == 1
    ));

    // An account without a Drafts mailbox keeps its drafts local.
    let mut local = Mail::new(ProviderPreset::Hostinger, &[("INBOX", None)]);
    create(&mut local);
    assert!(
        ingest(&mut local, Event::Closing {}, T0 + 1)
            .effects
            .is_empty()
    );
}

#[test]
fn discarding_a_saved_draft_deletes_its_server_copy_and_undo_saves_it_again() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    // Never saved: nothing to delete.
    let discard = dispatch(&mut m, Command::DiscardDraft { id }, T0 + 1);
    assert!(of_kind(&discard.effects, "delete_draft").is_empty());
    dispatch(&mut m, Command::Undo {}, T0 + 2);

    let due = tick(&mut m, T0 + 2 + DRAFT_SAVE_IDLE_MS);
    let [Effect::SaveDraft { revision, .. }] = saves(&due)[..] else {
        panic!("{:?}", due.effects);
    };
    let revision = *revision;
    ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision,
        },
        T0 + 8_000,
    );
    assert!(!state(&m).drafts[&id].needs_save());
    let message_id = state(&m).drafts[&id].message_id_hdr.clone();
    let discard = dispatch(&mut m, Command::DiscardDraft { id }, T0 + 9_000);
    assert!(matches!(
        of_kind(&discard.effects, "delete_draft")[..],
        [Effect::DeleteDraft { draft, path, message_id: mid, .. }]
            if *draft == id && path == "Drafts" && *mid == message_id
    ));
    let undo = dispatch(&mut m, Command::Undo {}, T0 + 10_000);
    assert!(undo.effects.contains(&Effect::ScheduleTick {
        at_ms: T0 + 10_000 + DRAFT_SAVE_IDLE_MS
    }));
    let restored = &state(&m).drafts[&id];
    assert_eq!(restored.saved_ms, None, "its server copy is gone");
    assert!(restored.needs_save());
}

#[test]
fn discarding_while_saving_deletes_the_copy_being_saved() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    tick(&mut m, T0 + DRAFT_SAVE_IDLE_MS);
    let discard = dispatch(&mut m, Command::DiscardDraft { id }, T0 + 5_100);
    assert_eq!(of_kind(&discard.effects, "delete_draft").len(), 1);
    assert!(state(&m).status.drafts_saving.is_empty());
    // The save's result arrives for a draft that is gone.
    let late = ingest(
        &mut m,
        Event::DraftSaved {
            draft: id,
            revision: T0,
        },
        T0 + 5_200,
    );
    assert!(late.effects.is_empty());
    assert!(state(&m).drafts.is_empty());
}

#[test]
fn a_failed_delete_is_shown_in_the_account_status() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    let account = m.account;
    ingest(
        &mut m,
        Event::DraftDeleteFailed {
            account,
            draft: id,
            error: "IMAP SELECT Drafts failed: BYE".into(),
        },
        T0 + 1,
    );
    let issue = state(&m).status.sync[&account].last_error.clone().unwrap();
    assert!(
        issue.error.contains("server copy of draft"),
        "{}",
        issue.error
    );
    assert_eq!(issue.at_ms, T0 + 1);
}

#[test]
fn a_restart_forgets_saves_in_flight() {
    let mut m = mailboxes();
    let (id, _) = create(&mut m);
    tick(&mut m, T0 + DRAFT_SAVE_IDLE_MS);
    let mut engine = Engine::new(state(&m).clone());
    assert!(engine.state().status.drafts_saving.is_empty());
    let closing = engine
        .ingest(Event::Closing {}, &support::ctx(500))
        .unwrap();
    assert!(matches!(
        &closing.effects[..],
        [Effect::SaveDraft { draft, .. }] if *draft == id
    ));
}
