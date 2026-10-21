//! Opt-in colours from `app-theme` (UI-16, amends ADR 0004). The widgets keep drawing with the
//! terminal's own attributes (bold, dim, reverse); when `tui_colors` is 1 and `NO_COLOR` is
//! unset, one pass over the finished frame maps them onto the active theme's roles in
//! truecolor: every cell gets the `canvas` fill and the `text` colour, bold text `text_strong`,
//! dim text `text_muted`, reversed cells (cursor, selection) the `selected` fill, box-drawing
//! characters `border`. The header and status lines take the chrome group's `base`.

use app_core::Settings;
use app_theme::{Rgba, Surface, Theme, builtin};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

/// The theme the TUI paints with: `theme_light` in light mode (1, or the legacy 4), else
/// `theme_dark`. A terminal can't tell the system's preference, so "follow the system" is dark.
pub fn theme_of(settings: &Settings) -> Theme {
    if matches!(settings.theme, 1 | 4) {
        builtin::get(&settings.theme_light).unwrap_or_else(builtin::light)
    } else {
        builtin::get(&settings.theme_dark).unwrap_or_else(builtin::dark)
    }
}

const fn rgb(c: Rgba) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

/// Paints `buf` with `theme`'s roles (see the module docs).
pub fn paint(buf: &mut Buffer, theme: &Theme) {
    let area = buf.area;
    let last = area.bottom().saturating_sub(1);
    for y in area.top()..area.bottom() {
        let bar = y == area.top() || y == last;
        let s: &Surface = if bar { &theme.chrome } else { &theme.content };
        let fill = if bar { s.base } else { s.canvas };
        for x in area.left()..area.right() {
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            let m = cell.modifier;
            let boxed = cell
                .symbol()
                .chars()
                .next()
                .is_some_and(|c| ('\u{2500}'..='\u{257f}').contains(&c));
            let (fg, bg) = if m.contains(Modifier::REVERSED) {
                (s.text_strong, s.selected)
            } else if m.contains(Modifier::DIM) {
                (s.text_muted, fill)
            } else if m.contains(Modifier::BOLD) {
                (s.text_strong, fill)
            } else if boxed {
                (s.border, fill)
            } else {
                (s.text, fill)
            };
            if cell.fg == Color::Reset {
                cell.fg = rgb(fg);
            }
            if cell.bg == Color::Reset {
                cell.bg = rgb(bg);
            }
            cell.modifier = m - Modifier::REVERSED - Modifier::DIM;
        }
    }
}
