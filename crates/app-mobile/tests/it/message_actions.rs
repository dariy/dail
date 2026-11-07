//! The actions of each message card (MISC-44): the buttons come from
//! `app_front::message_actions`, a tap submits the command of that message alone, and a draft
//! card has Edit and Discard only. MISC-44e: a card swipes as a list row does (start: archive,
//! end: trash; a draft: edit, discard), and the buttons are listed on a long press.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, MessageId};
use app_mobile::UiEvent;
use app_mobile::frame::ReaderMessage;

use crate::support::{Phone, inbox_of, requests_of, synced_state};

fn labels(card: &ReaderMessage) -> Vec<&str> {
    card.actions.iter().map(|b| b.label.as_str()).collect()
}

fn action(card: &ReaderMessage, label: &str) -> u32 {
    card.actions
        .iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", card.actions))
        .action
}

/// The first card of the inbox's first conversation, opened on a phone over `state`.
fn first_card(phone: &mut Phone) -> ReaderMessage {
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.frame().reader.unwrap().messages[0].clone()
}

#[test]
fn every_card_carries_the_actions_of_its_message() {
    let mut phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let reader = phone.frame().reader.unwrap();
    assert!(!reader.messages.is_empty());
    for card in &reader.messages {
        let labels = labels(card);
        assert_eq!(
            &labels[..3],
            ["Reply", "Reply all", "Forward"],
            "{labels:?}"
        );
        assert!(labels.contains(&"Mark read") || labels.contains(&"Mark unread"));
        assert!(labels.contains(&"Star") || labels.contains(&"Unstar"));
        assert_eq!(labels.last(), Some(&"Trash"));
        assert!(card.actions.iter().all(|b| b.icon.is_some()));
    }
}

#[test]
fn a_tap_runs_the_action_on_that_message_alone() {
    let mut phone = Phone::inbox(360, 800);
    let card = first_card(&mut phone);
    let message: MessageId = card.message;
    let star = card
        .actions
        .iter()
        .find(|b| b.label == "Star" || b.label == "Unstar")
        .unwrap()
        .clone();
    let sent = phone.tap(star.action);
    let expected = if star.label == "Star" {
        Command::Star {
            ids: Vec::new(),
            messages: vec![message],
        }
    } else {
        Command::Unstar {
            ids: Vec::new(),
            messages: vec![message],
        }
    };
    assert_eq!(sent, vec![expected]);
    let card = first_card(&mut phone);
    let forward = action(&card, "Forward");
    // The presenter then opens the composer on the new draft.
    assert_eq!(
        phone.tap(forward)[0],
        Command::Forward {
            message,
            body: None
        }
    );
}

#[test]
fn a_draft_card_offers_edit_and_discard() {
    let mut state = synced_state();
    state.settings.stale_secs = 86_400;
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let envelope = state.messages.get_mut(&message).unwrap();
    envelope.flags.draft = true;
    envelope.message_id_hdr = Some("draft-1@example.com".to_owned());
    let mut phone = Phone::new(state, 360, 800);
    phone.dispatch(Command::OpenMailbox { id: inbox });
    let card = first_card(&mut phone);
    assert_eq!(card.message, message);
    assert_eq!(labels(&card), ["Edit", "Discard"]);
    assert_eq!(
        phone.tap(action(&card, "Edit")),
        vec![Command::EditDraft { message }]
    );
    // Edit made it a local draft: Discard now discards that, and the conversation stays.
    let card = first_card(&mut phone);
    let sent = phone.tap(action(&card, "Discard"));
    assert!(
        matches!(
            sent.as_slice(),
            [Command::DiscardDraft {
                keep_server_copy: false,
                ..
            }]
        ),
        "{sent:?}"
    );
    assert!(phone.frame().reader.is_some());
}

#[test]
fn a_server_only_draft_card_is_discarded_to_the_trash() {
    let mut state = synced_state();
    state.settings.stale_secs = 86_400;
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let envelope = state.messages.get_mut(&message).unwrap();
    envelope.flags.draft = true;
    envelope.message_id_hdr = Some("draft-1@example.com".to_owned());
    let mut phone = Phone::new(state, 360, 800);
    phone.dispatch(Command::OpenMailbox { id: inbox });
    let card = first_card(&mut phone);
    assert_eq!(
        phone.tap(action(&card, "Discard")),
        vec![Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![message],
        }]
    );
}

fn swipe(phone: &mut Phone, action: u32) -> Vec<Command> {
    phone.take();
    phone.event(UiEvent::Swipe { action });
    requests_of(&phone.take())
}

fn draft_phone() -> (Phone, MessageId) {
    let mut state = synced_state();
    state.settings.stale_secs = 86_400;
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let envelope = state.messages.get_mut(&message).unwrap();
    envelope.flags.draft = true;
    envelope.message_id_hdr = Some("draft-1@example.com".to_owned());
    let mut phone = Phone::new(state, 360, 800);
    phone.dispatch(Command::OpenMailbox { id: inbox });
    (phone, message)
}

#[test]
fn a_card_swipes_as_a_list_row() {
    let mut phone = Phone::inbox(360, 800);
    let card = first_card(&mut phone);
    let start = card.swipe_start.clone().unwrap();
    let end = card.swipe_end.clone().unwrap();
    assert_eq!(
        (start.icon.as_deref(), start.label.as_str()),
        (Some("archive"), "Archive")
    );
    assert_eq!(
        (end.icon.as_deref(), end.label.as_str()),
        (Some("delete"), "Trash")
    );
    assert_eq!(
        swipe(&mut phone, start.action),
        vec![Command::ArchiveThreads {
            ids: Vec::new(),
            messages: vec![card.message],
        }]
    );
    let card = first_card(&mut phone);
    let end = card.swipe_end.unwrap();
    assert_eq!(
        swipe(&mut phone, end.action),
        vec![Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![card.message],
        }]
    );
}

#[test]
fn a_draft_card_swipes_to_edit_and_to_discard() {
    let (mut phone, message) = draft_phone();
    let card = first_card(&mut phone);
    let start = card.swipe_start.clone().unwrap();
    assert_eq!(start.label, "Edit");
    let end = card.swipe_end.clone().unwrap();
    assert_eq!(end.label, "Discard");
    // A server-only draft is discarded to the Trash.
    assert_eq!(
        swipe(&mut phone, end.action),
        vec![Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![message],
        }]
    );
    let (mut phone, message) = draft_phone();
    let card = first_card(&mut phone);
    assert_eq!(
        swipe(&mut phone, card.swipe_start.unwrap().action),
        vec![Command::EditDraft { message }]
    );
    let card = first_card(&mut phone);
    let sent = swipe(&mut phone, card.swipe_end.unwrap().action);
    assert!(
        matches!(sent.as_slice(), [Command::DiscardDraft { .. }]),
        "{sent:?}"
    );
}
