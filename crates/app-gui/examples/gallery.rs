//! The theme gallery: the type scale, the colour roles with their contrast, and every component
//! of the kit in its states, content and chrome side by side.
//! `cargo run -p app-gui --example gallery [-- --theme <id>]` (default `dail-dark`; the IDs are
//! `app_theme::builtin::all()`'s).

use app_gui::{theme, ui};

struct Gallery {
    theme: app_theme::Theme,
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::install_fonts(ui.ctx());
        let t = theme::Tokens::of(&self.theme);
        let dark = self.theme.appearance == app_theme::Appearance::Dark;
        ui.ctx().set_visuals(theme::visuals(&t.content, &t.m, dark));
        egui::CentralPanel::default().show(ui, |ui| ui::gallery::show(ui, &self.theme));
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id = args
        .iter()
        .position(|a| a == "--theme")
        .and_then(|i| args.get(i + 1))
        .map_or("dail-dark", String::as_str);
    let theme = app_theme::builtin::get(id).unwrap_or_else(app_theme::builtin::dark);
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("dail gallery — {}", theme.name))
            .with_inner_size([1280.0, 900.0]),
        ..Default::default()
    };
    options.glow_options.vsync = false;
    eframe::run_native(
        "dail-gallery",
        options,
        Box::new(move |_cc| Ok(Box::new(Gallery { theme }))),
    )
}
