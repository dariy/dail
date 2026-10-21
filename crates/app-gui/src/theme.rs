//! The egui adapter of the theme library (`app-theme`, ADR 0008): a [`Theme`] becomes [`Tokens`]
//! (roles as `Color32`, metrics, the type scale) and egui `Visuals`, once per theme change.
//! Widgets read roles and metrics from the tokens only, through the kit in `crate::ui`.

use std::sync::Arc;

use app_theme::fonts::{self, WEIGHTS};
use app_theme::icon::{ICON_FAMILY, ICON_FONT};
use app_theme::{Face, Icon, Metrics, Rgba, Surface, Theme, TypeScale, TypeToken, builtin};
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Stroke,
    TextStyle, ThemePreference, Visuals, epaint::text::VariationCoords,
};

/// `Rgba` as egui's (premultiplied) colour.
pub fn color(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// The colour roles of one surface group (content or chrome), as egui colours; each field is
/// the `app_theme::Surface` role of the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roles {
    pub base: Color32,
    pub canvas: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub emphasis: Color32,
    pub scrim: Color32,
    pub border_subtle: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub focus: Color32,
    pub text_strong: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub inverse_bg: Color32,
    pub inverse_text: Color32,
    pub danger: Color32,
    pub inverse_danger: Color32,
    /// A dark group (egui's `dark_mode` for its widgets).
    pub dark: bool,
}

impl Roles {
    /// The roles of `surface`.
    pub fn of(s: &Surface) -> Self {
        Self {
            base: color(s.base),
            canvas: color(s.canvas),
            raised: color(s.raised),
            hover: color(s.hover),
            selected: color(s.selected),
            emphasis: color(s.emphasis),
            scrim: color(s.scrim),
            border_subtle: color(s.border_subtle),
            border: color(s.border),
            border_strong: color(s.border_strong),
            focus: color(s.focus),
            text_strong: color(s.text_strong),
            text: color(s.text),
            text_secondary: color(s.text_secondary),
            text_muted: color(s.text_muted),
            inverse_bg: color(s.inverse_bg),
            inverse_text: color(s.inverse_text),
            danger: color(s.danger),
            inverse_danger: color(s.inverse_danger),
            dark: s.canvas.luminance() < 0.5,
        }
    }
}

/// What widgets draw with: the content roles (through `Deref`), the chrome roles, the account
/// tags, the metrics and the type scale of one theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    /// The theme's ID (`dail-dark`).
    pub id: String,
    /// List, reader, composer, dialogs.
    pub content: Roles,
    /// Sidebar and top bar.
    pub chrome_roles: Roles,
    /// Account colours.
    pub tags: [Color32; 6],
    /// Spacing, radii, sizes.
    pub m: Metrics,
    /// The type scale.
    pub type_scale: TypeScale,
}

impl std::ops::Deref for Tokens {
    type Target = Roles;
    fn deref(&self) -> &Roles {
        &self.content
    }
}

impl Tokens {
    /// The tokens of `theme`.
    pub fn of(theme: &Theme) -> Self {
        Self {
            id: theme.id.to_string(),
            content: Roles::of(&theme.content),
            chrome_roles: Roles::of(&theme.chrome),
            tags: theme.tags.map(color),
            m: theme.metrics,
            type_scale: theme.type_scale,
        }
    }

    /// The same tokens with the chrome group as the content: for the sidebar and the top bar.
    #[must_use]
    pub fn chrome(&self) -> Self {
        Self {
            content: self.chrome_roles,
            ..self.clone()
        }
    }

    /// The colour tag of `account`: by its position among the accounts.
    pub fn tag(&self, state: &app_core::AppState, account: app_core::AccountId) -> Color32 {
        let index = state.accounts.get_index_of(&account).unwrap_or(0);
        self.tags[index % self.tags.len()]
    }
}

/// The light and dark themes of a `theme` setting, and the preference between them:
/// 0 = follow the system, 1 = light, 2 = dark, 3 = high-contrast dark, 4 = high-contrast light.
pub fn themes(setting: u64) -> (Theme, Theme, ThemePreference) {
    let named = |id: &str, fallback: fn() -> Theme| builtin::get(id).unwrap_or_else(fallback);
    match setting {
        1 => (builtin::light(), builtin::dark(), ThemePreference::Light),
        2 => (builtin::light(), builtin::dark(), ThemePreference::Dark),
        3 => (
            builtin::light(),
            named("dail-dark-hc", builtin::dark),
            ThemePreference::Dark,
        ),
        4 => (
            named("dail-light-hc", builtin::light),
            builtin::dark(),
            ThemePreference::Light,
        ),
        _ => (builtin::light(), builtin::dark(), ThemePreference::System),
    }
}

/// The installed `(setting, light, dark)` tokens, in the context's memory.
fn installed_id() -> egui::Id {
    egui::Id::unique("dail-theme-installed")
}

/// The tokens in effect for `ctx`.
pub fn tokens(ctx: &egui::Context) -> Tokens {
    let dark = ctx.global_style().visuals.dark_mode;
    ctx.data(|d| d.get_temp::<(u64, Tokens, Tokens)>(installed_id()))
        .map_or_else(
            || {
                Tokens::of(&if dark {
                    builtin::dark()
                } else {
                    builtin::light()
                })
            },
            |(_, light, darkt)| if dark { darkt } else { light },
        )
}

/// The theme preference for the `theme` setting.
pub fn preference(setting: u64) -> ThemePreference {
    themes(setting).2
}

/// Installs the themes of the `theme` setting (again when it changes) and follows it. The
/// first call also installs the fonts.
pub fn apply(ctx: &egui::Context, setting: u64) {
    install_fonts(ctx);
    let current = ctx.data(|d| d.get_temp::<(u64, Tokens, Tokens)>(installed_id()));
    let (light, dark, wanted) = themes(setting);
    if current.map(|c| c.0) != Some(setting) {
        let (lt, dt) = (Tokens::of(&light), Tokens::of(&dark));
        for (theme, t) in [(egui::Theme::Light, &lt), (egui::Theme::Dark, &dt)] {
            let v = visuals(&t.content, &t.m, theme == egui::Theme::Dark);
            ctx.style_mut_of(theme, |style| {
                style.visuals = v;
                style.animation_time = t.m.animation_ms as f32 / 1000.0;
                style.spacing.scroll = egui::style::ScrollStyle::floating();
            });
        }
        ctx.data_mut(|d| d.insert_temp(installed_id(), (setting, lt, dt)));
    }
    if ctx.options(|o| o.theme_preference) != wanted {
        ctx.set_theme(wanted);
    }
}

/// Applies the `text_scale` setting (percent) as egui's zoom factor whenever it changes.
/// Keyboard zoom (Ctrl + plus/minus/0, egui's own) lasts until the window closes; the setting
/// is what is kept (writing each step back would make it an undo step).
pub fn zoom(ctx: &egui::Context, setting: u64) {
    let applied = egui::Id::unique("dail-text-scale");
    if ctx.data(|d| d.get_temp::<u64>(applied)) != Some(setting) {
        ctx.data_mut(|d| d.insert_temp(applied, setting));
        ctx.set_zoom_factor(setting as f32 / 100.0);
    }
}

/// The `Visuals` of the chrome group: the sidebar and the top bar set them on their `Ui`, so
/// egui's own widgets in them (scrollbars, text edits, tooltips) match a dark chrome.
pub fn chrome_visuals(t: &Tokens) -> Visuals {
    visuals(&t.chrome_roles, &t.m, t.chrome_roles.dark)
}

/// egui's `Visuals` from one group of roles.
pub fn visuals(r: &Roles, m: &Metrics, dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(m.radius.md as u8);
    let hair = m.stroke.hairline;
    v.panel_fill = r.canvas;
    v.window_fill = r.base;
    v.window_stroke = Stroke::new(hair, r.border);
    v.window_corner_radius = CornerRadius::same(m.radius.lg as u8);
    v.menu_corner_radius = radius;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.extreme_bg_color = r.base;
    v.text_edit_bg_color = Some(r.raised);
    v.faint_bg_color = r.raised;
    v.code_bg_color = r.selected;
    v.hyperlink_color = r.text_strong;
    v.warn_fg_color = r.text_strong;
    v.error_fg_color = r.danger;
    v.weak_text_color = Some(r.text_muted);
    v.override_text_color = None;
    v.selection.bg_fill = r.emphasis;
    v.selection.stroke = Stroke::new(m.stroke.focus, r.focus);
    v.text_cursor.stroke = Stroke::new(2.0, r.focus);
    v.widgets.noninteractive.bg_fill = r.base;
    v.widgets.noninteractive.weak_bg_fill = r.base;
    v.widgets.noninteractive.bg_stroke = Stroke::new(hair, r.border_subtle);
    v.widgets.noninteractive.fg_stroke = Stroke::new(hair, r.text);
    v.widgets.noninteractive.corner_radius = radius;
    for (w, fill, stroke, fg) in [
        (&mut v.widgets.inactive, r.raised, r.border_subtle, r.text),
        (&mut v.widgets.hovered, r.hover, r.border, r.text_strong),
        (&mut v.widgets.active, r.selected, r.focus, r.text_strong),
        (&mut v.widgets.open, r.selected, r.border, r.text_strong),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(hair, stroke);
        w.fg_stroke = Stroke::new(1.0, fg);
        w.corner_radius = radius;
        w.expansion = 0.0;
    }
    v.striped = false;
    v
}

/// egui's own fonts, after the bundled ones: scripts and symbols Geist and JetBrains Mono lack
/// (Greek in Geist, arrows, emoji in mail) still render as before.
const SANS_FALLBACKS: [&str; 4] = [
    "Ubuntu-Light",
    "egui-icons",
    "NotoEmoji-Regular",
    "emoji-icon-font",
];
const MONO_FALLBACKS: [&str; 5] = [
    "Hack",
    "Ubuntu-Light",
    "egui-icons",
    "NotoEmoji-Regular",
    "emoji-icon-font",
];

/// The fonts: Geist and JetBrains Mono, each registered once per weight (`wght` through
/// [`FontTweak::coords`], the bytes shared) as the families of [`fonts::family`]; egui's
/// built-in fonts as fallbacks. `Proportional` is `sans` and `Monospace` is `mono`.
pub fn font_definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    for font in fonts::fonts() {
        let fallbacks: &[&str] = match font.face {
            Face::Sans => &SANS_FALLBACKS,
            Face::Mono => &MONO_FALLBACKS,
        };
        for weight in WEIGHTS {
            let data_name = format!("{}-{weight}", font.name);
            let data = FontData::from_static(font.bytes).tweak(FontTweak {
                coords: VariationCoords::new([(b"wght", f32::from(weight))]),
                ..Default::default()
            });
            defs.font_data.insert(data_name.clone(), Arc::new(data));
            let list: Vec<String> = std::iter::once(data_name)
                .chain(fallbacks.iter().map(|f| (*f).to_owned()))
                .collect();
            if weight == 400 {
                let generic = match font.face {
                    Face::Sans => FontFamily::Proportional,
                    Face::Mono => FontFamily::Monospace,
                };
                defs.families.insert(generic, list.clone());
            }
            defs.families.insert(
                FontFamily::Name(fonts::family(font.face, weight).into()),
                list,
            );
        }
    }
    // The icons, outlined and filled (`FILL` 0 and 1), in families of their own: text never
    // falls back to them, and they never fall back to text.
    for (name, fill) in [(ICON_FAMILY, 0.0), (ICON_FAMILY_FILLED, 1.0)] {
        let data = FontData::from_static(ICON_FONT).tweak(FontTweak {
            coords: VariationCoords::new([(b"FILL", fill), (b"wght", 400.0)]),
            ..Default::default()
        });
        defs.font_data.insert(name.to_owned(), Arc::new(data));
        defs.families
            .insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
    }
    defs
}

/// The family of the filled icons (a starred star, the active nav icon).
pub const ICON_FAMILY_FILLED: &str = "icons-filled";

/// The font of an icon at `size`, outlined or `filled`.
pub fn icon_font(size: f32, filled: bool) -> FontId {
    FontId::new(
        size,
        FontFamily::Name(
            if filled {
                ICON_FAMILY_FILLED
            } else {
                ICON_FAMILY
            }
            .into(),
        ),
    )
}

/// The icon of a mailbox role: total over [`app_core::MailboxRole`], a folder for the rest.
pub const fn role_icon(role: Option<app_core::MailboxRole>) -> Icon {
    use app_core::MailboxRole;
    match role {
        Some(MailboxRole::Inbox) => Icon::Inbox,
        Some(MailboxRole::Sent) => Icon::Send,
        Some(MailboxRole::Drafts) => Icon::Draft,
        Some(MailboxRole::Trash) => Icon::Delete,
        Some(MailboxRole::Archive) => Icon::Archive,
        Some(MailboxRole::Junk) => Icon::Report,
        Some(MailboxRole::All) => Icon::AllInbox,
        Some(MailboxRole::Flagged) => Icon::Star,
        None => Icon::Folder,
    }
}

/// The font of `token` at its own weight.
pub fn font(token: &TypeToken) -> FontId {
    font_at(token, token.weight)
}

/// The font of `token` at `weight` (an unread row is its token at 600).
pub fn font_at(token: &TypeToken, weight: u16) -> FontId {
    FontId::new(
        token.size,
        FontFamily::Name(fonts::family(token.face, weight).into()),
    )
}

/// Geist at `size` and `weight`.
pub fn sans(size: f32, weight: u16) -> FontId {
    FontId::new(
        size,
        FontFamily::Name(fonts::family(Face::Sans, weight).into()),
    )
}

/// JetBrains Mono at `size` and `weight`.
pub fn mono(size: f32, weight: u16) -> FontId {
    FontId::new(
        size,
        FontFamily::Name(fonts::family(Face::Mono, weight).into()),
    )
}

/// egui's text styles on the type scale, so its own widgets match: Body `body_md`, Small
/// `body_sm`, Button `body_md` at 500, Heading `headline_lg`, Monospace `label_md`.
pub fn text_styles(scale: &TypeScale) -> std::collections::BTreeMap<TextStyle, FontId> {
    [
        (TextStyle::Body, font(&scale.body_md)),
        (TextStyle::Small, font(&scale.body_sm)),
        (TextStyle::Button, font_at(&scale.body_md, 500)),
        (TextStyle::Heading, font(&scale.headline_lg)),
        (TextStyle::Monospace, font(&scale.label_md)),
    ]
    .into_iter()
    .collect()
}

/// Installs the fonts and the text styles once per context.
pub fn install_fonts(ctx: &egui::Context) {
    let installed = egui::Id::unique("dail-fonts-installed");
    if ctx.data(|d| d.get_temp::<bool>(installed)).is_some() {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(installed, true));
    ctx.set_fonts(font_definitions());
    let styles = text_styles(&TypeScale::DEFAULT);
    ctx.all_styles_mut(|style| style.text_styles.clone_from(&styles));
}

/// Geist at 600, at the size the text already has: headings, unread rows, strong buttons.
pub fn semibold() -> FontFamily {
    FontFamily::Name(fonts::family(Face::Sans, 600).into())
}
