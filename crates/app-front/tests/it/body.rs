//! The reader's body cache: which `get_thread` it asks for and what it keeps of the answer.

use crate::support::{inbox_of, synced_state};
use app_core::document::Block;
use app_core::{Command, CommandReceipt};
use app_front::body::Bodies;
use serde_json::json;

#[test]
fn a_rich_cache_asks_for_documents_and_keeps_them() {
    let mut state = synced_state();
    let (_, threads) = inbox_of(&state);
    state.view.thread = Some(threads[0]);
    let message = state.threads[&threads[0]].messages[0];

    let plain = Bodies::default().refresh(&state);
    assert!(matches!(
        plain,
        Some((
            _,
            Command::GetThread {
                include_body: true,
                rich_body: false,
                ..
            }
        ))
    ));
    let mut bodies = Bodies::rich();
    let (thread, cmd) = bodies.refresh(&state).unwrap();
    assert_eq!(
        cmd,
        Command::GetThread {
            id: threads[0],
            include_body: true,
            rich_body: true,
        }
    );
    let receipt = CommandReceipt {
        ok: true,
        name: "get_thread".into(),
        kind: None,
        affected: Vec::new(),
        data: json!({ "messages": [{ "id": message, "untrusted_content": { "body": {
            "text": "Hi", "text_source": "html", "attachments": [],
            "document": { "blocks": [{ "type": "rule" }] },
        }}}]}),
        undo_label: None,
    };
    bodies.accept(thread, Some(&receipt));
    let body = bodies.get(message).unwrap();
    assert_eq!(body.text, "Hi");
    assert_eq!(body.document.as_ref().unwrap().blocks, [Block::Rule]);
}
