//! A theme: two surface groups, the account tags, the type scale and the metrics.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{Metrics, Rgba, Surface, TypeScale};

/// Light or dark: egui's `dark_mode`, scrollbars, text cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    Light,
    Dark,
}

/// A complete theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    /// Stable slug, stored in the settings (`dail-dark`).
    pub id: Cow<'static, str>,
    /// Shown in the settings page (`Dail Dark`).
    pub name: Cow<'static, str>,
    /// Appearance of the content group.
    pub appearance: Appearance,
    /// List, reader, composer, dialogs.
    pub content: Surface,
    /// Sidebar and top bar; equal to `content` in a flat theme.
    pub chrome: Surface,
    /// Account colours: they carry information, not accent.
    pub tags: [Rgba; 6],
    pub type_scale: TypeScale,
    pub metrics: Metrics,
}

impl Theme {
    /// Whether the chrome is a different group from the content.
    pub fn split_chrome(&self) -> bool {
        self.chrome != self.content
    }
}
