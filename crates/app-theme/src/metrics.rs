//! Spacing, radii, strokes and component sizes, in points.

use serde::{Deserialize, Serialize};

/// The spacing scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

/// Corner radii. `full` is a pill or a circle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Radius {
    /// Key caps, badges, chips, checkbox.
    pub sm: f32,
    /// Buttons, inputs, row highlights, cards, menus.
    pub md: f32,
    /// Palette, dialogs, toasts, floating composer.
    pub lg: f32,
    /// Pips, avatars, switch.
    pub full: f32,
}

/// Stroke widths.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Strokes {
    pub hairline: f32,
    pub focus: f32,
    /// Keyboard cursor and active-row bar.
    pub indicator: f32,
}

/// The window frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub top_bar: f32,
    pub sidebar: f32,
    pub sidebar_min: f32,
    pub sidebar_max: f32,
    pub rail: f32,
    pub list: f32,
    pub list_min: f32,
    pub list_max: f32,
    pub pane_header: f32,
    pub pane_footer: f32,
    pub reader_toolbar: f32,
    pub reader_padding: f32,
    pub text_width_max: f32,
}

/// Row heights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rows {
    pub compact: f32,
    pub cozy: f32,
    pub preview: f32,
    pub nav: f32,
    pub palette: f32,
}

/// Control sizes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    pub button: f32,
    pub button_small: f32,
    pub icon_button: f32,
    pub key_cap: f32,
    pub badge: f32,
    pub checkbox: f32,
    pub switch_width: f32,
    pub switch_height: f32,
    pub avatar_sm: f32,
    pub avatar_md: f32,
    pub avatar_lg: f32,
    pub unread_pip: f32,
    pub icon_xs: f32,
    pub icon_sm: f32,
    pub icon_md: f32,
    pub icon_lg: f32,
}

/// Overlay sizes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Overlays {
    pub palette_width: f32,
    /// Below the top bar.
    pub palette_offset: f32,
    pub palette_list_max: f32,
    pub dialog_width: f32,
    pub floating_composer_width: f32,
    pub toast_width_max: f32,
}

/// All metrics. No shadows: an overlay is a fill, a border and a scrim.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub space: Spacing,
    pub radius: Radius,
    pub stroke: Strokes,
    pub frame: Frame,
    pub rows: Rows,
    pub controls: Controls,
    pub overlays: Overlays,
    /// Hover and press, in milliseconds.
    pub animation_ms: u32,
}

impl Metrics {
    /// The metrics of the design (§3.5 of the UI plan).
    pub const DEFAULT: Self = Self {
        space: Spacing {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
        },
        radius: Radius {
            sm: 2.0,
            md: 4.0,
            lg: 8.0,
            full: 9999.0,
        },
        stroke: Strokes {
            hairline: 1.0,
            focus: 1.0,
            indicator: 2.0,
        },
        frame: Frame {
            top_bar: 56.0,
            sidebar: 224.0,
            sidebar_min: 160.0,
            sidebar_max: 360.0,
            rail: 56.0,
            list: 410.0,
            list_min: 300.0,
            list_max: 640.0,
            pane_header: 48.0,
            pane_footer: 40.0,
            reader_toolbar: 48.0,
            reader_padding: 24.0,
            text_width_max: 720.0,
        },
        rows: Rows {
            compact: 40.0,
            cozy: 48.0,
            preview: 72.0,
            nav: 32.0,
            palette: 36.0,
        },
        controls: Controls {
            button: 28.0,
            button_small: 24.0,
            icon_button: 28.0,
            key_cap: 18.0,
            badge: 18.0,
            checkbox: 14.0,
            switch_width: 44.0,
            switch_height: 24.0,
            avatar_sm: 28.0,
            avatar_md: 32.0,
            avatar_lg: 40.0,
            unread_pip: 6.0,
            icon_xs: 14.0,
            icon_sm: 16.0,
            icon_md: 18.0,
            icon_lg: 20.0,
        },
        overlays: Overlays {
            palette_width: 560.0,
            palette_offset: 80.0,
            palette_list_max: 380.0,
            dialog_width: 480.0,
            floating_composer_width: 540.0,
            toast_width_max: 480.0,
        },
        animation_ms: 80,
    };
}
