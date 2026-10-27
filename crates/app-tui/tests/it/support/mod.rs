//! Shared helpers: the TUI over a real `Engine` (standing in for the session) on ratatui's
//! `TestBackend`, and buffer snapshots.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]
// reason: shared helpers; each test module uses a subset of them.
#![allow(dead_code)]

use app_core::{
    Addr, AppState, Command, Ctx, Engine, MailboxId, MailboxRole, Origin, Pane, ThreadId, listing,
};
use app_testkit::Fixture;
use app_tui::{Input, Purpose, Reply, Request, Tui};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A fixed "now" for the TUI: 2023-11-15 08:00 UTC, the day after the fixture's mail.
pub fn fixed_now() -> i64 {
    1_700_035_200_000
}

thread_local! {
    /// The TUI's clock in this test (each test runs on a thread of its own).
    static NOW: std::cell::Cell<i64> = const { std::cell::Cell::new(1_700_035_200_000) };
}

/// The test's clock, for `Tui::with_clock`.
pub fn test_now() -> i64 {
    NOW.with(std::cell::Cell::get)
}

/// alice@example.com's account, added and synced. The harness never answers effects, so
/// opening a mailbox must not start a refresh fetch (UX-6) that would stay in flight.
pub fn synced_state() -> AppState {
    let mut state = app_testkit::synced_state(Fixture::builtin("basic").unwrap()).unwrap();
    state.settings.stale_secs = 86_400;
    state
}

/// The inbox and its threads, newest first.
pub fn inbox_of(state: &AppState) -> (MailboxId, Vec<ThreadId>) {
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(MailboxRole::Inbox))
        .unwrap()
        .id;
    let threads = listing::threads_in(state, inbox)
        .iter()
        .map(|t| t.id)
        .collect();
    (inbox, threads)
}

/// The TUI under test: requests are dispatched synchronously with `Origin::Tui` (effects are
/// dropped) and answered as replies.
pub struct Term {
    pub tui: Tui,
    pub engine: Engine,
    pub terminal: Terminal<TestBackend>,
    /// Requests since the last [`Term::pump`] / [`Term::take`].
    pub out: Vec<Request>,
    seed: u128,
}

impl Term {
    /// A `width`×`height` terminal over `state`.
    pub fn new(state: AppState, width: u16, height: u16) -> Self {
        Self {
            tui: Tui::with_clock(test_now),
            engine: Engine::new(state),
            terminal: Terminal::new(TestBackend::new(width, height)).unwrap(),
            out: Vec::new(),
            seed: 1000,
        }
    }

    fn ctx(&mut self, origin: Origin) -> Ctx {
        self.seed += 1;
        Ctx {
            now_ms: fixed_now(),
            id_seed: self.seed << 64,
            origin,
        }
    }

    /// Draws a frame and returns the screen, one line per row (trailing spaces trimmed). Like
    /// the runner, it first queues the background requests the state calls for.
    pub fn screen(&mut self) -> String {
        let state = self.engine.state().clone();
        let background = self.tui.refresh(&state);
        self.out.extend(background);
        let tui = &mut self.tui;
        self.terminal.draw(|frame| tui.draw(frame, &state)).unwrap();
        let buffer = self.terminal.backend().buffer();
        let width = usize::from(buffer.area.width);
        let mut out = String::new();
        for row in buffer.content.chunks(width) {
            let line: String = row.iter().map(|cell| cell.symbol()).collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }

    /// Moves the TUI's clock on by `ms`, ticks, and draws.
    pub fn advance(&mut self, ms: i64) {
        NOW.with(|now| now.set(now.get() + ms));
        self.input(Input::Tick);
    }

    /// Feeds an input event, then draws.
    pub fn input(&mut self, input: Input) {
        let state = self.engine.state().clone();
        let requests = self.tui.input(input, &state);
        self.out.extend(requests);
        self.screen();
    }

    /// Presses a key without modifiers.
    pub fn key(&mut self, code: KeyCode) {
        self.input(Input::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    /// Presses a key with modifiers.
    pub fn chord(&mut self, modifiers: KeyModifiers, code: KeyCode) {
        self.input(Input::Key(KeyEvent::new(code, modifiers)));
    }

    /// Types text key by key.
    pub fn typed(&mut self, text: &str) {
        for c in text.chars() {
            self.key(KeyCode::Char(c));
        }
    }

    /// Dispatches every queued request and hands the replies to the TUI (and any follow-ups
    /// again). Returns the dispatched user commands.
    pub fn pump(&mut self) -> Vec<Command> {
        let mut cmds = Vec::new();
        for _ in 0..8 {
            let out = std::mem::take(&mut self.out);
            if out.is_empty() {
                break;
            }
            let mut replies = Vec::new();
            for Request { purpose, cmd } in out {
                let ctx = self.ctx(Origin::Tui);
                let name = cmd.command_name();
                if purpose == Purpose::User {
                    cmds.push(cmd.clone());
                }
                let result = self.engine.dispatch(cmd, &ctx).map(|o| o.receipt);
                replies.push(Reply {
                    tag: purpose,
                    name,
                    result: Ok(result),
                });
            }
            let state = self.engine.state().clone();
            let follow = self.tui.replies(replies, &state);
            self.out.extend(follow);
        }
        self.screen();
        cmds
    }

    /// Takes the queued user commands without dispatching them.
    pub fn take(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.out)
            .into_iter()
            .filter(|r| r.purpose == Purpose::User)
            .map(|r| r.cmd)
            .collect()
    }

    /// Runs `cmd` on the engine directly (as another front-end would) and draws.
    pub fn apply(&mut self, cmd: Command) -> app_core::CommandReceipt {
        let ctx = self.ctx(Origin::Cli);
        let receipt = self.engine.dispatch(cmd, &ctx).unwrap().receipt;
        self.screen();
        receipt
    }

    /// Feeds a runtime event to the engine and draws.
    pub fn ingest(&mut self, event: app_core::Event) {
        let ctx = self.ctx(Origin::Runtime);
        self.engine.ingest(event, &ctx).unwrap();
        self.screen();
    }
}

/// The `basic` fixture with `extra` more unread messages from Dave in the inbox (one thread each,
/// newest last), added and synced.
pub fn synced_with(extra: usize) -> AppState {
    let mut fixture = Fixture::builtin("basic").unwrap();
    let inbox = &mut fixture.accounts[0].mailboxes[0];
    for n in 0..extra {
        let raw = format!(
            "Message-ID: <extra{n}@example.org>\r\nFrom: Dave <dave@example.org>\r\nTo: alice@example.com\r\n\
             Subject: Note {n:02}\r\nDate: Mon, 13 Nov 2023 {:02}:{:02}:00 +0000\r\n\r\nNote {n}.\r\n",
            n / 60,
            n % 60
        );
        inbox.append(Vec::new(), raw);
    }
    app_testkit::synced_state(fixture).unwrap()
}

impl Term {
    /// Answers the queued `get_thread` body request for the open thread with `texts` (one per
    /// message, oldest first) as the session would, and draws.
    pub fn deliver_bodies(&mut self, texts: &[&str]) {
        let requests = std::mem::take(&mut self.out);
        let (bodies, rest): (Vec<Request>, Vec<Request>) = requests
            .into_iter()
            .partition(|r| matches!(r.purpose, Purpose::Bodies(_)));
        self.out = rest;
        let request = bodies.last().unwrap();
        let Purpose::Bodies(thread) = request.purpose else {
            unreachable!()
        };
        let messages: Vec<serde_json::Value> = self.engine.state().threads[&thread]
            .messages
            .iter()
            .zip(texts)
            .map(|(id, text)| {
                serde_json::json!({
                    "id": id,
                    "untrusted_content": {"body": {
                        "text": text,
                        "text_source": "plain",
                        "attachments": [
                            {"filename": "q3.pdf", "content_type": "application/pdf", "size": 120_000, "inline": false}
                        ],
                    }},
                })
            })
            .collect();
        let receipt = app_core::CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(app_core::CommandKind::Query),
            affected: Vec::new(),
            data: serde_json::json!({ "messages": messages }),
            undo_label: None,
        };
        let state = self.engine.state().clone();
        let follow = self.tui.replies(
            vec![Reply {
                tag: Purpose::Bodies(thread),
                name: app_core::CommandName::GetThread,
                result: Ok(Ok(receipt)),
            }],
            &state,
        );
        self.out.extend(follow);
        self.screen();
    }
}

fn dispatch(engine: &mut Engine, seed: u128, cmd: Command) -> app_core::CommandReceipt {
    let ctx = Ctx {
        now_ms: fixed_now(),
        id_seed: seed << 64,
        origin: Origin::Cli,
    };
    engine.dispatch(cmd, &ctx).unwrap().receipt
}

/// A terminal over a state where every command has something to act on: an open thread, a
/// pending op, an open draft with an attachment, a scheduled send and an agent's send request.
/// (Built on the terminal's own engine: a new `Engine` cancels sends scheduled before it.)
pub fn rich_term(width: u16, height: u16) -> Term {
    let mut t = Term::new(synced_state(), width, height);
    enrich(&mut t.engine);
    t.out.clear();
    t
}

fn enrich(engine: &mut Engine) {
    let state = engine.state().clone();
    let (inbox, threads) = inbox_of(&state);
    let account = *state.accounts.keys().next().unwrap();
    dispatch(engine, 501, Command::OpenMailbox { id: inbox });
    dispatch(engine, 502, Command::OpenThread { id: threads[0] });
    dispatch(
        engine,
        503,
        Command::MarkRead {
            ids: vec![threads[0]],
            messages: Vec::new(),
        },
    );
    let draft = |engine: &mut Engine, seed, to: Vec<Addr>| {
        let receipt = dispatch(
            engine,
            seed,
            Command::CreateDraft {
                account,
                to,
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Hi".into(),
                body: String::new(),
            },
        );
        serde_json::from_value(receipt.data["id"].clone()).unwrap()
    };
    let scheduled = draft(
        engine,
        504,
        vec![Addr {
            name: None,
            email: "bob@example.org".into(),
        }],
    );
    dispatch(engine, 505, Command::SendDraft { id: scheduled });
    let asked = draft(
        engine,
        510,
        vec![Addr {
            name: None,
            email: "carol@example.org".into(),
        }],
    );
    let agent = Ctx {
        now_ms: fixed_now(),
        id_seed: 511 << 64,
        origin: Origin::Mcp,
    };
    engine
        .dispatch(Command::SendDraft { id: asked }, &agent)
        .unwrap();
    let open = draft(engine, 506, Vec::new());
    dispatch(
        engine,
        507,
        Command::AttachFile {
            draft: open,
            path: "/tmp/a.txt".into(),
            index: None,
        },
    );
    dispatch(engine, 508, Command::OpenComposer { id: open });
    dispatch(engine, 509, Command::FocusPane { pane: Pane::List });
}
