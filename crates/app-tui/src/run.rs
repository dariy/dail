//! The terminal runner: sets the terminal up, connects [`Tui`] to the session through an
//! `app_sync::Bridge`, and restores the terminal however it ends.
//!
//! Everything the loop reacts to arrives on one queue ([`Event`]): keys, pastes and resizes from
//! an input thread, wake-ups from the bridge (state changes, replies), signals, and a 1 s tick
//! while a countdown or an age is on screen. After each wake-up the loop takes every queued event
//! and draws once, so a burst of changes is one redraw. A dead terminal (read or write fails),
//! SIGHUP or SIGTERM (closing the console on Windows) ends the loop like quitting: the bridge
//! closes and an owned session shuts down, saving unsaved drafts.

use std::io::{self, Stdout, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Once, PoisonError};
use std::time::Duration;

use app_core::Origin;
use app_sync::{Bridge, Connection, Session};
use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::TuiError;
use crate::editor::{self, Edited};
use crate::request::Purpose;
use crate::tui::{EditOutcome, External, Input, Tui};

/// How long closing waits for the session to save drafts and finish in-flight effects.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// How often the input thread checks whether it should stop.
const INPUT_POLL: Duration = Duration::from_millis(100);

/// How the TUI reaches the session.
pub enum Target {
    /// This process owns the session: it is shut down (drafts saved) when the TUI ends.
    Own(Session),
    /// Another process owns it (the GUI or `dail mcp`).
    Attach(Connection),
}

/// What the loop reacts to.
#[derive(Debug)]
pub enum Event {
    /// A key, paste or resize.
    Input(Input),
    /// The bridge has news (a state change or a reply).
    Wake,
    /// The terminal can't be read any more (the connection dropped).
    TerminalGone,
    /// SIGHUP, SIGTERM or the console being closed.
    Signal(&'static str),
}

/// Why the loop ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    /// The user quit.
    Quit,
    /// The session went away (an attached owner ended, or the session shut down).
    SessionGone,
    /// The terminal died or the process was asked to stop.
    Hangup(String),
}

/// What the loop needs from the terminal beyond drawing: running `$VISUAL`/`$EDITOR`.
pub trait Host {
    /// Lets the user edit `text` in their editor.
    fn edit(&mut self, text: &str) -> EditOutcome;

    /// Turns the terminal's mouse capture on or off.
    fn mouse(&mut self, on: bool);
}

/// The real terminal: pauses the input thread and leaves the alternate screen while the editor
/// runs.
struct TerminalHost {
    /// Held by the input thread while it polls or reads.
    gate: Arc<Mutex<()>>,
    /// Tells the input thread to stop taking the gate.
    paused: Arc<AtomicBool>,
    /// Whether mouse capture is on (it is taken back after the editor).
    mouse: bool,
}

impl Host for TerminalHost {
    fn edit(&mut self, text: &str) -> EditOutcome {
        let Some(command) = editor::configured() else {
            return EditOutcome::NoEditor;
        };
        // Stop reading input: the editor owns the terminal now. The flag keeps the input
        // thread from taking the gate again; the gate waits for a poll in progress.
        self.paused.store(true, Ordering::SeqCst);
        let gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        restore();
        let edited = editor::edit(&command, text);
        if let Err(err) = Screen::reenter(self.mouse) {
            tracing::warn!(error = %err, "could not take the terminal back");
        }
        drop(gate);
        self.paused.store(false, Ordering::SeqCst);
        match edited {
            Ok(Edited::Text(text)) => EditOutcome::Text(text),
            Ok(Edited::Failed(reason)) => EditOutcome::Failed(reason),
            Err(err) => EditOutcome::Failed(err.to_string()),
        }
    }

    fn mouse(&mut self, on: bool) {
        self.mouse = on;
        let mut out = io::stdout();
        let done = if on {
            execute!(out, EnableMouseCapture)
        } else {
            execute!(out, DisableMouseCapture)
        };
        if let Err(err) = done {
            tracing::debug!(error = %err, "mouse capture could not be changed");
        }
    }
}

/// Runs `tui` on `runtime` until the user quits, then restores the terminal and closes the
/// bridge (shutting an owned session down).
pub fn run(
    runtime: &tokio::runtime::Runtime,
    target: Target,
    mut tui: Tui,
) -> Result<Exit, TuiError> {
    let (tx, rx) = mpsc::channel::<Event>();
    let wake_tx = tx.clone();
    let wake: app_sync::Wake = Arc::new(move || {
        let _ = wake_tx.send(Event::Wake);
    });
    let (mut bridge, task) = match target {
        Target::Own(session) => {
            Bridge::<Purpose>::own(runtime.handle(), session, Origin::Tui, wake)
        }
        Target::Attach(conn) => runtime.block_on(Bridge::attach(conn, Origin::Tui, wake))?,
    };
    let signals = runtime.spawn(watch_signals(tx.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let gate = Arc::new(Mutex::new(()));
    let paused = Arc::new(AtomicBool::new(false));
    let exit = match Screen::enter() {
        Ok(screen) => {
            let input = spawn_input(
                tx,
                Arc::clone(&stop),
                Arc::clone(&gate),
                Arc::clone(&paused),
            );
            let mut host = TerminalHost {
                gate,
                paused,
                mouse: true,
            };
            let exit = match Terminal::new(CrosstermBackend::new(io::stdout())) {
                Ok(mut terminal) => event_loop(&mut terminal, &mut tui, &bridge, &rx, &mut host),
                Err(err) => Err(err.into()),
            };
            drop(screen);
            // Not joined: on a dead terminal crossterm's read may never return, and the session
            // must shut down (saving drafts) regardless. The thread stops at its next poll, or
            // with the process.
            stop.store(true, Ordering::SeqCst);
            drop(input);
            exit
        }
        Err(err) => Err(err.into()),
    };
    signals.abort();
    bridge.close();
    drop(bridge);
    // The timer must be created inside the runtime.
    match runtime.block_on(async { tokio::time::timeout(SHUTDOWN_TIMEOUT, task).await }) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::warn!(error = %err, "session task failed"),
        Err(_) => tracing::warn!("the session did not shut down within {SHUTDOWN_TIMEOUT:?}"),
    }
    exit
}

/// The loop: drains the queue, applies every event to `tui`, draws once per batch. Ends when
/// the user quits, the session goes away, the terminal dies or a signal arrives.
pub fn event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    tui: &mut Tui,
    bridge: &Bridge<Purpose>,
    events: &mpsc::Receiver<Event>,
    host: &mut dyn Host,
) -> Result<Exit, TuiError> {
    loop {
        let state = bridge.state();
        for request in tui.replies(bridge.replies(), &state) {
            bridge.submit(request.purpose, request.cmd);
        }
        for request in tui.refresh(&state) {
            bridge.submit(request.purpose, request.cmd);
        }
        if tui.quit_requested() {
            return Ok(Exit::Quit);
        }
        if bridge.is_gone() {
            return Ok(Exit::SessionGone);
        }
        if let Err(err) = terminal.draw(|frame| tui.draw(frame, &state)) {
            return Ok(Exit::Hangup(format!("the terminal can't be drawn: {err}")));
        }
        let first = if tui.wants_tick(&state) {
            match events.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Ok(Exit::Hangup("input closed".to_owned()));
                }
            }
        } else {
            match events.recv() {
                Ok(event) => Some(event),
                Err(_) => return Ok(Exit::Hangup("input closed".to_owned())),
            }
        };
        let batch: Vec<Event> = first.into_iter().chain(events.try_iter()).collect();
        if batch.is_empty() {
            for request in tui.input(Input::Tick, &state) {
                bridge.submit(request.purpose, request.cmd);
            }
        }
        for event in batch {
            match event {
                Event::Input(input) => {
                    // Each input sees the state as it is now (an earlier one may have changed
                    // the view through the session already).
                    let state = bridge.state();
                    for request in tui.input(input, &state) {
                        bridge.submit(request.purpose, request.cmd);
                    }
                }
                Event::Wake => {}
                Event::TerminalGone => {
                    return Ok(Exit::Hangup("the terminal went away".to_owned()));
                }
                Event::Signal(name) => return Ok(Exit::Hangup(format!("received {name}"))),
            }
        }
        match tui.take_external() {
            Some(External::EditBody { draft, text }) => {
                let outcome = host.edit(&text);
                // The editor drew over the screen: draw everything again.
                if let Err(err) = terminal.clear() {
                    return Ok(Exit::Hangup(format!("the terminal can't be drawn: {err}")));
                }
                for request in tui.edited(draft, outcome, &bridge.state()) {
                    bridge.submit(request.purpose, request.cmd);
                }
            }
            Some(External::MouseCapture(on)) => host.mouse(on),
            None => {}
        }
    }
}

/// Raw mode, the alternate screen and bracketed paste, restored on drop and on panic.
struct Screen;

static PANIC_HOOK: Once = Once::new();

impl Screen {
    /// Takes the terminal back after an external program (the editor) had it.
    fn reenter(mouse: bool) -> io::Result<()> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen)?;
        let _ = execute!(out, EnableBracketedPaste);
        if mouse {
            let _ = execute!(out, EnableMouseCapture);
        }
        Ok(())
    }

    fn enter() -> io::Result<Self> {
        PANIC_HOOK.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                restore();
                previous(info);
            }));
        });
        enable_raw_mode()?;
        let mut out = io::stdout();
        if let Err(err) = execute!(out, EnterAlternateScreen) {
            restore();
            return Err(err);
        }
        // Not every terminal (or the Windows console API) supports bracketed paste.
        if let Err(err) = execute!(out, EnableBracketedPaste) {
            tracing::debug!(error = %err, "bracketed paste is not available");
        }
        // Taps and swipes; `m` turns it off so that the terminal's selection can copy text.
        if let Err(err) = execute!(out, EnableMouseCapture) {
            tracing::debug!(error = %err, "mouse capture is not available");
        }
        Ok(Self)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        restore();
    }
}

/// Leaves the alternate screen and raw mode. Errors are ignored: the terminal may be gone.
fn restore() {
    let mut out: Stdout = io::stdout();
    let _ = execute!(
        out,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        ratatui::crossterm::cursor::Show
    );
    let _ = disable_raw_mode();
    let _ = out.flush();
}

/// Reads terminal events on a thread of its own until `stop` is set or the terminal fails.
fn spawn_input(
    tx: mpsc::Sender<Event>,
    stop: Arc<AtomicBool>,
    gate: Arc<Mutex<()>>,
    paused: Arc<AtomicBool>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("dail-tui-input".to_owned())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                // The editor holds the gate while it runs: no reading meanwhile.
                if paused.load(Ordering::SeqCst) {
                    std::thread::sleep(INPUT_POLL / 5);
                    continue;
                }
                let open = gate.lock().unwrap_or_else(PoisonError::into_inner);
                let event = match event::poll(INPUT_POLL) {
                    Ok(false) => continue,
                    Ok(true) => event::read(),
                    Err(err) => Err(err),
                };
                drop(open);
                let sent = match event {
                    Ok(event::Event::Key(key)) => tx.send(Event::Input(Input::Key(key))),
                    Ok(event::Event::Paste(text)) => tx.send(Event::Input(Input::Paste(text))),
                    Ok(event::Event::Resize(w, h)) => tx.send(Event::Input(Input::Resize(w, h))),
                    Ok(event::Event::Mouse(mouse)) => tx.send(Event::Input(Input::Mouse(mouse))),
                    Ok(_) => Ok(()),
                    Err(err) => {
                        tracing::info!(error = %err, "terminal input failed");
                        let _ = tx.send(Event::TerminalGone);
                        return;
                    }
                };
                if sent.is_err() {
                    return;
                }
            }
        })
}

/// Turns the signals that end a terminal session into [`Event::Signal`].
async fn watch_signals(tx: mpsc::Sender<Event>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let (Ok(mut hup), Ok(mut term)) = (
            signal(SignalKind::hangup()),
            signal(SignalKind::terminate()),
        ) else {
            tracing::warn!("cannot watch SIGHUP/SIGTERM");
            return;
        };
        let name = tokio::select! {
            _ = hup.recv() => "SIGHUP",
            _ = term.recv() => "SIGTERM",
        };
        let _ = tx.send(Event::Signal(name));
    }
    #[cfg(windows)]
    {
        let Ok(mut close) = tokio::signal::windows::ctrl_close() else {
            tracing::warn!("cannot watch the console's close event");
            return;
        };
        close.recv().await;
        let _ = tx.send(Event::Signal("console close"));
    }
}
