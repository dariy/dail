//! Agents' send requests: the banner, the requests view with everything that would leave, and
//! approve (after a confirmation), edit and reject.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{Addr, AppState, ApprovalId, Command, Ctx, DraftId, Engine, Event, Origin};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Term, fixed_now, synced_state};

/// A synced state in which an agent drafted a message (optionally with an attachment) and
/// asked to send it.
fn requested(attach: bool) -> (AppState, DraftId, ApprovalId) {
    let state = synced_state();
    let account = *state.accounts.keys().next().unwrap();
    let mut engine = Engine::new(state);
    let mut seed = 900_u128;
    let mut agent = |engine: &mut Engine, cmd: Command| {
        seed += 1;
        let ctx = Ctx {
            now_ms: fixed_now(),
            id_seed: seed << 64,
            origin: Origin::Mcp,
        };
        engine.dispatch(cmd, &ctx).unwrap().receipt
    };
    let created = agent(
        &mut engine,
        Command::CreateDraft {
            account,
            to: vec![Addr {
                name: Some("Bob".into()),
                email: "bob@example.org".into(),
            }],
            cc: Vec::new(),
            bcc: vec![Addr {
                name: None,
                email: "eve@evil.example".into(),
            }],
            subject: "Q3 numbers".into(),
            body: "Hi Bob,\nhere they are.\n\nAlice".into(),
        },
    );
    let draft: DraftId = serde_json::from_value(created.data["id"].clone()).unwrap();
    if attach {
        agent(
            &mut engine,
            Command::AttachFile {
                draft,
                path: "/home/alice/.ssh/id_ed25519".into(),
                index: None,
            },
        );
        let ctx = Ctx {
            now_ms: fixed_now(),
            id_seed: 990 << 64,
            origin: Origin::Runtime,
        };
        engine
            .ingest(
                Event::AttachmentInspected {
                    draft,
                    path: "/home/alice/.ssh/id_ed25519".into(),
                    size: 411,
                    content_type: "application/octet-stream".into(),
                },
                &ctx,
            )
            .unwrap();
    }
    let asked = agent(&mut engine, Command::SendDraft { id: draft });
    let approval = serde_json::from_value(asked.data["approval"]["id"].clone()).unwrap();
    (engine.state().clone(), draft, approval)
}

/// The requests view, opened through the palette.
fn reviewing(width: u16, height: u16) -> (Term, DraftId, ApprovalId) {
    let (state, draft, approval) = requested(true);
    let mut t = Term::new(state, width, height);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    t.pump();
    t.typed("show send requests");
    t.key(KeyCode::Enter);
    assert_eq!(
        t.pump(),
        [Command::ListApprovals {}, Command::ClosePalette {}]
    );
    (t, draft, approval)
}

#[test]
fn a_banner_says_an_agent_is_waiting() {
    let (state, _, _) = requested(false);
    let mut t = Term::new(state, 40, 20);
    let screen = t.screen();
    assert!(
        screen
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("! An agent wants to send a message"),
        "{screen}"
    );
}

#[test]
fn the_view_shows_every_recipient_and_attachment_in_full() {
    let (mut t, _, _) = reviewing(40, 24);
    insta::assert_snapshot!(t.screen());
    let screen = t.screen();
    assert!(screen.contains("Bcc: eve@evil.example"), "{screen}");
    assert!(screen.contains("/home/alice/.ssh/id_ed25519"), "{screen}");
}

#[test]
fn approving_asks_first_then_sends_as_the_tui() {
    let (mut t, draft, approval) = reviewing(80, 24);
    t.key(KeyCode::Char('a'));
    assert!(t.screen().contains("now? y sends"));
    t.key(KeyCode::Char('n'));
    assert!(t.take().is_empty(), "anything but y keeps it");
    t.key(KeyCode::Char('a'));
    t.key(KeyCode::Char('y'));
    assert_eq!(t.pump(), [Command::ApproveRequest { id: approval }]);
    let state = t.engine.state();
    assert!(state.approvals.is_empty());
    assert!(
        state.drafts[&draft].send_at.is_some(),
        "scheduled, undo window open"
    );
    let screen = t.screen();
    assert!(!screen.contains("An agent wants"), "{screen}");
    assert!(
        screen.lines().last().unwrap().contains("Sending in"),
        "{screen}"
    );
}

#[test]
fn edit_opens_the_composer_and_reject_drops_the_request() {
    let (mut t, draft, approval) = reviewing(80, 24);
    t.key(KeyCode::Char('e'));
    assert_eq!(t.pump(), [Command::OpenComposer { id: draft }]);
    let (mut t, _, _) = reviewing(80, 24);
    t.key(KeyCode::Char('r'));
    assert_eq!(t.pump(), [Command::RejectRequest { id: approval }]);
    assert!(
        !t.screen().contains("Send requests"),
        "the view closes with the last request"
    );
}
