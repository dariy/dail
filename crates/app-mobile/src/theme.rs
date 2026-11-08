//! The theme in the frame: `app-theme`'s tokens as JSON for Material 3, and the About screen's
//! licences. Kotlin maps roles onto colours and never chooses one (ADR 0008, ADR 0009).

use std::collections::BTreeMap;

use app_core::Settings;
use app_front::i18n::Key;
use app_theme::{Icon, Rgba, Surface, Theme, TypeToken, builtin, fonts, icon};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A theme, sent in the frame when it changes (the settings or the system's dark mode).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FrameTheme {
    /// The theme's slug (`dail-dark`).
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether it is dark (the status bar's icons, Material's dark scheme).
    pub dark: bool,
    /// The content group's colour roles (`base`, `text`, …) as `#RRGGBBAA`.
    pub content: BTreeMap<String, String>,
    /// The chrome group's roles (top bar, drawer); equal to `content` in a flat theme.
    pub chrome: BTreeMap<String, String>,
    /// The account colours, `#RRGGBBAA`.
    pub tags: Vec<String>,
    /// The type scale by token name (`body_md`, …).
    pub type_scale: BTreeMap<String, TextStyle>,
    /// Spacing, radii, strokes and sizes in dp, as `app_theme::Metrics`.
    pub metrics: serde_json::Value,
    /// The icon font's glyph for each icon name the frames use (`archive` → `"\u{e149}"`).
    pub icons: BTreeMap<String, String>,
}

/// One text style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextStyle {
    /// `sans` (Geist) or `mono` (JetBrains Mono).
    pub face: String,
    /// Size in sp.
    pub size: f32,
    /// Line height in sp.
    pub line_height: f32,
    /// 400, 500 or 600.
    pub weight: u16,
    /// Letter spacing in em.
    pub tracking: f32,
}

/// The About screen: the version, then a list of pages, one per licence of what the app bundles
/// (as the desktop's About). A tapped page replaces the list until Back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct About {
    /// The screen's title.
    pub title: String,
    /// What the app is.
    pub text: String,
    /// One page per bundled work's licence, in this order.
    pub pages: Vec<AboutPage>,
    /// The privacy policy (STORE-19), opened in the browser.
    pub privacy: AboutLink,
    /// The open page: draw its text instead of the list. The top bar carries its name and Back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<Licence>,
}

/// A row of the About screen's list: tapping it opens the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AboutPage {
    /// The work (`Geist`).
    pub label: String,
    /// Opens its page.
    pub action: crate::frame::ActionId,
}

/// A link on the About screen: its label and its `https` target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AboutLink {
    /// What the link says.
    pub label: String,
    /// Where it goes.
    pub url: String,
}

/// The privacy policy dail links from About and the store listings (STORE-19).
pub const PRIVACY_URL: &str = "https://github.com/dariy/dail/blob/main/PRIVACY.md";

/// A bundled work and its licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Licence {
    /// The work (`Geist`).
    pub name: String,
    /// The licence's text.
    pub text: String,
}

fn hex(color: Rgba) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r, color.g, color.b, color.a
    )
}

fn roles(surface: &Surface) -> BTreeMap<String, String> {
    let Ok(serde_json::Value::Object(map)) = serde_json::to_value(surface) else {
        return BTreeMap::new();
    };
    map.into_iter()
        .filter_map(|(role, value)| {
            let color: Rgba = serde_json::from_value(value).ok()?;
            Some((role, hex(color)))
        })
        .collect()
}

fn style(token: TypeToken) -> TextStyle {
    let face = match token.face {
        app_theme::Face::Sans => "sans",
        app_theme::Face::Mono => "mono",
    };
    TextStyle {
        face: face.to_owned(),
        size: token.size,
        line_height: token.line_height,
        weight: token.weight,
        tracking: token.tracking,
    }
}

/// The theme the settings choose for the system's mode, as the GUI chooses it: `theme` 0
/// follows the system, 1 is light, 2 dark (the legacy 3 and 4 the high-contrast pair); an
/// unknown name falls back to its mode's default.
pub fn chosen(settings: &Settings, system_dark: bool) -> Theme {
    let mut settings = settings.clone();
    settings.migrate_legacy_theme();
    let dark = match settings.theme {
        1 => false,
        2 => true,
        _ => system_dark,
    };
    if dark {
        builtin::get(&settings.theme_dark).unwrap_or_else(builtin::dark)
    } else {
        builtin::get(&settings.theme_light).unwrap_or_else(builtin::light)
    }
}

/// `theme` as the frame carries it.
pub fn frame_theme(theme: &Theme) -> FrameTheme {
    let scale = &theme.type_scale;
    let type_scale = [
        ("headline_xl", scale.headline_xl),
        ("headline_lg", scale.headline_lg),
        ("headline_md", scale.headline_md),
        ("body_lg", scale.body_lg),
        ("body_md", scale.body_md),
        ("body_sm", scale.body_sm),
        ("label_md", scale.label_md),
        ("label_sm", scale.label_sm),
        ("code_badge", scale.code_badge),
    ]
    .into_iter()
    .map(|(name, token)| (name.to_owned(), style(token)))
    .collect();
    FrameTheme {
        id: theme.id.to_string(),
        name: theme.name.to_string(),
        dark: theme.appearance == app_theme::Appearance::Dark,
        content: roles(&theme.content),
        chrome: roles(&theme.chrome),
        tags: theme.tags.iter().copied().map(hex).collect(),
        type_scale,
        metrics: serde_json::to_value(theme.metrics).unwrap_or_default(),
        icons: Icon::ALL
            .iter()
            .map(|icon| (icon.name().to_owned(), icon.char().to_string()))
            .collect(),
    }
}

/// The About screen's title, with the running version: `About dail (0.1.0)`.
pub fn about_title() -> String {
    app_front::i18n::f(
        Key::AboutDailVersion,
        &[("version", env!("CARGO_PKG_VERSION"))],
    )
}

/// The licences of what the app bundles, one per About page. Prose is reflowed for the screen
/// (`text::reflow`); the crate and library lists keep one entry per line.
pub fn licences() -> Vec<Licence> {
    let mut licences: Vec<Licence> = fonts::fonts()
        .iter()
        .map(|font| Licence {
            name: font.name.to_owned(),
            text: app_front::text::reflow(font.license),
        })
        .collect();
    licences.push(Licence {
        name: "Material Symbols".to_owned(),
        text: app_front::text::reflow(icon::ICON_LICENSE),
    });
    // Written by `cargo xtask android licences`; `android check` fails when they are stale.
    licences.push(Licence {
        name: app_front::i18n::s(Key::RustCrates).to_owned(),
        text: include_str!("../licences/rust-crates.txt").to_owned(),
    });
    licences.push(Licence {
        name: app_front::i18n::s(Key::AndroidLibraries).to_owned(),
        text: include_str!("../licences/android-libraries.txt").to_owned(),
    });
    licences.push(Licence {
        name: app_front::i18n::s(Key::PdfFontsAndCmaps).to_owned(),
        text: app_front::text::reflow(include_str!("../licences/pdf-assets.txt")),
    });
    licences.push(Licence {
        name: app_front::i18n::s(Key::PdfCraftViewer).to_owned(),
        text: app_front::text::reflow(include_str!("../licences/pdfcraft-viewer.txt")),
    });
    licences
}

/// The About screen with the given rows and the open page.
pub fn about(pages: Vec<AboutPage>, page: Option<Licence>) -> About {
    About {
        title: about_title(),
        // The phone has no MCP server and no keyboard-first use: neither is said here (MISC-13).
        text: app_front::i18n::f(
            Key::AboutMobileText,
            &[("licence", env!("CARGO_PKG_LICENSE"))],
        ),
        pages,
        privacy: AboutLink {
            label: app_front::i18n::s(Key::PrivacyPolicy).to_owned(),
            url: PRIVACY_URL.to_owned(),
        },
        page,
    }
}
