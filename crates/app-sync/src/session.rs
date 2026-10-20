//! `Session`: the single owner of the `Engine` (`ARCHITECTURE.md §5.1`).
//!
//! Commands and events are serialized through one queue into one engine task, so the engine needs
//! no locks. Store and network effects run in order on worker tasks: one per account for its
//! servers and keyring, one for everything local (store writes stay in order); timers
//! (`ScheduleTick`) and watches run concurrently. Results come back as events through the same
//! engine task.

use std::collections::HashMap;
use std::sync::Arc;

use app_core::{
    AccountId, AppState, Command, CommandReceipt, CoreError, Ctx, Effect, Engine, Event, Origin,
    ReplayInput, ReplayRecord,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::audit::{AuditLog, AuditWriter};
use crate::backend::Backend;
use crate::clock::{Clock, SeedSource};
use crate::error::SyncError;
use crate::executor::run_effect;

/// Queue depth of the input channel.
const INPUT_CAPACITY: usize = 256;

/// Result of one submitted command: the engine's receipt or its structured rejection.
pub type SubmitResult = Result<CommandReceipt, CoreError>;

enum Input {
    Command {
        cmd: Command,
        origin: Origin,
        reply: oneshot::Sender<SubmitResult>,
    },
    Event(Event),
    Flush(oneshot::Sender<()>),
    Preview {
        cmd: Command,
        origin: Origin,
        reply: oneshot::Sender<PreviewResult>,
    },
}

/// The result of [`SessionClient::preview`]: what the command would do, or the rejection.
pub type PreviewResult = Result<app_core::Preview, CoreError>;

/// The owner of a running session. Dropping it (or calling [`Session::shutdown`]) stops the
/// engine task after in-flight effects complete; [`SessionClient`]s handed out by
/// [`Session::client`] then see [`SyncError::SessionClosed`].
pub struct Session {
    client: SessionClient,
    /// Dropping it tells the engine loop to stop taking inputs.
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

/// A cheap, cloneable handle that submits to a running session (e.g. for each IPC connection or
/// the MCP server). It does not keep the session alive: once the owner shuts it down, every call
/// fails with [`SyncError::SessionClosed`].
#[derive(Clone)]
pub struct SessionClient {
    backend: Arc<dyn Backend>,
    input: mpsc::Sender<Input>,
    state: watch::Receiver<Arc<AppState>>,
}

impl Session {
    /// Loads the persisted state from `backend` and starts the engine task. Must be called inside
    /// a tokio runtime.
    pub async fn open(
        backend: Arc<dyn Backend>,
        clock: Arc<dyn Clock>,
        seeds: Arc<dyn SeedSource>,
    ) -> Result<Self, SyncError> {
        Self::open_audited(backend, clock, seeds, None).await
    }

    /// Like [`Session::open`], and with `audit` every command and event the engine sees is
    /// appended to that audit log as a redacted replay record.
    pub async fn open_audited(
        backend: Arc<dyn Backend>,
        clock: Arc<dyn Clock>,
        seeds: Arc<dyn SeedSource>,
        audit: Option<AuditLog>,
    ) -> Result<Self, SyncError> {
        let initial = backend.load_state().await?.unwrap_or_default();
        // Publish what the engine starts from (`Engine::new` normalizes the snapshot, e.g.
        // cancels sends that were scheduled before a restart), not the raw snapshot.
        let engine = Engine::new(initial);
        let (input_tx, input_rx) = mpsc::channel(INPUT_CAPACITY);
        let (state_tx, state_rx) = watch::channel(Arc::new(engine.state().clone()));
        let (stop_tx, stop_rx) = oneshot::channel();
        let audit = audit.map(|log| log.start(clock.now_ms()));
        let task = tokio::spawn(
            EngineLoop {
                engine,
                backend: Arc::clone(&backend),
                clock,
                seeds,
                state_tx,
                audit,
            }
            .run(input_rx, stop_rx),
        );
        Ok(Self {
            client: SessionClient {
                backend,
                input: input_tx,
                state: state_rx,
            },
            stop: stop_tx,
            task,
        })
    }

    /// A handle that submits to this session without owning it.
    pub fn client(&self) -> SessionClient {
        self.client.clone()
    }

    /// Dispatches a command; see [`SessionClient::submit`].
    pub async fn submit(&self, cmd: Command, origin: Origin) -> Result<SubmitResult, SyncError> {
        self.client.submit(cmd, origin).await
    }

    /// Feeds an external event (e.g. from tests) into the engine.
    pub async fn ingest(&self, event: Event) -> Result<(), SyncError> {
        self.client.ingest(event).await
    }

    /// A dry run; see [`SessionClient::preview`].
    pub async fn preview(&self, cmd: Command, origin: Origin) -> Result<PreviewResult, SyncError> {
        self.client.preview(cmd, origin).await
    }

    /// Waits for in-flight effects; see [`SessionClient::flush`].
    pub async fn flush(&self) -> Result<(), SyncError> {
        self.client.flush().await
    }

    /// See [`SessionClient::open_original`].
    pub async fn open_original(&self, blob: &str) -> Result<Option<std::path::PathBuf>, SyncError> {
        self.client.open_original(blob).await
    }

    /// The current state.
    pub fn state(&self) -> Arc<AppState> {
        self.client.state()
    }

    /// A receiver that is notified on every state change (`StateChanged`).
    pub fn subscribe(&self) -> watch::Receiver<Arc<AppState>> {
        self.client.subscribe()
    }

    /// Tells the engine the front-end is closing (drafts with unsaved changes are saved now),
    /// completes in-flight effects, then stops the engine task. Inputs that clients submit
    /// meanwhile are refused with [`SyncError::SessionClosed`].
    pub async fn shutdown(self) -> Result<(), SyncError> {
        self.client.ingest(Event::Closing {}).await?;
        self.client.flush().await?;
        drop(self.stop);
        drop(self.client);
        self.task.await.map_err(|_| SyncError::SessionClosed)
    }
}

impl std::fmt::Debug for SessionClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionClient")
            .field("backend", &self.backend.name())
            .finish_non_exhaustive()
    }
}

impl SessionClient {
    /// Dispatches a command. The outer error means the session is gone; the inner one is the
    /// engine's rejection. Message bodies requested with `include_body` are read from the store
    /// and added to the receipt ([`crate::bodies`]).
    pub async fn submit(&self, cmd: Command, origin: Origin) -> Result<SubmitResult, SyncError> {
        let bodies = match cmd {
            Command::GetThread {
                include_body: true,
                rich_body,
                ..
            } => Some(rich_body),
            Command::GetMessage {
                include_body: true, ..
            } => Some(false),
            _ => None,
        };
        let (reply, response) = oneshot::channel();
        self.input
            .send(Input::Command { cmd, origin, reply })
            .await
            .map_err(|_| SyncError::SessionClosed)?;
        let mut result = response.await.map_err(|_| SyncError::SessionClosed)?;
        if let (Some(rich), Ok(receipt)) = (bodies, &mut result) {
            crate::bodies::attach_bodies(self.backend.as_ref(), &mut receipt.data, rich).await;
        }
        Ok(result)
    }

    /// A dry run (`dry_run: true`): what `cmd` would do, from a copy of the engine
    /// ([`app_core::Engine::preview`]). Nothing changes, no effect runs, nothing is logged.
    pub async fn preview(&self, cmd: Command, origin: Origin) -> Result<PreviewResult, SyncError> {
        let (reply, response) = oneshot::channel();
        self.input
            .send(Input::Preview { cmd, origin, reply })
            .await
            .map_err(|_| SyncError::SessionClosed)?;
        response.await.map_err(|_| SyncError::SessionClosed)
    }

    /// Feeds an external event (e.g. from IPC or tests) into the engine.
    pub async fn ingest(&self, event: Event) -> Result<(), SyncError> {
        self.input
            .send(Input::Event(event))
            .await
            .map_err(|_| SyncError::SessionClosed)
    }

    /// Waits until every store/network effect issued so far (and every effect those caused) has
    /// completed and been ingested. Pending timers are not waited for.
    pub async fn flush(&self) -> Result<(), SyncError> {
        let (reply, done) = oneshot::channel();
        self.input
            .send(Input::Flush(reply))
            .await
            .map_err(|_| SyncError::SessionClosed)?;
        done.await.map_err(|_| SyncError::SessionClosed)
    }

    /// Writes the sanitized "open original" page of a stored message body (`blob`, from the
    /// envelope's `body: fetched`) to a private temporary file and returns its path; `None` if
    /// the store has no such blob. Reads only; the engine is not involved.
    pub async fn open_original(&self, blob: &str) -> Result<Option<std::path::PathBuf>, SyncError> {
        let Some(raw) = self.backend.get_blob(blob).await? else {
            return Ok(None);
        };
        let html = app_io::original::original_html(&raw);
        crate::original::write_page(html).await.map(Some)
    }

    /// The part `Content-ID: <cid>` of a stored message body (`blob`): an inline image the
    /// HTML shows as `cid:…`. `None` if the store has no such blob or the message no such part.
    /// Reads only; the engine is not involved.
    pub async fn inline_part(
        &self,
        blob: &str,
        cid: &str,
    ) -> Result<Option<app_io::mime::InlinePart>, SyncError> {
        let Some(raw) = self.backend.get_blob(blob).await? else {
            return Ok(None);
        };
        Ok(app_io::mime::inline_part(&raw, cid))
    }

    /// Attachment `part` of a stored message body (`blob`) with its safe file name, for a
    /// preview. `None` if the store has no such blob or the message no such attachment. Reads
    /// only; the engine is not involved.
    pub async fn attachment_part(
        &self,
        blob: &str,
        part: usize,
    ) -> Result<Option<(String, app_io::mime::InlinePart)>, SyncError> {
        let Some(raw) = self.backend.get_blob(blob).await? else {
            return Ok(None);
        };
        Ok(app_io::mime::attachment_part(&raw, part))
    }

    /// Writes a private temporary copy of attachment `part` of a stored body (for the system
    /// viewer) and returns its path; `None` if there is no such blob or attachment.
    pub async fn open_attachment(
        &self,
        blob: &str,
        part: usize,
    ) -> Result<Option<std::path::PathBuf>, SyncError> {
        let Some((name, part)) = self.attachment_part(blob, part).await? else {
            return Ok(None);
        };
        crate::original::write_copy(&name, &part.bytes)
            .await
            .map(Some)
    }

    /// The current state.
    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state.borrow())
    }

    /// A receiver that is notified on every state change (`StateChanged`).
    pub fn subscribe(&self) -> watch::Receiver<Arc<AppState>> {
        self.state.clone()
    }
}

struct EngineLoop {
    engine: Engine,
    backend: Arc<dyn Backend>,
    clock: Arc<dyn Clock>,
    seeds: Arc<dyn SeedSource>,
    state_tx: watch::Sender<Arc<AppState>>,
    audit: Option<AuditWriter>,
}

impl EngineLoop {
    async fn run(mut self, mut input: mpsc::Receiver<Input>, mut stop: oneshot::Receiver<()>) {
        let (done_tx, mut done_rx) = mpsc::unbounded_channel::<Vec<Event>>();
        let mut lanes = Lanes::new(Arc::clone(&self.backend), Arc::clone(&self.clock), done_tx);
        let mut timers: JoinSet<Vec<Event>> = JoinSet::new();
        let mut in_flight: usize = 0;
        let mut flush_waiters: Vec<oneshot::Sender<()>> = Vec::new();
        let mut input_open = true;

        loop {
            if in_flight == 0 {
                for waiter in flush_waiters.drain(..) {
                    let _ = waiter.send(());
                }
                if !input_open {
                    break;
                }
            }
            let effects = tokio::select! {
                next = input.recv(), if input_open => match next {
                    Some(Input::Command { cmd, origin, reply }) => {
                        let (result, effects) = self.dispatch(cmd, origin);
                        let _ = reply.send(result);
                        effects
                    }
                    Some(Input::Event(event)) => self.ingest(event),
                    Some(Input::Preview { cmd, origin, reply }) => {
                        let ctx = self.ctx(origin);
                        let _ = reply.send(self.engine.preview(cmd, &ctx));
                        Vec::new()
                    }
                    Some(Input::Flush(waiter)) => {
                        flush_waiters.push(waiter);
                        Vec::new()
                    }
                    None => {
                        input_open = false;
                        Vec::new()
                    }
                },
                Some(events) = done_rx.recv() => {
                    in_flight = in_flight.saturating_sub(1);
                    events.into_iter().flat_map(|event| self.ingest(event)).collect()
                }
                Some(joined) = timers.join_next(), if !timers.is_empty() => {
                    match joined {
                        Ok(events) => events.into_iter().flat_map(|event| self.ingest(event)).collect(),
                        Err(err) => {
                            tracing::warn!(error = %err, "timer task failed");
                            Vec::new()
                        }
                    }
                }
                // The owner is gone or shutting down: refuse further inputs.
                _ = &mut stop, if input_open => {
                    input_open = false;
                    input.close();
                    Vec::new()
                }
                // Input closed and the worker is gone: nothing can make progress any more.
                else => break,
            };
            for effect in effects {
                match effect {
                    // Timers and watches wait for the world, so they run beside the worker and
                    // `flush` does not wait for them.
                    Effect::ScheduleTick { .. } | Effect::WatchMailbox { .. } => {
                        let backend = Arc::clone(&self.backend);
                        let clock = Arc::clone(&self.clock);
                        timers.spawn(async move {
                            run_effect(backend.as_ref(), clock.as_ref(), effect).await
                        });
                    }
                    effect => {
                        if lanes.send(effect) {
                            in_flight += 1;
                        } else {
                            tracing::error!("effect worker stopped; effect dropped");
                        }
                    }
                }
            }
        }
        timers.abort_all();
        lanes.close().await;
        if let Some(audit) = self.audit.take() {
            audit.finish().await;
        }
    }

    fn ctx(&self, origin: Origin) -> Ctx {
        Ctx {
            now_ms: self.clock.now_ms(),
            id_seed: self.seeds.next_seed(),
            origin,
        }
    }

    fn dispatch(&mut self, cmd: Command, origin: Origin) -> (SubmitResult, Vec<Effect>) {
        let ctx = self.ctx(origin);
        if let Some(audit) = &self.audit {
            audit.record(&ReplayRecord::new(&ctx, ReplayInput::Command(cmd.clone())));
        }
        match self.engine.dispatch(cmd, &ctx) {
            Ok(outcome) => {
                self.publish();
                (Ok(outcome.receipt), outcome.effects)
            }
            Err(err) => (Err(err), Vec::new()),
        }
    }

    fn ingest(&mut self, event: Event) -> Vec<Effect> {
        let ctx = self.ctx(Origin::Runtime);
        if let Some(audit) = &self.audit {
            audit.record(&ReplayRecord::new(&ctx, ReplayInput::Event(event.clone())));
        }
        let name = event.name();
        match self.engine.ingest(event, &ctx) {
            Ok(outcome) => {
                self.publish();
                outcome.effects
            }
            Err(err) => {
                tracing::warn!(event = name, error = %err, "event rejected");
                Vec::new()
            }
        }
    }

    fn publish(&self) {
        let state = self.engine.state();
        if self.state_tx.borrow().as_ref() != state {
            self.state_tx.send_replace(Arc::new(state.clone()));
        }
    }
}

/// Which worker runs an effect: each account's server and keyring effects in order on its own
/// worker, so that a slow or unreachable server delays only its account; everything local
/// (store writes, files, the index) in order on one more; lookups of other servers (autoconfig)
/// on their own, as they may take seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Lane {
    Local,
    Lookup,
    Account(AccountId),
}

fn lane(effect: &Effect) -> Lane {
    match effect {
        Effect::VerifyAccount { account, .. }
        | Effect::StoreSecret { account, .. }
        | Effect::DeleteSecret { account, .. }
        | Effect::ListMailboxes { account, .. }
        | Effect::FetchEnvelopes { account, .. }
        | Effect::ResyncFlags { account, .. }
        | Effect::FetchBody { account, .. }
        | Effect::FetchOlder { account, .. }
        | Effect::RunRemoteOp { account, .. }
        | Effect::SaveDraft { account, .. }
        | Effect::DeleteDraft { account, .. }
        | Effect::SearchMailbox { account, .. }
        | Effect::SmtpSend { account, .. }
        | Effect::WatchMailbox { account, .. } => Lane::Account(*account),
        Effect::PersistState { .. }
        | Effect::ScheduleTick { .. }
        | Effect::InspectAttachment { .. }
        | Effect::SaveAttachments { .. }
        | Effect::CompactStore { .. }
        | Effect::SearchIndex { .. } => Lane::Local,
        Effect::DiscoverServers { .. } => Lane::Lookup,
    }
}

/// The effect workers, one per [`Lane`], started on first use.
struct Lanes {
    backend: Arc<dyn Backend>,
    clock: Arc<dyn Clock>,
    done: mpsc::UnboundedSender<Vec<Event>>,
    senders: HashMap<Lane, mpsc::UnboundedSender<Effect>>,
    workers: Vec<JoinHandle<()>>,
}

impl Lanes {
    fn new(
        backend: Arc<dyn Backend>,
        clock: Arc<dyn Clock>,
        done: mpsc::UnboundedSender<Vec<Event>>,
    ) -> Self {
        Self {
            backend,
            clock,
            done,
            senders: HashMap::new(),
            workers: Vec::new(),
        }
    }

    /// Queues `effect` on its lane; `false` if that worker is gone.
    fn send(&mut self, effect: Effect) -> bool {
        let key = lane(&effect);
        let sender = self.senders.entry(key).or_insert_with(|| {
            let (tx, rx) = mpsc::unbounded_channel();
            self.workers.push(tokio::spawn(worker(
                Arc::clone(&self.backend),
                Arc::clone(&self.clock),
                rx,
                self.done.clone(),
            )));
            tx
        });
        sender.send(effect).is_ok()
    }

    /// Lets every worker finish what it has, then waits for them.
    async fn close(self) {
        drop(self.senders);
        for worker in self.workers {
            if let Err(err) = worker.await {
                tracing::warn!(error = %err, "effect worker failed");
            }
        }
    }
}

/// Runs one lane's effects one at a time, in order.
async fn worker(
    backend: Arc<dyn Backend>,
    clock: Arc<dyn Clock>,
    mut work: mpsc::UnboundedReceiver<Effect>,
    done: mpsc::UnboundedSender<Vec<Event>>,
) {
    while let Some(effect) = work.recv().await {
        let events = run_effect(backend.as_ref(), clock.as_ref(), effect).await;
        if done.send(events).is_err() {
            break;
        }
    }
}
