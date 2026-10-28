//! Query results as documents (MISC-22): one snapshot per query over the demo state, every query
//! has a view, and untrusted text is cleaned.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{AppState, Command, CommandKind, CommandName, Ctx, Engine, Origin};
use app_front::result::{self, ResultDoc};
use serde_json::{Value, json};

use crate::palette::rich_state;
use crate::support::{self, inbox_of};

fn run(state: &AppState, cmd: Value) -> (CommandName, Value) {
    let cmd: Command = serde_json::from_value(cmd).unwrap();
    let name = CommandName::from_name(cmd.name()).unwrap();
    let ctx = Ctx {
        now_ms: support::NOW,
        id_seed: 7 << 64,
        origin: Origin::Cli,
    };
    let receipt = Engine::new(state.clone())
        .dispatch(cmd, &ctx)
        .unwrap()
        .receipt;
    (name, receipt.data)
}

fn doc(state: &AppState, cmd: Value) -> ResultDoc {
    let (name, data) = run(state, cmd);
    result::doc(name, &data, state, support::NOW).unwrap()
}

/// Every query, with arguments over the demo state.
fn queries(state: &AppState) -> Vec<(CommandName, Value)> {
    let (inbox, threads) = inbox_of(state);
    let account = *state.accounts.keys().next().unwrap();
    let message = state.threads[&threads[0]].messages[0];
    let all = vec![
        json!({"cmd": "get_message", "args": {"id": message}}),
        json!({"cmd": "get_thread", "args": {"id": threads[0]}}),
        json!({"cmd": "list_threads", "args": {"mailbox": inbox}}),
        json!({"cmd": "search", "args": {"query": "a"}}),
        json!({"cmd": "list_mailboxes", "args": {}}),
        json!({"cmd": "list_contacts", "args": {}}),
        json!({"cmd": "list_pending_ops", "args": {}}),
        json!({"cmd": "list_approvals", "args": {}}),
        json!({"cmd": "get_settings", "args": {}}),
        json!({"cmd": "get_view", "args": {}}),
        json!({"cmd": "test_account", "args": {"id": account}}),
        json!({"cmd": "discover_servers", "args": {"email": "me@example.org"}}),
    ];
    all.into_iter()
        .map(|cmd| {
            let name = CommandName::from_name(
                serde_json::from_value::<Command>(cmd.clone())
                    .unwrap()
                    .name(),
            )
            .unwrap();
            (name, cmd)
        })
        .collect()
}

#[test]
fn every_query_has_a_view() {
    let state = rich_state();
    let covered: Vec<CommandName> = queries(&state).into_iter().map(|(n, _)| n).collect();
    for name in CommandName::ALL.iter().copied() {
        if name.kind() == CommandKind::Query {
            assert!(result::has_view(name), "{name:?} has no result view");
            assert!(covered.contains(&name), "{name:?} has no snapshot here");
        } else {
            assert!(!result::has_view(name));
        }
    }
}

#[test]
fn snapshots_per_query() {
    let state = rich_state();
    for (name, cmd) in queries(&state) {
        let doc = doc(&state, cmd);
        insta::assert_debug_snapshot!(name.as_str(), doc);
    }
}

#[test]
fn a_thread_row_opens_the_conversation() {
    let state = rich_state();
    let (inbox, threads) = inbox_of(&state);
    let doc = doc(
        &state,
        json!({"cmd": "list_threads", "args": {"mailbox": inbox}}),
    );
    let row = &doc.sections[0].rows[0];
    assert_eq!(row.command, Some(Command::OpenThread { id: threads[0] }));
}

#[test]
fn untrusted_text_is_cleaned() {
    let state = rich_state();
    let data = json!({
        "threads": [{
            "id": null,
            "untrusted_content": {
                "subject": "pay\u{202e}now\u{7}",
                "from": ["evil\u{200f}\u{1b}[31m"],
            },
        }],
        "total": 1,
    });
    let doc = result::doc(CommandName::ListThreads, &data, &state, support::NOW).unwrap();
    let row = &doc.sections[0].rows[0];
    for text in [&row.title, row.detail.as_ref().unwrap()] {
        assert!(
            !text.chars().any(|c| c.is_control()
                || ('\u{2066}'..='\u{2069}').contains(&c)
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{200e}' | '\u{200f}')),
            "{text:?}"
        );
    }
    assert!(row.command.is_none());
}

#[test]
fn unknown_data_falls_back_to_fields_without_braces() {
    let state = rich_state();
    let data = json!({"odd": {"nested": {"deep": [1, 2]}}, "flag": true});
    let doc = result::doc(CommandName::GetView, &data, &state, support::NOW).unwrap();
    let text = format!("{:?}", doc.sections);
    assert!(!text.contains('{') || text.contains("Section {"));
    for section in &doc.sections {
        for field in &section.fields {
            assert!(
                !field.value.contains('{') && !field.value.contains('}'),
                "{field:?}"
            );
        }
    }
}
