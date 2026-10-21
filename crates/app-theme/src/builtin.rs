//! The built-in themes. `dail-dark-hc` and `dail-light-hc` are the GUI's `theme` 3 and 4.

use std::borrow::Cow;

use crate::{Appearance, Metrics, Rgba, Surface, Theme, TypeScale, hex};

/// Account tags: hues that read on light and dark surfaces alike.
pub const TAGS: [Rgba; 6] = [
    hex(0x3B82F6),
    hex(0xE11D48),
    hex(0x10B981),
    hex(0xF59E0B),
    hex(0x8B5CF6),
    hex(0x06B6D4),
];

const DARK_RED: Rgba = hex(0xB3261E);
const LIGHT_RED: Rgba = hex(0xFFB4AB);

/// The content group of `dail-dark`, also the chrome of `dail-light`.
pub const DARK: Surface = Surface {
    base: hex(0x0E0E0E),
    canvas: hex(0x131313),
    raised: hex(0x1B1B1B),
    hover: hex(0x1F1F1F),
    selected: hex(0x2A2A2A),
    emphasis: hex(0x343434),
    scrim: hex(0x000000).with_alpha(153),
    border_subtle: hex(0x222222),
    border: hex(0x2F2F2F),
    border_strong: hex(0x6B6B6B),
    focus: hex(0xFFFFFF),
    text_strong: hex(0xFFFFFF),
    text: hex(0xE2E2E2),
    text_secondary: hex(0xC6C6C6),
    text_muted: hex(0x959595),
    inverse_bg: hex(0xFFFFFF),
    inverse_text: hex(0x0E0E0E),
    danger: LIGHT_RED,
    inverse_danger: DARK_RED,
};

/// The content group of `dail-light`.
pub const LIGHT: Surface = Surface {
    base: hex(0xFFFFFF),
    canvas: hex(0xFFFFFF),
    raised: hex(0xF2F2F2),
    hover: hex(0xF2F2F2),
    selected: hex(0xE2E2E2),
    emphasis: hex(0xE2E2E2),
    scrim: hex(0x0E0E0E).with_alpha(102),
    border_subtle: hex(0xE9E9E9),
    border: hex(0xD7D7D7),
    border_strong: hex(0x8A8A8A),
    focus: hex(0x0E0E0E),
    text_strong: hex(0x0E0E0E),
    text: hex(0x1F1F1F),
    text_secondary: hex(0x343434),
    text_muted: hex(0x626262),
    inverse_bg: hex(0x0E0E0E),
    inverse_text: hex(0xFFFFFF),
    danger: DARK_RED,
    inverse_danger: LIGHT_RED,
};

/// High-contrast dark: white on black, borders that show (`theme` 3).
pub const DARK_HC: Surface = Surface {
    base: hex(0x000000),
    canvas: hex(0x000000),
    raised: hex(0x1F1F1F),
    hover: hex(0x383838),
    selected: hex(0x383838),
    emphasis: hex(0x383838),
    scrim: hex(0x000000).with_alpha(153),
    border_subtle: hex(0xB0B0B0),
    border: hex(0xB0B0B0),
    border_strong: hex(0xFFFFFF),
    focus: hex(0xFFFFFF),
    text_strong: hex(0xFFFFFF),
    text: hex(0xF2F2F2),
    text_secondary: hex(0xF2F2F2),
    text_muted: hex(0xD4D4D4),
    inverse_bg: hex(0xFFFFFF),
    inverse_text: hex(0x000000),
    danger: LIGHT_RED,
    inverse_danger: DARK_RED,
};

/// High-contrast light: black on white (`theme` 4). Its `danger` is darker than `dail-light`'s,
/// for AA on `hover`.
pub const LIGHT_HC: Surface = Surface {
    base: hex(0xFFFFFF),
    canvas: hex(0xFFFFFF),
    raised: hex(0xEBEBEB),
    hover: hex(0xD6D6D6),
    selected: hex(0xD6D6D6),
    emphasis: hex(0xD6D6D6),
    scrim: hex(0x000000).with_alpha(102),
    border_subtle: hex(0x4D4D4D),
    border: hex(0x4D4D4D),
    border_strong: hex(0x000000),
    focus: hex(0x000000),
    text_strong: hex(0x000000),
    text: hex(0x0D0D0D),
    text_secondary: hex(0x0D0D0D),
    text_muted: hex(0x2E2E2E),
    inverse_bg: hex(0x000000),
    inverse_text: hex(0xFFFFFF),
    danger: hex(0x9C1F17),
    inverse_danger: LIGHT_RED,
};

fn theme(
    id: &'static str,
    name: &'static str,
    appearance: Appearance,
    content: Surface,
    chrome: Surface,
) -> Theme {
    Theme {
        id: Cow::Borrowed(id),
        name: Cow::Borrowed(name),
        appearance,
        content,
        chrome,
        tags: TAGS,
        type_scale: TypeScale::DEFAULT,
        metrics: Metrics::DEFAULT,
    }
}

/// Every built-in theme: dark, light, then the high-contrast pair.
pub fn all() -> Vec<Theme> {
    vec![
        theme("dail-dark", "Dail Dark", Appearance::Dark, DARK, DARK),
        // Dark sidebar and top bar, as in the light mockup (UI plan, D2).
        theme("dail-light", "Dail Light", Appearance::Light, LIGHT, DARK),
        theme(
            "dail-dark-hc",
            "Dail Dark (high contrast)",
            Appearance::Dark,
            DARK_HC,
            DARK_HC,
        ),
        theme(
            "dail-light-hc",
            "Dail Light (high contrast)",
            Appearance::Light,
            LIGHT_HC,
            LIGHT_HC,
        ),
    ]
}

/// The built-in theme `id`.
pub fn get(id: &str) -> Option<Theme> {
    all().into_iter().find(|t| t.id == id)
}
