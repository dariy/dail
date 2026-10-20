//! The monochrome look (`docs/ui/monochrome_minimalist_mail/DESIGN.md`): tonal surfaces,
//! hairline borders and a single accent, the ink colour itself (black on light, white on dark).

use egui::{Color32, CornerRadius, Stroke, Theme, ThemePreference, Visuals};

/// The colours of one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Reading area and inputs.
    pub canvas: Color32,
    /// Sidebar, list and bars.
    pub surface: Color32,
    /// Selected rows, active inputs.
    pub subtle: Color32,
    /// Hover, badges.
    pub muted: Color32,
    /// Hairline dividers.
    pub border: Color32,
    /// Focus rings.
    pub border_strong: Color32,
    /// Headings, unread rows. Also the accent.
    pub ink: Color32,
    /// Body text and read rows.
    pub secondary: Color32,
    /// Timestamps, hints, disabled.
    pub faint: Color32,
    /// Text on an ink fill.
    pub on_ink: Color32,
}

/// Light palette.
pub const LIGHT: Palette = Palette {
    canvas: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface: Color32::from_rgb(0xFA, 0xFA, 0xFA),
    subtle: Color32::from_rgb(0xF4, 0xF4, 0xF5),
    muted: Color32::from_rgb(0xE4, 0xE4, 0xE7),
    border: Color32::from_rgb(0xE4, 0xE4, 0xE7),
    border_strong: Color32::from_rgb(0xA1, 0xA1, 0xAA),
    ink: Color32::from_rgb(0x09, 0x09, 0x0B),
    secondary: Color32::from_rgb(0x52, 0x52, 0x5B),
    faint: Color32::from_rgb(0x8A, 0x8A, 0x93),
    on_ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
};

/// Dark palette.
pub const DARK: Palette = Palette {
    canvas: Color32::from_rgb(0x00, 0x00, 0x00),
    surface: Color32::from_rgb(0x09, 0x09, 0x0B),
    subtle: Color32::from_rgb(0x16, 0x16, 0x18),
    muted: Color32::from_rgb(0x22, 0x22, 0x25),
    border: Color32::from_rgb(0x22, 0x22, 0x22),
    border_strong: Color32::from_rgb(0x3F, 0x3F, 0x46),
    ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    secondary: Color32::from_rgb(0xA1, 0xA1, 0xAA),
    faint: Color32::from_rgb(0x71, 0x71, 0x7A),
    on_ink: Color32::from_rgb(0x00, 0x00, 0x00),
};

/// Account tags: hues that read on the light and the dark surfaces alike.
const ACCOUNT_COLORS: [Color32; 6] = [
    Color32::from_rgb(0x3B, 0x82, 0xF6),
    Color32::from_rgb(0xE1, 0x1D, 0x48),
    Color32::from_rgb(0x10, 0xB9, 0x81),
    Color32::from_rgb(0xF5, 0x9E, 0x0B),
    Color32::from_rgb(0x8B, 0x5C, 0xF6),
    Color32::from_rgb(0x06, 0xB6, 0xD4),
];

/// The colour tag of `account`: by its position among the accounts.
pub fn account_color(state: &app_core::AppState, account: app_core::AccountId) -> Color32 {
    let index = state.accounts.get_index_of(&account).unwrap_or(0);
    ACCOUNT_COLORS[index % ACCOUNT_COLORS.len()]
}

/// High-contrast dark palette (`theme` 3): white on black, borders that show.
pub const HIGH_CONTRAST_DARK: Palette = Palette {
    canvas: Color32::from_rgb(0x00, 0x00, 0x00),
    surface: Color32::from_rgb(0x00, 0x00, 0x00),
    subtle: Color32::from_rgb(0x1F, 0x1F, 0x1F),
    muted: Color32::from_rgb(0x38, 0x38, 0x38),
    border: Color32::from_rgb(0xB0, 0xB0, 0xB0),
    border_strong: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    secondary: Color32::from_rgb(0xF2, 0xF2, 0xF2),
    faint: Color32::from_rgb(0xD4, 0xD4, 0xD4),
    on_ink: Color32::from_rgb(0x00, 0x00, 0x00),
};

/// High-contrast light palette (`theme` 4): black on white.
pub const HIGH_CONTRAST_LIGHT: Palette = Palette {
    canvas: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    subtle: Color32::from_rgb(0xEB, 0xEB, 0xEB),
    muted: Color32::from_rgb(0xD6, 0xD6, 0xD6),
    border: Color32::from_rgb(0x4D, 0x4D, 0x4D),
    border_strong: Color32::from_rgb(0x00, 0x00, 0x00),
    ink: Color32::from_rgb(0x00, 0x00, 0x00),
    secondary: Color32::from_rgb(0x0D, 0x0D, 0x0D),
    faint: Color32::from_rgb(0x2E, 0x2E, 0x2E),
    on_ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
};

/// The light and dark palettes of a `theme` setting, and the preference between them:
/// 0 = follow the system, 1 = light, 2 = dark, 3 = high-contrast dark, 4 = high-contrast light.
pub const fn palettes(setting: u64) -> (Palette, Palette, ThemePreference) {
    match setting {
        1 => (LIGHT, DARK, ThemePreference::Light),
        2 => (LIGHT, DARK, ThemePreference::Dark),
        3 => (LIGHT, HIGH_CONTRAST_DARK, ThemePreference::Dark),
        4 => (HIGH_CONTRAST_LIGHT, DARK, ThemePreference::Light),
        _ => (LIGHT, DARK, ThemePreference::System),
    }
}

/// The installed `(light, dark)` palettes, in the context's memory.
fn installed_id() -> egui::Id {
    egui::Id::new("dail-theme-installed")
}

/// The palette in effect for `ctx`.
pub fn palette(ctx: &egui::Context) -> Palette {
    let (light, dark) = ctx
        .data(|d| d.get_temp::<(u64, Palette, Palette)>(installed_id()))
        .map_or((LIGHT, DARK), |(_, light, dark)| (light, dark));
    if ctx.global_style().visuals.dark_mode {
        dark
    } else {
        light
    }
}

/// The theme preference for the `theme` setting.
pub const fn preference(setting: u64) -> ThemePreference {
    palettes(setting).2
}

/// Installs the palettes of the `theme` setting (again when it changes) and follows it.
pub fn apply(ctx: &egui::Context, setting: u64) {
    let (light, dark, wanted) = palettes(setting);
    let current = ctx.data(|d| d.get_temp::<(u64, Palette, Palette)>(installed_id()));
    if current.map(|c| c.0) != Some(setting) {
        ctx.set_visuals_of(Theme::Light, visuals(&light, false));
        ctx.set_visuals_of(Theme::Dark, visuals(&dark, true));
        ctx.data_mut(|d| d.insert_temp(installed_id(), (setting, light, dark)));
    }
    if ctx.options(|o| o.theme_preference) != wanted {
        ctx.set_theme(wanted);
    }
}

/// Applies the `text_scale` setting (percent) as egui's zoom factor whenever it changes.
/// Keyboard zoom (Ctrl + plus/minus/0, egui's own) lasts until the window closes; the setting
/// is what is kept (writing each step back would make it an undo step).
pub fn zoom(ctx: &egui::Context, setting: u64) {
    let applied = egui::Id::new("dail-text-scale");
    if ctx.data(|d| d.get_temp::<u64>(applied)) != Some(setting) {
        ctx.data_mut(|d| d.insert_temp(applied, setting));
        ctx.set_zoom_factor(setting as f32 / 100.0);
    }
}

fn visuals(p: &Palette, dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(4);
    v.panel_fill = p.surface;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(8);
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.extreme_bg_color = p.canvas;
    v.faint_bg_color = p.subtle;
    v.code_bg_color = p.subtle;
    v.hyperlink_color = p.ink;
    v.warn_fg_color = p.ink;
    // The one exception to "no colour": failures must stand out.
    v.error_fg_color = if dark {
        Color32::from_rgb(0xFF, 0xB4, 0xAB)
    } else {
        Color32::from_rgb(0xB3, 0x26, 0x1E)
    };
    v.selection.bg_fill = p.muted;
    v.selection.stroke = Stroke::new(1.0, p.ink);
    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.secondary);
    v.widgets.noninteractive.corner_radius = radius;
    for (w, fill, stroke) in [
        (&mut v.widgets.inactive, p.subtle, p.border),
        (&mut v.widgets.hovered, p.muted, p.border_strong),
        (&mut v.widgets.active, p.muted, p.ink),
        (&mut v.widgets.open, p.subtle, p.border_strong),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.fg_stroke = Stroke::new(1.0, p.ink);
        w.corner_radius = radius;
        w.expansion = 0.0;
    }
    v.striped = false;
    v
}
