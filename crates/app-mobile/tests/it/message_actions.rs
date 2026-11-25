//! The actions of each message card (MISC-44): the buttons come from
//! `app_front::message_actions`, a tap submits the command of that message alone, and a draft
//! card has Edit and Discard only. MISC-44e: a card swipes as a list row does (start: archive,
//! end: trash; a draft: edit, discard), and the buttons are listed on a long press.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, MessageId};
use app_mobile::frame::ReaderMessage;

use crate::support::{Phone, inbox_of, synced_state};

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
    // Edit makes it a local draft and opens the composer on it.
    let sent = phone.tap(action(&card, "Edit"));
    assert!(
        matches!(
            sent.as_slice(),
            [Command::EditDraft { message: m }, Command::OpenComposer { .. }] if *m == message
        ),
        "{sent:?}"
    );
    phone.dispatch(Command::CloseComposer {});
    // Discard now discards that local draft, and the conversation stays.
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

#[test]
fn a_card_archives_its_message_alone_and_has_no_swipes() {
    let mut phone = Phone::inbox(360, 800);
    let card = first_card(&mut phone);
    let all = labels(&card);
    let at = |label: &str| all.iter().position(|l| *l == label).unwrap();
    assert_eq!(at("Archive") + 1, at("Trash"), "{all:?}");
    assert_eq!(
        phone.tap(action(&card, "Archive")),
        vec![Command::ArchiveThreads {
            ids: Vec::new(),
            messages: vec![card.message],
        }]
    );
    // The reader's horizontal drag closes the message: no card of any frame swipes.
    let json = serde_json::to_string(&phone.frame().reader).unwrap();
    assert!(!json.contains("swipe"), "{json}");
}

#[test]
fn a_card_makes_a_rule_from_its_message() {
    // TCH-8: once in the card's menu, before Archive and Trash; a tap opens the palette at the
    // rule step.
    let mut phone = Phone::inbox(360, 800);
    let card = first_card(&mut phone);
    let labels = labels(&card);
    assert_eq!(labels.iter().filter(|l| **l == "Make a rule…").count(), 1);
    assert_eq!(
        labels[labels.len() - 3..],
        ["Make a rule…", "Archive", "Trash"]
    );
    let tap = action(&card, "Make a rule…");
    assert_eq!(phone.tap(tap), [Command::OpenPalette {}]);
    let sheet = phone.frame().palette.unwrap();
    assert!(sheet.prompt.is_some(), "the palette opens at a step");
}
