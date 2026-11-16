//! The actions of one message (MISC-44).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, CommandName, DraftId, MessageId};
use app_front::actions::Run;
use app_front::message_actions::{MessageAction, for_key, message_actions};
use app_front::palette::{self, AskKind, Choice, Flow, Target};

use crate::support::synced_state;

fn first_message(state: &AppState) -> MessageId {
    *state.messages.keys().next().unwrap()
}

fn actions(state: &AppState, message: MessageId) -> Vec<MessageAction> {
    message_actions(state, message)
        .into_iter()
        .map(|i| i.action)
        .collect()
}

fn command(state: &AppState, message: MessageId, action: MessageAction) -> Command {
    message_actions(state, message)
        .into_iter()
        .find(|i| i.action == action)
        .and_then(|i| match i.run {
            Run::Command(cmd) => Some(cmd),
            _ => None,
        })
        .unwrap()
}

/// Makes `message` a draft (`\Draft`) with a `Message-ID`.
fn make_draft(state: &mut AppState, message: MessageId) {
    let envelope = state.messages.get_mut(&message).unwrap();
    envelope.flags.draft = true;
    envelope.message_id_hdr = Some("draft-1@example.com".to_owned());
}

/// A local draft saved as `message`.
fn add_local_draft(state: &mut AppState, message: MessageId) -> DraftId {
    let account = state.messages[&message].account;
    let draft: app_core::Draft = serde_json::from_value(serde_json::json!({
        "id": "00000000-0000-0000-0000-0000000000d1",
        "account": account,
        "message_id_hdr": "draft-1@example.com",
        "created_ms": 0,
        "updated_ms": 0,
    }))
    .unwrap();
    let id = draft.id;
    state.drafts.insert(id, draft);
    id
}

#[test]
fn a_message_answers_toggles_and_trashes_alone() {
    let mut state = synced_state();
    let m = first_message(&state);
    {
        let flags = &mut state.messages.get_mut(&m).unwrap().flags;
        flags.seen = false;
        flags.flagged = false;
    }
    assert_eq!(
        actions(&state, m),
        [
            MessageAction::Reply,
            MessageAction::ReplyAll,
            MessageAction::Forward,
            MessageAction::MarkRead,
            MessageAction::Star,
            MessageAction::MakeRule,
            MessageAction::Trash,
        ]
    );
    assert_eq!(
        command(&state, m, MessageAction::Reply),
        Command::Reply {
            message: m,
            body: None
        }
    );
    assert_eq!(
        command(&state, m, MessageAction::ReplyAll),
        Command::ReplyAll {
            message: m,
            body: None
        }
    );
    assert_eq!(
        command(&state, m, MessageAction::Forward),
        Command::Forward {
            message: m,
            body: None
        }
    );
    assert_eq!(
        command(&state, m, MessageAction::MarkRead),
        Command::MarkRead {
            ids: Vec::new(),
            messages: vec![m]
        }
    );
    assert_eq!(
        command(&state, m, MessageAction::Star),
        Command::Star {
            ids: Vec::new(),
            messages: vec![m]
        }
    );
    assert_eq!(
        command(&state, m, MessageAction::Trash),
        Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![m]
        }
    );
}

#[test]
fn read_and_star_flip_with_the_flags() {
    let mut state = synced_state();
    let m = first_message(&state);
    {
        let flags = &mut state.messages.get_mut(&m).unwrap().flags;
        flags.seen = true;
        flags.flagged = true;
    }
    let got = actions(&state, m);
    assert!(got.contains(&MessageAction::MarkUnread) && got.contains(&MessageAction::Unstar));
    assert!(!got.contains(&MessageAction::MarkRead) && !got.contains(&MessageAction::Star));
    assert_eq!(
        command(&state, m, MessageAction::Unstar),
        Command::Unstar {
            ids: Vec::new(),
            messages: vec![m]
        }
    );
    // The read and star keys toggle, whichever of the pair they are bound to.
    let unread = Command::MarkUnread {
        ids: Vec::new(),
        messages: vec![m],
    };
    assert_eq!(
        for_key(&state, m, CommandName::MarkRead),
        Some(unread.clone())
    );
    assert_eq!(for_key(&state, m, CommandName::MarkUnread), Some(unread));
    assert_eq!(
        for_key(&state, m, CommandName::Star),
        Some(Command::Unstar {
            ids: Vec::new(),
            messages: vec![m]
        })
    );
    assert_eq!(for_key(&state, m, CommandName::ArchiveThreads), None);
    assert_eq!(for_key(&state, m, CommandName::EditDraft), None);
}

#[test]
fn a_local_draft_is_edited_and_discarded() {
    let mut state = synced_state();
    let m = first_message(&state);
    make_draft(&mut state, m);
    let id = add_local_draft(&mut state, m);
    assert_eq!(
        actions(&state, m),
        [MessageAction::Edit, MessageAction::Discard]
    );
    assert_eq!(
        command(&state, m, MessageAction::Edit),
        Command::EditDraft { message: m }
    );
    let discard = Command::DiscardDraft {
        id,
        keep_server_copy: false,
    };
    assert_eq!(command(&state, m, MessageAction::Discard), discard);
    assert_eq!(for_key(&state, m, CommandName::TrashThreads), Some(discard));
    assert_eq!(for_key(&state, m, CommandName::Reply), None);
}

#[test]
fn a_server_only_draft_is_edited_or_trashed() {
    let mut state = synced_state();
    let m = first_message(&state);
    make_draft(&mut state, m);
    assert_eq!(
        actions(&state, m),
        [MessageAction::Edit, MessageAction::Discard]
    );
    assert_eq!(
        for_key(&state, m, CommandName::EditDraft),
        Some(Command::EditDraft { message: m })
    );
    assert_eq!(
        command(&state, m, MessageAction::Discard),
        Command::TrashThreads {
            ids: Vec::new(),
            messages: vec![m]
        }
    );
}

#[test]
fn an_unknown_message_has_no_actions() {
    let state = synced_state();
    let id: MessageId = serde_json::from_str("\"00000000-0000-0000-0000-00000000beef\"").unwrap();
    assert!(message_actions(&state, id).is_empty());
}

/// Follows a flow, answering every step with its first choice.
fn walk(state: &AppState, mut flow: Flow) -> Flow {
    for _ in 0..6 {
        let Flow::Ask(step) = &flow else {
            return flow;
        };
        let choice = match &step.kind {
            AskKind::Text(_) => Choice::Text("Rule".to_owned()),
            AskKind::Confirm(_) => Choice::Yes,
            kind => match palette::ask_rows(state, kind, "")
                .first()
                .map(|r| r.target.clone())
            {
                Some(Target::Choice(choice)) => choice,
                other => panic!("{kind:?} offered {other:?}"),
            },
        };
        flow = step.answer(state, choice);
    }
    flow
}

#[test]
fn a_message_makes_a_rule_from_itself() {
    // TCH-8: from this message, not the thread's newest one; no key runs it.
    let mut state = synced_state();
    let m = first_message(&state);
    state.messages.get_mut(&m).unwrap().from[0].email = "only-this@example.net".to_owned();
    let item = message_actions(&state, m)
        .into_iter()
        .find(|i| i.action == MessageAction::MakeRule)
        .unwrap();
    assert_eq!(item.run, Run::RuleFrom(m));
    assert_eq!(MessageAction::MakeRule.key_command(), None);
    let Flow::Run(cmd @ Command::SaveRule { .. }) = walk(&state, item.run.flow(&state)) else {
        panic!("no save_rule");
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("only-this@example.net"), "{json}");
    // A draft does not offer it.
    make_draft(&mut state, m);
    assert!(!actions(&state, m).contains(&MessageAction::MakeRule));
}
