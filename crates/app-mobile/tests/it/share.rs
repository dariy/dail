//! `mailto:` links and the share sheet: a new draft in the composer, never a send; only other
//! apps' content URIs are attached.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{AppState, Command};
use app_mobile::{External, UiEvent};

use crate::support::{Phone, requests_of};

fn share(phone: &mut Phone, event: UiEvent) -> Vec<Command> {
    phone.take();
    phone.event(event);
    requests_of(&phone.take())
}

#[test]
fn a_mailto_link_opens_a_filled_draft_and_never_sends() {
    let mut phone = Phone::inbox(360, 800);
    let sent = share(
        &mut phone,
        UiEvent::Share {
            mailto: Some("mailto:bob@example.org?subject=Hi&body=See%20you".to_owned()),
            to: Vec::new(),
            subject: None,
            text: None,
            files: Vec::new(),
        },
    );
    assert!(
        matches!(&sent[0], Command::CreateDraft { subject, body, to, .. }
        if subject == "Hi" && body == "See you" && to[0].email == "bob@example.org")
    );
    assert!(
        sent.iter().all(|c| !matches!(c, Command::SendDraft { .. })),
        "{sent:?}"
    );
    let id = phone.state().view.composer.unwrap();
    assert!(!phone.state().drafts[&id].is_outgoing());
    assert!(phone.frame().composer.is_some());
    assert!(phone.externals().is_empty(), "no files, nothing to import");
}

#[test]
fn shared_text_and_files_become_a_draft_with_checked_files() {
    let mut phone = Phone::inbox(360, 800);
    share(
        &mut phone,
        UiEvent::Share {
            mailto: None,
            to: vec!["carol@example.org".to_owned(), "junk".to_owned()],
            subject: Some("Photos".to_owned()),
            text: Some("Here they are".to_owned()),
            files: vec![
                "content://com.android.providers.media.documents/document/image%3A12".to_owned(),
                "file:///data/data/net.darii.dail/files/profiles/default/state.redb".to_owned(),
                "content://net.darii.dail.debug/state".to_owned(),
                "CONTENT://10@net.darii.dail/x".to_owned(),
                "content://com.example.files/a.pdf".to_owned(),
            ],
        },
    );
    let id = phone.state().view.composer.unwrap();
    let draft = &phone.state().drafts[&id];
    assert_eq!(draft.to.len(), 1);
    assert_eq!(
        (&*draft.subject, &*draft.body_text),
        ("Photos", "Here they are")
    );
    let frame = phone.frame();
    assert_eq!(
        phone.externals(),
        [External::ImportFiles {
            draft: id,
            uris: vec![
                "content://com.android.providers.media.documents/document/image%3A12".to_owned(),
                "content://com.example.files/a.pdf".to_owned(),
            ],
        }]
    );
    let text = frame.snackbar.unwrap().text;
    assert!(
        text.starts_with("3 shared files could not be attached"),
        "{text}"
    );
}

#[test]
fn sharing_without_an_account_says_so() {
    let mut phone = Phone::new(AppState::default(), 360, 800);
    let sent = share(
        &mut phone,
        UiEvent::Share {
            mailto: Some("mailto:bob@example.org".to_owned()),
            to: Vec::new(),
            subject: None,
            text: None,
            files: Vec::new(),
        },
    );
    assert!(sent.is_empty());
    assert!(phone.frame().snackbar.unwrap().text.contains("account"));
}
