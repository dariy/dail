//! The composer: typed text goes back as one `update_draft` per pause, field change or command.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{Addr, Command, DraftId};
use app_mobile::UiEvent;
use app_mobile::event::Field;
use app_mobile::frame::Composer;

use crate::support::{Phone, advance, inbox_of, requests_of};

fn composing() -> (Phone, DraftId) {
    let mut phone = Phone::inbox(360, 800);
    let fab = phone.frame().fab.unwrap();
    phone.tap(fab.action);
    let id = phone.state().view.composer.unwrap();
    (phone, id)
}

fn composer(phone: &mut Phone) -> Composer {
    phone.frame().composer.unwrap()
}

fn text(phone: &mut Phone, field: Field, text: &str) -> Vec<Command> {
    phone.take();
    phone.event(UiEvent::Text {
        field,
        text: text.to_owned(),
    });
    requests_of(&phone.take())
}

fn addr(email: &str) -> Addr {
    Addr {
        name: None,
        email: email.to_owned(),
    }
}

#[test]
fn typing_goes_back_once_typing_pauses() {
    let (mut phone, id) = composing();
    assert!(text(&mut phone, Field::Subject, "Lun").is_empty());
    assert!(text(&mut phone, Field::Subject, "Lunch?").is_empty());
    assert!(text(&mut phone, Field::Body, "Noon").is_empty());
    let frame = phone.frame();
    assert!(frame.wants_tick);
    let shown = frame.composer.unwrap();
    assert_eq!(shown.subject.text, "Lunch?");
    phone.event(UiEvent::Tick);
    assert!(phone.take().is_empty(), "not yet a pause");
    advance(1_000);
    phone.event(UiEvent::Tick);
    assert_eq!(
        requests_of(&phone.take()),
        [Command::UpdateDraft {
            id,
            to: None,
            cc: None,
            bcc: None,
            subject: Some("Lunch?".to_owned()),
            body: Some("Noon".to_owned()),
        }]
    );
    let draft = &phone.state().drafts[&id];
    assert_eq!((&*draft.subject, &*draft.body_text), ("Lunch?", "Noon"));
    phone.event(UiEvent::Tick);
    assert!(phone.take().is_empty(), "written once");
    assert_eq!(
        composer(&mut phone).subject.version,
        shown.subject.version,
        "its own write-back does not reload the field"
    );
}

#[test]
fn leaving_a_field_or_acting_writes_back_first() {
    let (mut phone, id) = composing();
    text(&mut phone, Field::Subject, "Hi");
    phone.event(UiEvent::Blur {
        field: Field::Subject,
    });
    assert!(matches!(
        &requests_of(&phone.take())[..],
        [Command::UpdateDraft { subject: Some(s), body: None, .. }] if s == "Hi"
    ));
    text(&mut phone, Field::Body, "Text");
    let discard = composer(&mut phone)
        .actions
        .into_iter()
        .find(|b| b.label == "Discard")
        .unwrap()
        .action;
    let sent = phone.tap(discard);
    assert!(matches!(&sent[0], Command::UpdateDraft { body: Some(b), .. } if b == "Text"));
    assert_eq!(sent[1], Command::DiscardDraft { id });
    assert!(!phone.state().drafts.contains_key(&id));
}

#[test]
fn recipients_become_chips_at_a_separator() {
    let (mut phone, id) = composing();
    assert!(text(&mut phone, Field::To, "bob@example.org").is_empty());
    let before = composer(&mut phone).recipients[0].input.version;
    let sent = text(&mut phone, Field::To, "bob@example.org, ca");
    assert_eq!(
        sent,
        [Command::UpdateDraft {
            id,
            to: Some(vec![addr("bob@example.org")]),
            cc: None,
            bcc: None,
            subject: None,
            body: None,
        }]
    );
    let to = &composer(&mut phone).recipients[0];
    assert_eq!(to.label, "To");
    assert_eq!(to.chips[0].label, "bob@example.org");
    assert_eq!(to.input.text, "ca", "the rest stays typed");
    assert_ne!(to.input.version, before, "the host reloads the field");
    // Invalid text stays; a blur takes what is valid.
    assert!(text(&mut phone, Field::To, "nope;").is_empty());
    assert_eq!(composer(&mut phone).recipients[0].input.text, "nope");
    text(&mut phone, Field::Cc, "carol@example.org");
    phone.event(UiEvent::Blur { field: Field::Cc });
    let draft = &phone.state().drafts[&id];
    assert_eq!(draft.cc, [addr("carol@example.org")]);
    // A chip's remove action takes it out.
    let remove = composer(&mut phone).recipients[0].chips[0].remove;
    phone.tap(remove);
    assert!(phone.state().drafts[&id].to.is_empty());
}

#[test]
fn contacts_are_suggested_for_typed_text() {
    let (mut phone, id) = composing();
    text(&mut phone, Field::To, "car");
    let to = composer(&mut phone).recipients.remove(0);
    let carol = to
        .suggestions
        .iter()
        .find(|s| s.label.contains("carol@example.org"))
        .unwrap();
    let sent = phone.tap(carol.action);
    assert!(
        matches!(&sent[..], [Command::UpdateDraft { to: Some(to), .. }]
        if to.iter().any(|a| a.email == "carol@example.org"))
    );
    let to = composer(&mut phone).recipients.remove(0);
    assert!(to.input.text.is_empty());
    assert!(to.suggestions.is_empty());
    assert_eq!(phone.state().drafts[&id].to.len(), 1);
}

#[test]
fn an_edit_from_elsewhere_reloads_fields_without_typing() {
    let (mut phone, id) = composing();
    text(&mut phone, Field::Body, "mine");
    let before = composer(&mut phone);
    phone.dispatch(Command::UpdateDraft {
        id,
        to: None,
        cc: None,
        bcc: None,
        subject: Some("From an agent".to_owned()),
        body: Some("theirs".to_owned()),
    });
    let after = composer(&mut phone);
    assert_eq!(after.subject.text, "From an agent");
    assert_ne!(after.subject.version, before.subject.version);
    assert_eq!(after.body.text, "mine", "unsaved typing wins");
    assert_eq!(after.body.version, before.body.version);
}

#[test]
fn a_reply_folds_its_quote_and_untrusted_text_is_scrubbed() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let reply = phone
        .frame()
        .reader
        .unwrap()
        .actions
        .into_iter()
        .find(|b| b.label == "Reply")
        .unwrap();
    phone.tap(reply.action);
    let shown = composer(&mut phone);
    assert_eq!(shown.title, "Reply");
    let quote = shown.quote.unwrap();
    assert!(!quote.expanded && quote.header.is_empty());
    phone.tap(quote.toggle);
    let quote = composer(&mut phone).quote.unwrap();
    assert!(quote.expanded && !quote.header.is_empty());
    let id = phone.state().view.composer.unwrap();
    phone.dispatch(Command::UpdateDraft {
        id,
        to: None,
        cc: None,
        bcc: None,
        subject: None,
        body: Some("one\ntwo\u{7}\u{1b}[31m\u{202e}".to_owned()),
    });
    let shown = composer(&mut phone);
    assert_eq!(shown.body.text, "one\ntwo\u{fffd}\u{fffd}[31m\u{fffd}");
}

#[test]
fn a_scheduled_send_makes_it_read_only() {
    let (mut phone, id) = composing();
    phone.dispatch(Command::UpdateDraft {
        id,
        to: Some(vec![addr("bob@example.org")]),
        cc: None,
        bcc: None,
        subject: None,
        body: None,
    });
    phone.dispatch(Command::SendDraft { id });
    let shown = composer(&mut phone);
    assert!(!shown.editable);
    assert!(shown.actions.is_empty());
    assert!(text(&mut phone, Field::Subject, "late").is_empty());
    advance(5_000);
    phone.event(UiEvent::Tick);
    assert!(phone.take().is_empty());
}

#[test]
fn keys_type_rather_than_act_in_the_composer() {
    let (mut phone, _) = composing();
    let _ = phone.frame();
    phone.take();
    phone.event(UiEvent::Key {
        chord: "e".to_owned(),
    });
    assert!(
        phone.take().is_empty(),
        "`e` archives in the list, not here"
    );
}

#[test]
fn attach_asks_the_host_and_the_files_show_as_chips() {
    let (mut phone, id) = composing();
    let attach = composer(&mut phone)
        .actions
        .into_iter()
        .find(|b| b.label == "Attach files")
        .unwrap();
    assert!(phone.tap(attach.action).is_empty());
    assert_eq!(
        phone.externals(),
        [app_mobile::External::PickFiles { draft: id }]
    );
    for path in ["/files/outgoing/a/notes.txt", "/files/outgoing/b/big.bin"] {
        phone.dispatch(Command::AttachFile {
            draft: id,
            path: path.to_owned(),
            index: None,
        });
    }
    let files = composer(&mut phone).attachments;
    assert_eq!(files[0].name, "notes.txt");
    assert_eq!((&*files[0].detail, files[0].failed), ("checking…", false));
    phone.ingest(app_core::Event::AttachmentInspected {
        draft: id,
        path: "/files/outgoing/a/notes.txt".to_owned(),
        size: 2048,
        content_type: "text/plain".to_owned(),
    });
    phone.ingest(app_core::Event::AttachmentFailed {
        draft: id,
        path: "/files/outgoing/b/big.bin".to_owned(),
        error: "gone\u{202e}".to_owned(),
    });
    let files = composer(&mut phone).attachments;
    assert!(
        !files[0].failed && files[0].detail.contains("KB"),
        "{files:?}"
    );
    assert!(files[1].failed && !files[1].detail.contains('\u{202e}'));
    assert_eq!(
        phone.tap(files[1].remove.unwrap()),
        [Command::RemoveAttachment {
            draft: id,
            index: 1
        }]
    );
    assert_eq!(phone.state().drafts[&id].attachments.len(), 1);
}

fn addressed(phone: &mut Phone, id: DraftId) {
    phone.dispatch(Command::UpdateDraft {
        id,
        to: Some(vec![addr("bob@example.org")]),
        cc: None,
        bcc: None,
        subject: Some("Hi".to_owned()),
        body: None,
    });
}

fn send_button(phone: &mut Phone) -> Option<u32> {
    composer(phone)
        .actions
        .into_iter()
        .find(|b| b.label == "Send")
        .map(|b| b.action)
}

#[test]
fn send_needs_a_recipient_and_keeps_the_process_until_it_leaves() {
    let (mut phone, id) = composing();
    assert!(send_button(&mut phone).is_none(), "no recipient yet");
    addressed(&mut phone, id);
    let send = send_button(&mut phone).unwrap();
    assert_eq!(phone.tap(send), [Command::SendDraft { id }]);
    let frame = phone.frame();
    assert_eq!(
        phone.externals(),
        [app_mobile::External::Sending { draft: id }]
    );
    assert!(frame.snackbar.unwrap().text.starts_with("Sending in"));
    let snackbar = phone.frame().snackbar.unwrap();
    assert!(phone.externals().is_empty(), "once per send");
    assert_eq!(
        phone.tap(snackbar.action.unwrap().action),
        [Command::CancelSend { id }]
    );
    assert!(!phone.state().drafts[&id].is_outgoing());
    phone.frame();
    assert!(phone.externals().is_empty());
}

#[test]
fn a_send_a_restart_cancelled_is_told_once() {
    let (mut phone, id) = composing();
    addressed(&mut phone, id);
    phone.dispatch(Command::SendDraft { id });
    // A restart: the engine starts again over the persisted state.
    let state = phone.state().clone();
    let mut phone = Phone::new(state, 360, 800);
    let frame = phone.frame();
    let text = frame.snackbar.unwrap().text;
    assert!(text.starts_with("Not sent: “Hi”"), "{text}");
    assert!(text.contains("restart"), "{text}");
    advance(7_000);
    assert!(phone.frame().snackbar.is_none(), "told once");
    advance(-7_000);
}
