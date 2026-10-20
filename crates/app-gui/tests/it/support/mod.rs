//! Shared helpers: sessions on the memory backend and headless frames.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use app_core::{AccountId, AppState, Command, Ctx, Engine, Origin, ProviderPreset, Secret};
use app_gui::{Bridge, Purpose, Reply, Request, Shell};
use app_sync::{Backend, CounterSeeds, Session, VirtualClock};
use app_testkit::{Fixture, MemoryBackend};
use egui_kittest::Harness;

/// A multi-threaded runtime for tests that run the bridge.
pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

/// A session on the `basic` fixture (alice@example.com), without accounts.
pub async fn session() -> (Arc<MemoryBackend>, Session) {
    let backend = Arc::new(MemoryBackend::from_fixture(
        Fixture::builtin("basic").unwrap(),
    ));
    let session = Session::open(
        Arc::clone(&backend) as Arc<dyn Backend>,
        Arc::new(VirtualClock::starting_at(1_700_000_000_000)),
        Arc::new(CounterSeeds::starting_at(1)),
    )
    .await
    .unwrap();
    (backend, session)
}

/// Runs a command to completion (effects included).
pub async fn run(session: &Session, cmd: Command) -> app_core::CommandReceipt {
    let receipt = session.submit(cmd, Origin::Cli).await.unwrap().unwrap();
    session.flush().await.unwrap();
    receipt
}

/// `add_account` for alice (Hostinger preset).
pub fn add_alice() -> Command {
    Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email: "alice@example.com".into(),
        display_name: None,
        password: Some(Secret::new("pw")),
        imap: None,
        smtp: None,
    }
}

/// A session with alice's account added and synced.
pub async fn synced() -> (Arc<MemoryBackend>, Session, AccountId) {
    let (backend, session) = session().await;
    let added = run(&session, add_alice()).await;
    let account = serde_json::from_value(added.data["id"].clone()).unwrap();
    run(&session, Command::SyncNow { account: None }).await;
    (backend, session, account)
}

/// The state of a synced session.
pub fn synced_state() -> AppState {
    runtime().block_on(async {
        let (_backend, session, _account) = synced().await;
        let state = session.state();
        (*state).clone()
    })
}

/// Waits (up to 5 s) until the bridge has a reply.
pub fn wait_for_replies(bridge: &Bridge) -> Vec<Reply> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let replies = bridge.replies();
        if !replies.is_empty() {
            return replies;
        }
        assert!(Instant::now() < deadline, "no reply within 5 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A fixed "now" for the shell: 2023-11-15 08:00 UTC, the day after the fixture's mail.
pub fn fixed_now() -> i64 {
    1_700_035_200_000
}

/// The UI under test, driven by a real `Engine` standing in for the session: requests are
/// dispatched synchronously with `Origin::Gui` (effects are dropped) and answered as replies.
pub struct Gui {
    pub shell: Shell,
    pub engine: Engine,
    pub replies: Vec<Reply>,
    /// Requests of the frames since the last [`pump`] / [`take`].
    pub out: Vec<Request>,
    pub seed: u128,
}

impl Gui {
    /// Dispatches every queued request and queues its reply for the next frame. Returns the
    /// dispatched user commands (background queries are dispatched but not returned).
    pub fn pump(&mut self) -> Vec<Command> {
        let out = std::mem::take(&mut self.out);
        let mut cmds = Vec::new();
        for Request { purpose, cmd } in out {
            self.seed += 1;
            let ctx = Ctx {
                now_ms: fixed_now(),
                id_seed: self.seed << 64,
                origin: Origin::Gui,
            };
            let name = cmd.command_name();
            if purpose == Purpose::User {
                cmds.push(cmd.clone());
            }
            let result = self.engine.dispatch(cmd, &ctx).map(|o| o.receipt);
            self.replies.push(Reply {
                purpose,
                name,
                result: Ok(result),
            });
        }
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
}

/// A kittest harness over [`Gui`], 1280×800.
pub fn gui(state: AppState) -> Harness<'static, Gui> {
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, gui: &mut Gui| {
                let replies = std::mem::take(&mut gui.replies);
                let queue = gui.shell.frame(ui, gui.engine.state(), replies);
                gui.out.extend(queue);
            },
            Gui {
                shell: Shell::with_clock(fixed_now),
                engine: Engine::new(state),
                replies: Vec::new(),
                out: Vec::new(),
                seed: 1000,
            },
        )
}

/// Runs `cmd` on the harness's engine directly (as another front-end would) and renders.
pub fn apply(h: &mut Harness<'static, Gui>, cmd: Command) -> app_core::CommandReceipt {
    let gui = h.state_mut();
    gui.seed += 1;
    let ctx = Ctx {
        now_ms: fixed_now(),
        id_seed: gui.seed << 64,
        origin: Origin::Cli,
    };
    let receipt = gui.engine.dispatch(cmd, &ctx).unwrap().receipt;
    h.run();
    receipt
}

/// The inbox and its newest thread in a synced state.
pub fn inbox_of(state: &AppState) -> (app_core::MailboxId, Vec<app_core::ThreadId>) {
    let inbox = state
        .mailboxes
        .values()
        .find(|m| m.role == Some(app_core::MailboxRole::Inbox))
        .unwrap()
        .id;
    let threads = app_core::listing::threads_in(state, inbox)
        .iter()
        .map(|t| t.id)
        .collect();
    (inbox, threads)
}

/// Feeds a runtime event to the harness's engine and renders.
pub fn ingest(h: &mut Harness<'static, Gui>, event: app_core::Event) {
    let gui = h.state_mut();
    gui.seed += 1;
    let ctx = Ctx {
        now_ms: fixed_now(),
        id_seed: gui.seed << 64,
        origin: Origin::Runtime,
    };
    gui.engine.ingest(event, &ctx).unwrap();
    h.run();
}

/// Waits (up to 5 s) until the bridge has a notice.
pub fn wait_for_notice(bridge: &Bridge) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let notices = bridge.notices();
        if !notices.is_empty() {
            return notices;
        }
        assert!(Instant::now() < deadline, "no notice within 5 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}
