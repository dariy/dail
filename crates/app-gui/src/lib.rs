//! dail desktop GUI (egui/eframe). A projection of `AppState` and an emitter of `Command`s
//! (`ARCHITECTURE.md §7.1`).
//!
//! The [`Session`] runs on a background tokio runtime behind a [`Bridge`]; the UI thread renders
//! from state snapshots, queues commands during a frame and submits them after it.

pub mod app;
pub mod approvals;
pub mod attachments;
pub mod bridge;
pub mod composer;
pub mod desktop;
pub mod document;
pub mod images;
pub mod keymap;
pub mod list;
pub mod palette;
pub mod reader;
pub mod shell;
pub mod sidebar;
pub mod theme;
pub mod toasts;
pub mod topbar;
pub mod ui;
pub mod wizard;

use std::time::Duration;

use app_sync::Session;

pub use app::DailApp;
pub use bridge::{Background, Bridge, Purpose, Reply, Request};
pub use shell::{External, Shell};

/// How long closing waits for the session to save drafts and finish in-flight effects.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// GUI failure.
#[derive(Debug, thiserror::Error)]
pub enum GuiError {
    /// The window or the graphics context could not be created.
    #[error("GUI: {0}")]
    Window(String),
    /// The session did not shut down in time.
    #[error("session did not shut down within {0:?}")]
    ShutdownTimeout(Duration),
}

/// Opens the main window over `session` and blocks until it is closed. The session runs on
/// `runtime`; it is shut down (unsaved drafts saved) before this returns.
pub fn run(runtime: &tokio::runtime::Runtime, session: Session) -> Result<(), GuiError> {
    let ctx = egui::Context::default();
    let (bridge, task) = Bridge::start(runtime.handle(), session, ctx.clone());
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("dail")
            .with_app_id("dail")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    // egui repaints only on demand, so vsync buys nothing; on some Wayland compositors (KDE)
    // eglSwapBuffers then blocks on frame callbacks that never come and the window is reported
    // as not responding. `DAIL_VSYNC=1` turns it back on.
    options.glow_options.vsync = std::env::var_os("DAIL_VSYNC").is_some();
    let result = eframe::run_native_ext(
        "dail",
        options,
        Some(ctx),
        Box::new(move |_cc| Ok(Box::new(DailApp::new(bridge)))),
    );
    // The app (and with it the bridge) is gone now, so the session task shuts down.
    // The timer must be created inside the runtime (`timeout` panics outside of one).
    let shutdown = runtime.block_on(async { tokio::time::timeout(SHUTDOWN_TIMEOUT, task).await });
    result.map_err(|err| GuiError::Window(err.to_string()))?;
    match shutdown {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "session task failed");
            Ok(())
        }
        Err(_) => Err(GuiError::ShutdownTimeout(SHUTDOWN_TIMEOUT)),
    }
}
