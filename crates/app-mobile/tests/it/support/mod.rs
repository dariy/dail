//! Shared helpers: the presenter over a real `Engine` (standing in for the session).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]
// reason: shared helpers; each test module uses a subset of them.
#![allow(dead_code)]

use app_core::{AppState, Command, Ctx, Engine, MailboxId, MailboxRole, Origin, ThreadId, listing};
use app_mobile::{Frame, Mobile, Reply, Request, UiEvent};
use app_testkit::Fixture;

/// A fixed "now": 2023-11-15 08:00 UTC, the day after the fixture's mail.
pub const NOW: i64 = 1_700_035_200_000;

/// How far each test has moved its clock past [`NOW`], by thread (tests run on threads of
/// their own).
static LATER: std::sync::Mutex<Vec<(std::thread::ThreadId, i64)>> =
    std::sync::Mutex::new(Vec::new());

fn later() -> i64 {
    let me = std::thread::current().id();
    LATER
        .lock()
        .unwrap()
        .iter()
        .find(|(id, _)| *id == me)
        .map_or(0, |(_, ms)| *ms)
}

fn fixed_now() -> i64 {
    NOW + later()
}

/// Moves this test's presenter clock on by `ms`.
pub fn advance(ms: i64) {
    let me = std::thread::current().id();
    let mut all = LATER.lock().unwrap();
    match all.iter_mut().find(|(id, _)| *id == me) {
        Some((_, later)) => *later += ms,
        None => all.push((me, ms)),
    }
}

/// The window sizes of the snapshots (dp): a phone, a small tablet, a large tablet.
pub const SIZES: [(u32, u32); 3] = [(360, 800), (700, 1000), (1280, 800)];

/// alice@example.com's account, added and synced.
pub fn synced_state() -> AppState {
    app_testkit::synced_state(Fixture::builtin("basic").unwrap()).unwrap()
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

/// The presenter under test: requests are dispatched synchronously with `Origin::Gui` (effects
/// are dropped) and answered as replies, as the Android host does through its bridge.
pub struct Phone {
    pub mobile: Mobile,
    pub engine: Engine,
    /// Requests since the last [`Phone::take`].
    pub out: Vec<Request>,
    pub seed: u128,
    /// The bodies `get_thread` answers with, message by message (`untrusted_content.body`), in
    /// place of the engine's (which has no blobs).
    pub bodies: Option<Vec<serde_json::Value>>,
}

impl Phone {
    /// A `width`×`height` dp window over `state`.
    pub fn new(state: AppState, width_dp: u32, height_dp: u32) -> Self {
        let mut phone = Self {
            mobile: Mobile::with_clock(fixed_now),
            engine: Engine::new(state),
            out: Vec::new(),
            seed: 1000,
            bodies: None,
        };
        phone.event(UiEvent::Configure {
            width_dp,
            height_dp,
            dark: false,
            font_scale: 1.0,
            locale: Some("en-US".to_owned()),
            keyboard: false,
        });
        phone
    }

    /// The synced fixture with its inbox open.
    pub fn inbox(width_dp: u32, height_dp: u32) -> Self {
        let mut state = synced_state();
        // Effects are dropped here: a refresh-on-open fetch (UX-6) would never finish.
        state.settings.stale_secs = 86_400;
        let (inbox, _) = inbox_of(&state);
        let mut phone = Self::new(state, width_dp, height_dp);
        phone.dispatch(Command::OpenMailbox { id: inbox });
        phone
    }

    pub fn state(&self) -> &AppState {
        self.engine.state()
    }

    /// Dispatches a command as the app (`Origin::Gui`), outside the presenter.
    pub fn dispatch(&mut self, cmd: Command) {
        self.seed += 1;
        let ctx = Ctx {
            now_ms: NOW,
            id_seed: self.seed << 64,
            origin: Origin::Gui,
        };
        self.engine.dispatch(cmd, &ctx).unwrap();
    }

    /// The next frame, after the background requests the state calls for (answered).
    pub fn frame(&mut self) -> Frame {
        let state = self.engine.state().clone();
        let background = self.mobile.refresh(&state);
        self.submit(background);
        let state = self.engine.state().clone();
        self.mobile.frame(&state)
    }

    /// Feeds one event and submits what it asks for; returns its requests.
    pub fn event(&mut self, event: UiEvent) -> Vec<Request> {
        let state = self.engine.state().clone();
        let requests = self.mobile.event(event, &state);
        self.out.extend(requests.clone());
        self.submit(requests);
        self.out.clone()
    }

    /// Ingests a runtime event (an effect's result).
    pub fn ingest(&mut self, event: app_core::Event) {
        self.seed += 1;
        let ctx = Ctx {
            now_ms: NOW,
            id_seed: self.seed << 64,
            origin: Origin::Runtime,
        };
        self.engine.ingest(event, &ctx).unwrap();
    }

    /// The host actions asked for since the last call, but the background schedule (which the
    /// first frame and each change of the settings carry; `background.rs` tests it).
    pub fn externals(&mut self) -> Vec<app_mobile::External> {
        self.mobile
            .externals()
            .into_iter()
            .filter(|e| !matches!(e, app_mobile::External::Schedule { .. }))
            .collect()
    }

    /// Takes the requests submitted since the last call.
    pub fn take(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.out)
    }

    /// Taps an action of the current table.
    pub fn tap(&mut self, action: u32) -> Vec<Command> {
        self.take();
        self.event(UiEvent::Tap { action });
        requests_of(&self.take())
    }

    fn submit(&mut self, mut requests: Vec<Request>) {
        while !requests.is_empty() {
            let mut replies = Vec::new();
            for request in requests {
                self.seed += 1;
                let ctx = Ctx {
                    now_ms: NOW,
                    id_seed: self.seed << 64,
                    origin: Origin::Gui,
                };
                let name = request.cmd.command_name();
                if let (app_mobile::Purpose::Bodies(thread), Some(bodies)) =
                    (&request.purpose, &self.bodies)
                {
                    let messages: Vec<serde_json::Value> = self.engine.state().threads[thread]
                        .messages
                        .iter()
                        .zip(bodies)
                        .map(|(id, body)| {
                            serde_json::json!({"id": id, "untrusted_content": {"body": body}})
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
                    replies.push(Reply {
                        purpose: request.purpose,
                        name,
                        result: Ok(Ok(receipt)),
                    });
                    continue;
                }
                let result = self.engine.dispatch(request.cmd, &ctx).map(|o| o.receipt);
                replies.push(Reply {
                    purpose: request.purpose,
                    name,
                    result: Ok(result),
                });
            }
            let state = self.engine.state().clone();
            requests = self.mobile.replies(replies, &state);
            self.out.extend(requests.clone());
        }
    }
}

/// The commands of `requests`.
pub fn requests_of(requests: &[Request]) -> Vec<Command> {
    requests.iter().map(|r| r.cmd.clone()).collect()
}

/// A phone over a state where every command has something to act on: an open thread, a pending
/// op, an open draft with an attachment, a scheduled send and an agent's send request (like the
/// TUI's `rich_term`; built on the phone's own engine, since a new `Engine` cancels sends
/// scheduled before it).
pub fn rich_phone() -> Phone {
    let mut phone = Phone::inbox(360, 800);
    let state = phone.state().clone();
    let (_, threads) = inbox_of(&state);
    let account = *state.accounts.keys().next().unwrap();
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.dispatch(Command::MarkRead {
        ids: vec![threads[0]],
        messages: Vec::new(),
    });
    let draft = |phone: &mut Phone, to: &str, origin: Origin| -> app_core::DraftId {
        phone.seed += 1;
        let ctx = Ctx {
            now_ms: NOW,
            id_seed: phone.seed << 64,
            origin,
        };
        let receipt = phone
            .engine
            .dispatch(
                Command::CreateDraft {
                    account,
                    to: (!to.is_empty())
                        .then(|| app_core::Addr {
                            name: None,
                            email: to.to_owned(),
                        })
                        .into_iter()
                        .collect(),
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: "Hi".into(),
                    body: String::new(),
                },
                &ctx,
            )
            .unwrap()
            .receipt;
        let id = serde_json::from_value(receipt.data["id"].clone()).unwrap();
        if origin == Origin::Mcp {
            phone.seed += 1;
            let ctx = Ctx {
                id_seed: phone.seed << 64,
                ..ctx
            };
            phone
                .engine
                .dispatch(Command::SendDraft { id }, &ctx)
                .unwrap();
        }
        id
    };
    let scheduled = draft(&mut phone, "bob@example.org", Origin::Gui);
    phone.dispatch(Command::SendDraft { id: scheduled });
    draft(&mut phone, "carol@example.org", Origin::Mcp);
    let open = draft(&mut phone, "", Origin::Gui);
    phone.dispatch(Command::AttachFile {
        draft: open,
        path: "/tmp/a.txt".into(),
        index: None,
    });
    phone.dispatch(Command::OpenComposer { id: open });
    phone.dispatch(Command::FocusPane {
        pane: app_core::Pane::List,
    });
    phone.take();
    phone
}

/// A plain-text body as `get_thread` returns it.
pub fn plain_body(text: &str) -> serde_json::Value {
    serde_json::json!({"text": text, "text_source": "plain", "attachments": []})
}

/// An HTML body: its text and its document (`app_core::document`).
pub fn html_body(text: &str, document: &app_core::document::Document) -> serde_json::Value {
    serde_json::json!({
        "text": text,
        "text_source": "html",
        "document": document,
        "attachments": [],
    })
}

/// A document with every kind of block, a link and an inline image.
pub fn sample_document() -> app_core::document::Document {
    use app_core::document::{Block, Cell, Document, FoldKind, Span};
    let span = |text: &str| Span {
        text: text.to_owned(),
        ..Span::default()
    };
    Document {
        blocks: vec![
            Block::Heading {
                level: 2,
                spans: vec![span("Q3 numbers")],
            },
            Block::Paragraph {
                spans: vec![
                    span("See "),
                    Span {
                        text: "the report".to_owned(),
                        link: Some("https://example.org/q3".to_owned()),
                        bold: true,
                        ..Span::default()
                    },
                    Span {
                        text: "[chart]".to_owned(),
                        image: Some("chart@example.org".to_owned()),
                        ..Span::default()
                    },
                ],
            },
            Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![Block::Paragraph {
                    spans: vec![span("up 4%")],
                }]],
            },
            Block::Preformatted {
                text: "total  12".to_owned(),
            },
            Block::Table {
                rows: vec![vec![
                    Cell {
                        header: true,
                        spans: vec![span("Q")],
                    },
                    Cell {
                        header: false,
                        spans: vec![span("3")],
                    },
                ]],
            },
            Block::Rule,
            Block::Quote {
                blocks: vec![Block::Paragraph {
                    spans: vec![span("as said")],
                }],
            },
            Block::Fold {
                kind: FoldKind::Signature,
                blocks: vec![Block::Paragraph {
                    spans: vec![span("Carol")],
                }],
            },
        ],
    }
}

/// The fixture's newest thread open, its first message fetched (blob `b0`) with three
/// attachments and the sample document.
pub fn with_files() -> Phone {
    let phone = Phone::inbox(360, 800);
    let (_, threads) = inbox_of(phone.state());
    let mut state = phone.state().clone();
    let first = state.threads[&threads[0]].messages[0];
    state.messages.get_mut(&first).unwrap().body = app_core::BodyState::Fetched {
        blob: "b0".to_owned(),
    };
    let mut phone = Phone::new(state, 360, 800);
    let (inbox, _) = inbox_of(phone.state());
    phone.dispatch(Command::OpenMailbox { id: inbox });
    let mut body = html_body("Q3 numbers", &sample_document());
    body["attachments"] = serde_json::json!([
        {"part": 2, "filename": "chart.png", "content_type": "image/png", "size": 2048},
        {"part": 3, "filename": "notes.txt", "content_type": "text/plain", "size": 12},
        {"part": 4, "filename": "q3.pdf", "content_type": "application/pdf", "size": 120_000},
        {"part": 5, "filename": "logo.png", "content_type": "image/png", "size": 10, "inline": true},
    ]);
    phone.bodies = Some(vec![body]);
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone
}
