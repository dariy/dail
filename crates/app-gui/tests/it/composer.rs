//! The composer (recipient chips, subject and body written back on pause or before send,
//! attachments) and the toasts (undo, send countdown, errors).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Gui, apply, gui, inbox_of, synced_state};
use app_core::{Addr, Command, DraftId};
use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A file dropped from the file manager: egui only hands the composer its path.
#[derive(Debug)]
struct PathFile(PathBuf);

impl egui::DroppedFile for PathFile {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        Err("not read by the composer".into())
    }
}

/// A harness with a new, open draft.
fn composing() -> (Harness<'static, Gui>, DraftId) {
    let state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let mut h = gui(state);
    let receipt = apply(
        &mut h,
        Command::CreateDraft {
            account,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        },
    );
    let id: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    apply(&mut h, Command::OpenComposer { id });
    h.state_mut().out.clear();
    (h, id)
}

fn type_into(h: &mut Harness<'static, Gui>, by: By<'_>, text: &str) {
    h.get(by).click();
    h.run();
    h.event(egui::Event::Text(text.to_owned()));
    h.run();
}

fn bob() -> Addr {
    Addr {
        name: Some("Bob".into()),
        email: "bob@example.org".into(),
    }
}

#[test]
fn recipients_become_chips_and_can_be_removed() {
    let (mut h, id) = composing();
    type_into(
        &mut h,
        By::new().label("To").role(Role::TextInput),
        "Bob <bob@example.org>,",
    );
    let sent = h.state_mut().pump();
    assert_eq!(
        sent,
        [Command::UpdateDraft {
            id,
            to: Some(vec![bob()]),
            cc: None,
            bcc: None,
            subject: None,
            body: None,
        }]
    );
    h.run();
    h.get_by_label("Bob <bob@example.org>");
    h.get_by_label("Remove bob@example.org").click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::UpdateDraft {
            id,
            to: Some(Vec::new()),
            cc: None,
            bcc: None,
            subject: None,
            body: None
        }]
    );
}

#[test]
fn invalid_addresses_stay_in_the_field() {
    let (mut h, _) = composing();
    type_into(
        &mut h,
        By::new().label("To").role(Role::TextInput),
        "not-an-address,",
    );
    assert!(h.state_mut().pump().is_empty());
    let field = h.get(By::new().label("To").role(Role::TextInput));
    assert_eq!(field.value().as_deref(), Some("not-an-address"));
}

#[test]
fn typed_text_is_written_back_before_send() {
    let (mut h, id) = composing();
    apply(
        &mut h,
        Command::UpdateDraft {
            id,
            to: Some(vec![bob()]),
            cc: None,
            bcc: None,
            subject: None,
            body: None,
        },
    );
    h.state_mut().out.clear();
    type_into(
        &mut h,
        By::new().label("Subject").role(Role::TextInput),
        "Lunch",
    );
    // Moving to the body writes the subject back.
    type_into(&mut h, By::new().role(Role::MultilineTextInput), "At noon?");
    assert_eq!(
        h.state_mut().pump(),
        [Command::UpdateDraft {
            id,
            to: None,
            cc: None,
            bcc: None,
            subject: Some("Lunch".into()),
            body: None
        }]
    );
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [
            Command::UpdateDraft {
                id,
                to: None,
                cc: None,
                bcc: None,
                subject: None,
                body: Some("At noon?".into())
            },
            Command::SendDraft { id },
        ]
    );
    // The send waits for its undo window; the toast can cancel it.
    h.run();
    h.get_by_label("Sending in 10 s");
    // The toast and the composer both offer it.
    h.get_all(By::new().role(Role::Button).label_contains("Cancel send"))
        .next()
        .unwrap()
        .click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::CancelSend { id }]);
}

#[test]
fn a_rejected_send_shows_the_error() {
    let (mut h, _) = composing();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    let sent = h.state_mut().pump();
    assert!(matches!(sent[..], [Command::SendDraft { .. }]));
    h.run();
    h.get_by_label_contains("Send: draft");
}

#[test]
fn attachments_show_their_state_and_can_be_removed() {
    let (mut h, id) = composing();
    apply(
        &mut h,
        Command::AttachFile {
            draft: id,
            path: "/home/alice/q3.pdf".into(),
            index: None,
        },
    );
    h.get_by_label("📎 q3.pdf");
    h.get_by_label("checking…");
    h.get_by_label("Remove q3.pdf").click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::RemoveAttachment {
            draft: id,
            index: 0
        }]
    );
}

#[test]
fn dropped_files_are_attached() {
    let (mut h, id) = composing();
    h.state_mut().out.clear();
    h.input_mut().hovered_files.push(egui::HoveredFile {
        path: Some("/home/alice/q3.pdf".into()),
        ..Default::default()
    });
    // egui repaints for as long as files hover: one frame at a time. The hint's area sizes
    // itself, unseen, in its first frame.
    h.run_steps(2);
    h.get_by_label("Drop the files to attach them");
    h.input_mut().hovered_files.clear();
    for path in ["/home/alice/q3.pdf", "/home/alice/notes.txt"] {
        h.input_mut()
            .dropped_files
            .push(Arc::new(PathFile(path.into())));
    }
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [
            Command::AttachFile {
                draft: id,
                path: "/home/alice/q3.pdf".into(),
                index: None,
            },
            Command::AttachFile {
                draft: id,
                path: "/home/alice/notes.txt".into(),
                index: None,
            },
        ]
    );
    assert!(h.query_by_label("Drop the files to attach them").is_none());
}

#[test]
fn discard_and_close() {
    let (mut h, id) = composing();
    h.get(By::new().role(Role::Button).label("Discard")).click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::DiscardDraft { id }]);
    h.run();
    assert!(
        h.query_by_label("New message").is_none(),
        "the composer closed with the draft"
    );
}

#[test]
fn undoable_commands_get_an_undo_toast() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.key_press(Key::E);
    h.run();
    assert!(matches!(
        h.state_mut().pump()[..],
        [Command::ArchiveThreads { .. }]
    ));
    h.run();
    h.get(By::new().role(Role::Button).label_contains("Undo"))
        .click();
    h.run();
    assert_eq!(h.state_mut().pump(), [Command::Undo {}]);
}

#[test]
fn recipients_are_suggested_from_the_mail() {
    let (mut h, id) = composing();
    type_into(&mut h, By::new().label("To").role(Role::TextInput), "car");
    h.run();
    assert!(h.state_mut().pump().is_empty(), "nothing added by typing");
    h.get_by_label("Carol <carol@example.org>").click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::UpdateDraft {
            id,
            to: Some(vec![Addr {
                name: Some("Carol".into()),
                email: "carol@example.org".into(),
            }]),
            cc: None,
            bcc: None,
            subject: None,
            body: None,
        }]
    );
}

#[test]
fn the_sender_is_chosen_among_the_accounts_addresses() {
    let (mut h, id) = composing();
    let account = h.state().engine.state().drafts[&id].account;
    apply(
        &mut h,
        Command::SetIdentity {
            account,
            email: "sales@example.com".into(),
            name: None,
            signature: None,
        },
    );
    h.state_mut().out.clear();
    h.get(By::new().role(Role::ComboBox)).click();
    h.run();
    h.get_by_label("sales@example.com").click();
    h.run();
    assert_eq!(
        h.state_mut().pump(),
        [Command::SetDraftFrom {
            id,
            email: "sales@example.com".into()
        }]
    );
}

/// A harness with the "Q3 numbers" conversation open and the message to answer.
fn reading() -> (Harness<'static, Gui>, app_core::MessageId) {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = *state.threads[&threads[1]].messages.last().unwrap();
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[1] });
    h.state_mut().out.clear();
    (h, message)
}

#[test]
fn a_reply_docks_under_the_conversation() {
    let (mut h, message) = reading();
    let receipt = apply(
        &mut h,
        Command::Reply {
            message,
            body: None,
        },
    );
    let id: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    apply(&mut h, Command::OpenComposer { id });
    // The reader stays: its toolbar is above the composer.
    let forward = h
        .get(By::new().role(Role::Button).label_contains("Forward"))
        .rect();
    let send = h
        .get(By::new().role(Role::Button).label_contains("Send"))
        .rect();
    assert!(
        forward.max.y < send.min.y,
        "the composer is under the reader"
    );
    h.state_mut().out.clear();
    type_into(&mut h, By::new().label("Subject"), " now");
    h.get(By::new().role(Role::Button).label("Discard")).click();
    h.run();
    let sent = h.state_mut().pump();
    assert!(
        matches!(&sent[..], [Command::UpdateDraft { subject: Some(s), .. }, Command::DiscardDraft { id: d }] if s.ends_with(" now") && *d == id),
        "{sent:?}"
    );
}

#[test]
fn a_forward_takes_the_whole_pane() {
    let (mut h, message) = reading();
    let receipt = apply(
        &mut h,
        Command::Forward {
            message,
            body: None,
        },
    );
    let id: DraftId = serde_json::from_value(receipt.data["id"].clone()).unwrap();
    apply(&mut h, Command::OpenComposer { id });
    h.get(By::new().role(Role::Button).label_contains("Send"));
    assert!(
        h.query_by_label_contains("Reply all").is_none(),
        "the reader is replaced"
    );
}
