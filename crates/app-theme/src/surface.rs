//! The colour roles of one surface group (content or chrome).

use serde::{Deserialize, Serialize};

use crate::Rgba;

/// The colour roles of a surface group. Components pick a role, never a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    /// List pane, cards, chrome.
    pub base: Rgba,
    /// Reader, window background.
    pub canvas: Rgba,
    /// Pane headers, secondary buttons, inputs, attachment and settings cards.
    pub raised: Rgba,
    /// Hovered rows and buttons.
    pub hover: Rgba,
    /// Selected row, active nav row, key caps.
    pub selected: Rgba,
    /// Badges, icon tiles, search marks.
    pub emphasis: Rgba,
    /// Behind dialogs and the palette (translucent).
    pub scrim: Rgba,
    /// Hairline dividers (decorative).
    pub border_subtle: Rgba,
    /// Inputs, overlays, chips (decorative).
    pub border: Rgba,
    /// Checkbox and switch outlines (at least 3:1).
    pub border_strong: Rgba,
    /// Focus ring, keyboard cursor bar.
    pub focus: Rgba,
    /// Titles, unread rows, active nav row.
    pub text_strong: Rgba,
    /// Body text.
    pub text: Rgba,
    /// Read rows, snippets, inactive nav rows.
    pub text_secondary: Rgba,
    /// Times, hints, captions, key letters.
    pub text_muted: Rgba,
    /// Primary buttons, the palette's cursor row, strong chips, toasts.
    pub inverse_bg: Rgba,
    /// Text on `inverse_bg`.
    pub inverse_text: Rgba,
    /// Failures in the status, the composer and the wizard.
    pub danger: Rgba,
    /// Failures on `inverse_bg` (toasts).
    pub inverse_danger: Rgba,
}
