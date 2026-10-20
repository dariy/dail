//! A TUI attached over IPC to a running session (as to a GUI or `dail mcp` that owns the profile):
//! an agent's archive reaches its screen through the change notifications, without polling, and
//! the user's approval of the agent's send goes through with origin `tui`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::{Arc, mpsc};
use std::time::Duration;

use app_core::{Command, Origin, listing};
use app_sync::{Bridge, Connection, IpcClient, IpcServer};
use app_testkit::Fixture;
use app_tui::{Input, Purpose, Tui};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::support::{inbox_of, test_now};

/// The TUI and its bridge, driven like the runner does, but one step at a time.
struct Attached {
    tui: Tui,
    bridge: Bridge<Purpose>,
    terminal: Terminal<TestBackend>,
    woken: mpsc::Receiver<()>,
}

impl Attached {
    /// Hands the replies and background requests over, and draws. Returns the screen.
    fn frame(&mut self) -> String {
        let state = self.bridge.state();
        for r in self.tui.replies(self.bridge.replies(), &state) {
            self.bridge.submit(r.purpose, r.cmd);
        }
        for r in self.tui.refresh(&state) {
            self.bridge.submit(r.purpose, r.cmd);
        }
        let tui = &mut self.tui;
        self.terminal.draw(|f| tui.draw(f, &state)).unwrap();
        let buffer = self.terminal.backend().buffer();
        let width = usize::from(buffer.area.width);
        buffer
            .content
            .chunks(width)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>() + "\n")
            .collect()
    }

    fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let state = self.bridge.state();
        for r in self
            .tui
            .input(Input::Key(KeyEvent::new(code, modifiers)), &state)
        {
            self.bridge.submit(r.purpose, r.cmd);
        }
        self.settle();
    }

    fn typed(&mut self, text: &str) {
        for c in text.chars() {
            self.key(KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    /// Waits for wake-ups (state changes, replies) until there are none for 100 ms, drawing
    /// after each, as the runner would.
    fn settle(&mut self) {
        self.frame();
        while self.woken.recv_timeout(Duration::from_millis(100)).is_ok() {
            self.frame();
        }
    }

    /// Waits (up to 5 s) for a wake-up after which the screen satisfies `done`.
    fn wait_for(&mut self, what: &str, done: impl Fn(&str) -> bool) -> String {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let screen = self.frame();
            if done(&screen) {
                return screen;
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(
                self.woken.recv_timeout(left).is_ok(),
                "no wake-up while waiting for {what}:\n{screen}"
            );
        }
    }
}

#[test]
fn an_agents_changes_show_up_live_and_the_tui_approves_as_tui() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let profile = std::env::temp_dir().join(format!("dail-tui-attach-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&profile);
    std::fs::create_dir_all(&profile).unwrap();
    let synced = rt
        .block_on(app_testkit::synced_session(
            Fixture::builtin("basic").unwrap(),
        ))
        .unwrap();
    let session = synced.session;
    // The owner listens on the profile's socket, as `dail gui` and `dail mcp` do.
    let server = rt
        .block_on(async { IpcServer::listen(&profile, session.client()) })
        .unwrap();
    let state = session.state();
    let (inbox, threads) = inbox_of(&state);
    rt.block_on(session.submit(Command::OpenMailbox { id: inbox }, Origin::Cli))
        .unwrap()
        .unwrap();

    let (wake_tx, woken) = mpsc::channel();
    let (bridge, _task) = rt
        .block_on(async {
            let client = IpcClient::connect(server.endpoint()).await?;
            Bridge::attach(
                Connection::Attached(client),
                Origin::Tui,
                Arc::new(move || {
                    let _ = wake_tx.send(());
                }),
            )
            .await
        })
        .unwrap();
    let mut t = Attached {
        tui: Tui::with_clock(test_now),
        bridge,
        terminal: Terminal::new(TestBackend::new(80, 24)).unwrap(),
        woken,
    };
    t.settle();
    let screen = t.frame();
    assert!(
        screen.contains("Q3 numbers") && screen.contains("Lunch?"),
        "{screen}"
    );

    // `dail mcp`, attached too, archives a thread as the agent.
    let agent = rt.block_on(IpcClient::connect(server.endpoint())).unwrap();
    let archive = Command::ArchiveThreads {
        ids: vec![threads[0]],
        messages: Vec::new(),
    };
    rt.block_on(agent.submit(&archive, Origin::Mcp))
        .unwrap()
        .unwrap();
    let screen = t.wait_for("the archive", |s| !s.contains("Lunch?"));
    assert!(screen.contains("Q3 numbers"), "{screen}");

    // The agent drafts a reply and asks to send it; the TUI shows the banner.
    let message = state.threads[&threads[1]].messages[0];
    let reply = rt
        .block_on(agent.submit(
            &Command::Reply {
                message,
                body: Some("Thanks, Bob.".into()),
            },
            Origin::Mcp,
        ))
        .unwrap()
        .unwrap();
    let draft = serde_json::from_value(reply.data["id"].clone()).unwrap();
    rt.block_on(agent.submit(&Command::SendDraft { id: draft }, Origin::Mcp))
        .unwrap()
        .unwrap();
    t.wait_for("the banner", |s| {
        s.contains("An agent wants to send a message")
    });

    // The user reviews and approves it in the TUI.
    t.key(KeyCode::Char('k'), KeyModifiers::CONTROL);
    t.typed("show send requests");
    t.key(KeyCode::Enter, KeyModifiers::NONE);
    let screen = t.wait_for("the requests", |s| s.contains("Bob <bob@example.org>"));
    assert!(screen.contains("Thanks, Bob."), "{screen}");
    t.key(KeyCode::Char('a'), KeyModifiers::NONE);
    t.key(KeyCode::Char('y'), KeyModifiers::NONE);
    t.wait_for("the countdown", |s| s.contains("Sending in"));
    let owner = session.state();
    assert!(
        owner.approvals.is_empty(),
        "approved in the owner's session"
    );
    assert!(
        owner.drafts[&draft].send_at.is_some(),
        "scheduled with the undo window"
    );
    let threads_left = listing::threads_in(&owner, inbox).len();
    assert_eq!(threads_left, 1);

    drop(server);
    rt.block_on(session.shutdown()).unwrap();
    let _ = std::fs::remove_dir_all(profile);
}
