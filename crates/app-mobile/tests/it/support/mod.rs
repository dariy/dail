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
    seed: u128,
}

impl Phone {
    /// A `width`×`height` dp window over `state`.
    pub fn new(state: AppState, width_dp: u32, height_dp: u32) -> Self {
        let mut phone = Self {
            mobile: Mobile::with_clock(fixed_now),
            engine: Engine::new(state),
            out: Vec::new(),
            seed: 1000,
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
        let state = synced_state();
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
