//! `DailApp`: the eframe application. Owns the [`Bridge`] and the [`Shell`].

use std::sync::Arc;

use app_core::AppState;

use crate::bridge::Bridge;
use crate::desktop::Notifier;
use crate::shell::{External, Shell};

/// The eframe application.
pub struct DailApp {
    bridge: Bridge,
    shell: Shell,
    /// The state of the last frame, to notice what arrived since.
    last: Option<Arc<AppState>>,
    notifier: Notifier,
}

impl DailApp {
    /// An app over a started bridge.
    pub fn new(bridge: Bridge) -> Self {
        let mut shell = Shell::default();
        shell.set_lang(app_front::i18n::Lang::from_tag(
            std::env::var("DAIL_LANG").ok().as_deref(),
        ));
        Self {
            bridge,
            shell,
            last: None,
            notifier: Notifier::start(),
        }
    }
}

impl eframe::App for DailApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Frame discipline (AGENTS.md §5): render from one immutable snapshot, collect the
        // commands, and only submit them once the frame is done.
        let state = self.bridge.state();
        // Desktop notifications while the window is in the background.
        if let Some(last) = self.last.replace(Arc::clone(&state))
            && !Arc::ptr_eq(&last, &state)
            && ui.ctx().input(|i| i.viewport().focused) == Some(false)
        {
            for alert in app_front::alerts::alerts(&last, &state) {
                self.notifier.show(alert);
            }
        }
        let replies = self.bridge.replies();
        for notice in self.bridge.notices() {
            self.shell.notify(notice);
        }
        for (key, image) in self.bridge.images() {
            self.shell.accept_image(ui.ctx(), key, image);
        }
        for (key, preview) in self.bridge.previews() {
            self.shell.accept_preview(ui.ctx(), key, preview);
        }
        let queue = self.shell.frame(ui, &state, replies);
        for request in queue {
            self.bridge.submit(request);
        }
        for external in self.shell.take_external() {
            match external {
                External::OpenUrl(url) => open_url(url),
                External::OpenOriginal { blob } => self.bridge.open_original(blob),
                External::LoadImage(key) => self.bridge.load_image(key),
                External::LoadPreview(key) => self.bridge.load_preview(key),
                External::OpenAttachment(key) => self.bridge.open_attachment(key),
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.bridge.close();
    }
}

/// Opens a link in the system browser on a helper thread (it can block).
fn open_url(url: String) {
    let spawned = std::thread::Builder::new()
        .name("dail-open-url".to_owned())
        .spawn(move || {
            if let Err(err) = webbrowser::open(&url) {
                tracing::warn!(error = %err, "could not open the browser");
            }
        });
    if let Err(err) = spawned {
        tracing::warn!(error = %err, "could not open the browser");
    }
}
