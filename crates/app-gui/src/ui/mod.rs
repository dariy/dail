//! The component kit (UI plan §3.8): every widget the GUI draws its chrome with. Components read
//! colour roles and metrics from [`Tokens`] only; no hex value or pixel number lives outside this
//! module and `theme.rs` (the style check, `tests/it/style.rs`, keeps it so).
//!
//! Each component keeps the accessible name its caller gives it and draws the focus ring when it
//! has the keyboard focus.

pub mod button;
pub mod gallery;

use app_theme::{Icon, TypeToken};
use egui::text::LayoutJob;
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Margin, Rect, Response, RichText, Sense,
    Stroke, TextFormat, Ui, Vec2, WidgetInfo,
};

use crate::theme::{self, Tokens};

pub use button::{ghost, icon_button, primary, secondary, secondary_icon};

/// A metric as egui's whole-point margin.
fn margin(v: f32) -> i8 {
    v.round() as i8
}

/// A radius as egui's corner radius.
pub fn radius(r: f32) -> CornerRadius {
    CornerRadius::same(r.clamp(0.0, 255.0) as u8)
}

/// The font of a type token at its weight.
pub fn font(token: &TypeToken) -> FontId {
    theme::font(token)
}

/// `text` in a type token and a colour: its weight family and size.
pub fn text(token: &TypeToken, text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text).font(font(token)).color(color)
}

/// `text` in a type token at `weight` (an unread row: its token at 600).
pub fn text_at(
    token: &TypeToken,
    weight: u16,
    text: impl Into<String>,
    color: Color32,
) -> RichText {
    RichText::new(text)
        .font(theme::font_at(token, weight))
        .color(color)
}

/// The text format of a type token: weight family, tracking (em × size) and line height.
pub fn format(token: &TypeToken, color: Color32) -> TextFormat {
    TextFormat {
        font_id: font(token),
        color,
        extra_letter_spacing: token.tracking * token.size,
        line_height: Some(token.line_height),
        valign: Align::Center,
        ..Default::default()
    }
}

/// `text` as a layout job in a type token, with its tracking and line height.
pub fn job(token: &TypeToken, text: &str, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, format(token, color));
    job
}

/// JetBrains Mono at 500 for counts, dates and key hints (`label_sm`'s face) at `size`.
pub fn mono(size: f32) -> FontId {
    theme::mono(size, 500)
}

/// An icon, painted (not a label: screen readers never read the private-use character).
pub fn icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32, filled: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_icon(ui, rect.center(), icon, size, color, filled);
    response
}

/// Paints `icon` centred on `at`.
pub fn paint_icon(ui: &Ui, at: egui::Pos2, icon: Icon, size: f32, color: Color32, filled: bool) {
    ui.painter().text(
        at,
        Align2::CENTER_CENTER,
        icon.char(),
        theme::icon_font(size, filled),
        color,
    );
}

/// An icon then `text`, for buttons and labels; give the widget its name with `widget_info`.
pub fn icon_text(icon: Icon, text: &str, font: FontId, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let gap = font.size * 0.45;
    job.append(
        &icon.char().to_string(),
        0.0,
        TextFormat {
            font_id: theme::icon_font(font.size + 3.0, false),
            color,
            valign: Align::Center,
            ..Default::default()
        },
    );
    job.append(
        text,
        gap,
        TextFormat {
            font_id: font,
            color,
            valign: Align::Center,
            ..Default::default()
        },
    );
    job
}

/// A label with a leading icon whose accessible name is `name`.
pub fn icon_label(
    ui: &mut Ui,
    t: &Tokens,
    icon: Icon,
    text: &str,
    color: Color32,
    name: &str,
) -> Response {
    let job = icon_text(icon, text, font(&t.type_scale.body_md), color);
    let response = ui.add(egui::Label::new(job).truncate());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, name));
    response
}

/// The caps of a binding: `"Ctrl+K"` → `Ctrl`, `K`; a sequence `"g i"` → `g`, then, `i`.
/// `None` separates the steps of a sequence.
pub fn caps(binding: &str) -> Vec<Option<&str>> {
    let mut out = Vec::new();
    for (i, step) in binding.split(' ').filter(|s| !s.is_empty()).enumerate() {
        if i > 0 {
            out.push(None);
        }
        if step == "+" {
            out.push(Some(step));
            continue;
        }
        out.extend(step.split('+').filter(|k| !k.is_empty()).map(Some));
    }
    out
}

/// Key caps for `binding` (`Ctrl+K`, `g i`): one cap per key, "then" between sequence steps.
pub fn kbd(ui: &mut Ui, t: &Tokens, binding: &str) -> Response {
    kbd_in(ui, t, binding, false)
}

/// [`kbd`] on an inverse fill (primary buttons, the palette's cursor row).
pub fn kbd_inverse(ui: &mut Ui, t: &Tokens, binding: &str) -> Response {
    kbd_in(ui, t, binding, true)
}

fn kbd_in(ui: &mut Ui, t: &Tokens, binding: &str, inverse: bool) -> Response {
    let token = &t.type_scale.code_badge;
    let (fill, fg, stroke) = if inverse {
        (
            t.inverse_text.gamma_multiply(0.18),
            t.inverse_text,
            t.inverse_text.gamma_multiply(0.4),
        )
    } else {
        (t.selected, t.text_muted, t.border)
    };
    let h = t.m.controls.key_cap;
    let gap = t.m.space.xs / 2.0;
    // One widget for the whole binding, named by it (`Ctrl+K`); the caps are painted.
    let pieces: Vec<(Option<std::sync::Arc<egui::Galley>>, f32)> = caps(binding)
        .into_iter()
        .map(|cap| match cap {
            Some(key) => {
                let galley = ui.painter().layout_no_wrap(key.to_owned(), font(token), fg);
                let w = (galley.size().x + 2.0 * t.m.space.xs + 2.0).max(h);
                (Some(galley), w)
            }
            None => {
                let then = ui.painter().layout_no_wrap(
                    "then".to_owned(),
                    font(&t.type_scale.label_sm),
                    t.text_muted,
                );
                let w = then.size().x;
                (Some(then), -w)
            }
        })
        .collect();
    let width: f32 = pieces.iter().map(|(_, w)| w.abs() + gap).sum::<f32>() - gap;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width.max(0.0), h), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, binding));
    let mut x = rect.left();
    for (galley, w) in pieces {
        let Some(galley) = galley else { continue };
        let cell = Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(w.abs(), h));
        if w > 0.0 {
            ui.painter().rect(
                cell,
                radius(t.m.radius.sm),
                fill,
                Stroke::new(t.m.stroke.hairline, stroke),
                egui::StrokeKind::Inside,
            );
            ui.painter()
                .galley(cell.center() - galley.size() / 2.0, galley, fg);
        } else {
            ui.painter()
                .galley(cell.center() - galley.size() / 2.0, galley, t.text_muted);
        }
        x += w.abs() + gap;
    }
    response
}

/// A label with a dimmed monospace key hint, e.g. `Reply  R`.
pub fn hinted(t: &Tokens, label: &str, keys: Option<&str>, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(label, 0.0, format(&t.type_scale.body_md, color));
    if let Some(keys) = keys {
        job.append(
            keys,
            t.m.space.sm,
            TextFormat {
                font_id: font(&t.type_scale.code_badge),
                color: t.text_muted,
                valign: Align::Center,
                ..Default::default()
            },
        );
    }
    job
}

/// What a badge looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    /// A counter on `emphasis`.
    Count,
    /// Inverse: the open mailbox's count, the unread badge.
    Strong,
    /// A hairline outline, no fill (labels, the mailbox of a search row).
    Outline,
}

/// A badge: fixed height, `code_badge`, radius `sm`; its text is its name.
pub fn badge(ui: &mut Ui, t: &Tokens, text: &str, kind: Badge) -> Response {
    let (fill, fg, stroke) = match kind {
        Badge::Count => (t.emphasis, t.text_strong, Color32::TRANSPARENT),
        Badge::Strong => (t.inverse_bg, t.inverse_text, Color32::TRANSPARENT),
        Badge::Outline => (Color32::TRANSPARENT, t.text_secondary, t.border),
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font(&t.type_scale.code_badge), fg);
    let h = t.m.controls.badge;
    let size = Vec2::new((galley.size().x + 2.0 * t.m.space.sm - 2.0).max(h), h);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, text));
    ui.painter().rect(
        rect,
        radius(t.m.radius.sm),
        fill,
        Stroke::new(t.m.stroke.hairline, stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
    response
}

/// A counter: [`Badge::Strong`] when `strong`, else [`Badge::Count`].
pub fn count(ui: &mut Ui, t: &Tokens, n: impl ToString, strong: bool) -> Response {
    let kind = if strong { Badge::Strong } else { Badge::Count };
    badge(ui, t, &n.to_string(), kind)
}

/// A section caption: `label_sm`, capitals, `text_muted`, with its tracking.
pub fn caption(ui: &mut Ui, t: &Tokens, text: &str) -> Response {
    let job = job(&t.type_scale.label_sm, &text.to_uppercase(), t.text_muted);
    ui.add(egui::Label::new(job).selectable(false))
}

/// The unread pip: a 6 px dot in `color`, or the room for one.
pub fn pip(ui: &mut Ui, t: &Tokens, color: Color32, visible: bool) {
    let side = t.m.controls.unread_pip + 2.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    if visible {
        ui.painter()
            .circle_filled(rect.center(), t.m.controls.unread_pip / 2.0, color);
    }
}

/// A sync state, told apart by shape, not colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dot {
    /// Synced: a filled dot.
    Idle,
    /// Syncing: a ring.
    Busy,
    /// Failing: a ring with a cross.
    Failed,
}

/// A status dot (8 px).
pub fn status_dot(ui: &mut Ui, t: &Tokens, dot: Dot) -> Response {
    let side = t.m.space.sm + 2.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let (c, r) = (rect.center(), t.m.space.xs);
    let p = ui.painter();
    match dot {
        Dot::Idle => {
            p.circle_filled(c, r, t.text_secondary);
        }
        Dot::Busy => {
            p.circle_stroke(c, r, Stroke::new(1.5, t.text_strong));
        }
        Dot::Failed => {
            p.circle_stroke(c, r, Stroke::new(1.5, t.danger));
            let d = r * 0.5;
            let s = Stroke::new(1.5, t.danger);
            p.line_segment([c - Vec2::splat(d), c + Vec2::splat(d)], s);
            p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], s);
        }
    }
    response
}

/// One or two initials of `name` (a display name or an address).
pub fn initials(name: &str) -> String {
    let name = name.split('@').next().unwrap_or(name);
    let mut out: String = name
        .split(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-')
        .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if out.is_empty() {
        out.push('?');
    }
    out
}

/// An initials avatar (28, 32 or 40): never a remote image.
pub fn avatar(ui: &mut Ui, t: &Tokens, name: &str, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), size / 2.0, t.emphasis);
    let token = if size >= t.m.controls.avatar_lg {
        &t.type_scale.headline_md
    } else {
        &t.type_scale.label_sm
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initials(name),
        font(token),
        t.text_strong,
    );
    response
}

/// A 1 px `border_subtle` line across the available width.
pub fn divider(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t.m.stroke.hairline),
        Sense::hover(),
    );
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(t.m.stroke.hairline, t.border_subtle),
    );
}

/// A vertical hairline between toolbar groups.
pub fn vdivider(ui: &mut Ui, t: &Tokens) {
    let h = t.m.controls.button - t.m.space.sm;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(t.m.space.lg, h), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(t.m.stroke.hairline, t.border_subtle),
    );
}

/// How a row of a list is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RowState {
    /// Hovered.
    pub hovered: bool,
    /// Selected, open or checked.
    pub selected: bool,
    /// The keyboard cursor is on it.
    pub cursor: bool,
    /// A hairline at the bottom.
    pub hairline: bool,
}

/// Paints a row's states in `rect`: `hover` and `selected` fills, the cursor bar, the hairline.
pub fn row_frame(ui: &Ui, t: &Tokens, rect: Rect, state: RowState) {
    let p = ui.painter();
    if state.selected {
        p.rect_filled(rect, 0.0, t.selected);
    } else if state.hovered {
        p.rect_filled(rect, 0.0, t.hover);
    }
    if state.cursor {
        p.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(t.m.stroke.indicator, rect.height())),
            0.0,
            t.focus,
        );
    }
    if state.hairline {
        p.hline(
            rect.x_range(),
            rect.bottom() - t.m.stroke.hairline / 2.0,
            Stroke::new(t.m.stroke.hairline, t.border_subtle),
        );
    }
}

/// What a nav row shows at its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trailing {
    /// Nothing.
    None,
    /// An unread count.
    Count(u32),
    /// A key hint.
    Keys(String),
}

/// One sidebar row.
#[derive(Debug, Clone)]
pub struct Nav<'a> {
    /// Its icon (filled when open).
    pub icon: Icon,
    /// What it shows.
    pub label: &'a str,
    /// What it shows at the end.
    pub trailing: Trailing,
    /// The open mailbox.
    pub open: bool,
    /// The sidebar has the keyboard focus.
    pub focused: bool,
    /// Its accessible name.
    pub name: &'a str,
}

/// A sidebar row: 32 high, icon 16, `body_md` label, a trailing count or key; `selected` fill
/// when open; a 2 px `focus` bar when the sidebar has the focus.
pub fn nav_row(ui: &mut Ui, t: &Tokens, nav: &Nav<'_>) -> Response {
    let Nav {
        icon,
        label,
        ref trailing,
        open,
        focused,
        name,
    } = *nav;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t.m.rows.nav),
        Sense::click(),
    );
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, open, name));
    let p = ui.painter();
    if open {
        p.rect_filled(rect, radius(t.m.radius.md), t.selected);
        if focused {
            p.rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(t.m.stroke.indicator, rect.height())),
                0.0,
                t.focus,
            );
        }
    } else if response.hovered() {
        p.rect_filled(rect, radius(t.m.radius.md), t.hover);
    }
    focus_ring(ui, t, &response, rect);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(t.m.space.sm + 2.0, 0.0)))
            .layout(egui::Layout::left_to_right(Align::Center))
            .accessibility_parent(response.id),
    );
    child.spacing_mut().item_spacing.x = t.m.space.sm;
    let color = if open {
        t.text_strong
    } else {
        t.text_secondary
    };
    self::icon(&mut child, icon, t.m.controls.icon_sm, color, open);
    let label = if open {
        text_at(&t.type_scale.body_md, 600, label, color)
    } else {
        text(&t.type_scale.body_md, label, color)
    };
    child.add(egui::Label::new(label).truncate().selectable(false));
    match trailing {
        Trailing::None => {}
        Trailing::Count(n) => {
            child.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                count(ui, t, n, open);
            });
        }
        Trailing::Keys(keys) => {
            child.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(text(&t.type_scale.label_sm, keys.as_str(), t.text_muted));
            });
        }
    }
    response
}

/// The focus ring around `rect` while `response` has the keyboard focus.
pub fn focus_ring(ui: &Ui, t: &Tokens, response: &Response, rect: Rect) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius(t.m.radius.md),
            Stroke::new(t.m.stroke.focus, t.focus),
            egui::StrokeKind::Inside,
        );
    }
}

/// A card: `raised` fill, `border_subtle`, radius `md`, padding `md`.
pub fn card(t: &Tokens) -> Frame {
    Frame::new()
        .fill(t.raised)
        .stroke(Stroke::new(t.m.stroke.hairline, t.border_subtle))
        .corner_radius(radius(t.m.radius.md))
        .inner_margin(Margin::same(margin(t.m.space.md)))
}

/// A card on `base` (a message on the canvas).
pub fn card_base(t: &Tokens) -> Frame {
    card(t).fill(t.base)
}

/// A callout: a card with a `border_strong` outline (agent requests).
pub fn callout(t: &Tokens) -> Frame {
    card(t).stroke(Stroke::new(t.m.stroke.hairline, t.border_strong))
}

/// The frame of an input: `raised` fill, `border`, radius `md`.
pub fn field(t: &Tokens) -> Frame {
    Frame::new()
        .fill(t.raised)
        .stroke(Stroke::new(t.m.stroke.hairline, t.border))
        .corner_radius(radius(t.m.radius.md))
        .inner_margin(Margin::symmetric(
            margin(t.m.space.sm),
            margin(t.m.space.xs),
        ))
}

/// A row with a fixed label column and a field: `label_md` labels, a hairline below.
pub fn labelled<R>(
    ui: &mut Ui,
    t: &Tokens,
    label: &str,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let inner = ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(width, t.m.controls.button),
            egui::Label::new(text(&t.type_scale.label_md, label, t.text_muted)),
        );
        add(ui)
    });
    divider(ui, t);
    inner.inner
}

/// The search pill: `raised`, `border`, search icon, hint, key caps. A click opens what it
/// stands for; its name is `hint`.
pub fn search_pill(ui: &mut Ui, t: &Tokens, hint: &str, keys: &str, width: f32) -> Response {
    let h = t.m.controls.button + t.m.space.xs;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, hint));
    let fill = if response.hovered() {
        t.hover
    } else {
        t.raised
    };
    ui.painter().rect(
        rect,
        radius(t.m.radius.md),
        fill,
        Stroke::new(t.m.stroke.hairline, t.border),
        egui::StrokeKind::Inside,
    );
    focus_ring(ui, t, &response, rect);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(t.m.space.md, 0.0)))
            .layout(egui::Layout::left_to_right(Align::Center))
            .accessibility_parent(response.id),
    );
    child.spacing_mut().item_spacing.x = t.m.space.sm;
    icon(
        &mut child,
        Icon::Search,
        t.m.controls.icon_sm,
        t.text_muted,
        false,
    );
    child.add(
        egui::Label::new(text(&t.type_scale.body_md, hint, t.text_muted))
            .truncate()
            .selectable(false),
    );
    child.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
        kbd(ui, t, keys);
    });
    response
}

/// A segmented control: an `emphasis` track; the chosen segment `raised` in `text_strong`.
/// Returns the segment clicked. Each segment is a button named by its label, selected when chosen.
pub fn segmented(ui: &mut Ui, t: &Tokens, labels: &[&str], chosen: usize) -> Option<usize> {
    let mut clicked = None;
    Frame::new()
        .fill(t.emphasis)
        .corner_radius(radius(t.m.radius.md))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, label) in labels.iter().enumerate() {
                    let on = i == chosen;
                    let color = if on { t.text_strong } else { t.text_secondary };
                    let button = egui::Button::new(text(&t.type_scale.body_sm, *label, color))
                        .fill(if on { t.raised } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(radius(t.m.radius.sm))
                        .min_size(Vec2::new(0.0, t.m.controls.button_small));
                    let response = ui.add(button);
                    response
                        .widget_info(|| WidgetInfo::selected(egui::Role::Button, true, on, *label));
                    if response.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    clicked
}

/// A switch: a 44 × 24 track (`inverse_bg` when on, a `border_strong` outline when off) and a
/// 20 px knob. A checkbox to screen readers, named `name`. Returns its response; flip on click.
pub fn switch(ui: &mut Ui, t: &Tokens, on: bool, name: &str) -> Response {
    let size = Vec2::new(t.m.controls.switch_width, t.m.controls.switch_height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::Role::CheckBox, true, on, name));
    let r = rect.height() / 2.0;
    let how_on = ui.ctx().animate_bool(response.id, on);
    let p = ui.painter();
    if on {
        p.rect_filled(rect, radius(r), t.inverse_bg);
    } else {
        p.rect(
            rect,
            radius(r),
            Color32::TRANSPARENT,
            Stroke::new(t.m.stroke.hairline, t.border_strong),
            egui::StrokeKind::Inside,
        );
    }
    let knob = r - 2.0;
    let x = egui::lerp((rect.left() + r)..=(rect.right() - r), how_on);
    let knob_color = if on { t.inverse_text } else { t.text_secondary };
    p.circle_filled(egui::pos2(x, rect.center().y), knob, knob_color);
    if response.has_focus() {
        p.rect_stroke(
            rect.expand(2.0),
            radius(r + 2.0),
            Stroke::new(t.m.stroke.focus, t.focus),
            egui::StrokeKind::Outside,
        );
    }
    response
}

/// Tabs: `body_sm` in `text_muted`; the active tab `raised` in `text_strong`, with its icon.
/// Returns the tab clicked. Each tab is a button named by its label, selected when active.
pub fn tabs(ui: &mut Ui, t: &Tokens, items: &[(Icon, &str)], active: usize) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.xs;
        for (i, (icon, label)) in items.iter().enumerate() {
            let on = i == active;
            let color = if on { t.text_strong } else { t.text_muted };
            let job = icon_text(*icon, label, font(&t.type_scale.body_sm), color);
            let button = egui::Button::new(job)
                .fill(if on { t.raised } else { Color32::TRANSPARENT })
                .stroke(Stroke::NONE)
                .corner_radius(radius(t.m.radius.md))
                .min_size(Vec2::new(0.0, t.m.controls.button));
            let response = ui.add(button);
            response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, on, *label));
            if response.clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// A toolbar: a row of `reader_toolbar` height, wrapping at large text scales, with a hairline
/// below. Split groups with [`vdivider`].
pub fn toolbar<R>(ui: &mut Ui, t: &Tokens, add: impl FnOnce(&mut Ui) -> R) -> R {
    let inner = ui.horizontal_wrapped(|ui| {
        ui.set_min_height(t.m.frame.reader_toolbar);
        ui.spacing_mut().item_spacing.x = t.m.space.xs + 2.0;
        add(ui)
    });
    divider(ui, t);
    inner.inner
}

/// A modal frame: `base` fill, `border`, radius `lg`, no shadow.
pub fn overlay(t: &Tokens) -> Frame {
    Frame::new()
        .fill(t.base)
        .stroke(Stroke::new(t.m.stroke.hairline, t.border))
        .corner_radius(radius(t.m.radius.lg))
        .inner_margin(Margin::same(margin(t.m.space.lg)))
}

/// Dims the whole screen behind an overlay with the `scrim`.
pub fn scrim(ctx: &egui::Context, t: &Tokens) {
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("dail-scrim"));
    ctx.layer_painter(layer)
        .rect_filled(ctx.content_rect(), 0.0, t.scrim);
}

/// A toast's frame: `inverse_bg`, radius `lg`.
pub fn toast(t: &Tokens) -> Frame {
    Frame::new()
        .fill(t.inverse_bg)
        .corner_radius(radius(t.m.radius.lg))
        .inner_margin(Margin::symmetric(
            margin(t.m.space.md),
            margin(t.m.space.sm),
        ))
}

/// Key caps then a label, as in footers and empty states: `j` `k` move.
pub fn hint(ui: &mut Ui, t: &Tokens, keys: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.xs;
        kbd(ui, t, keys);
        ui.label(text(
            &t.type_scale.label_sm,
            label.to_uppercase(),
            t.text_muted,
        ));
    });
}

/// A footer's hints: `label_sm` capitals in `text_muted`, with key caps, `·` between them.
pub fn footer_hint(ui: &mut Ui, t: &Tokens, hints: &[(&str, &str)]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.sm;
        for (i, (keys, label)) in hints.iter().enumerate() {
            if i > 0 {
                ui.label(text(&t.type_scale.label_sm, "·", t.text_muted));
            }
            hint(ui, t, keys, label);
        }
    });
}

/// An empty state: `headline_md` in `text_secondary`, then hints with key caps.
pub fn empty_state(ui: &mut Ui, t: &Tokens, title: &str, hints: &[(&str, &str)]) {
    ui.vertical_centered(|ui| {
        ui.add_space(t.m.space.xl);
        ui.label(text(&t.type_scale.headline_md, title, t.text_secondary));
        if !hints.is_empty() {
            ui.add_space(t.m.space.sm);
            footer_hint(ui, t, hints);
        }
    });
}

/// `text` with the occurrences of `marks` (search words, `app_front::highlight`) on `emphasis`
/// in `text_strong`.
pub fn marked(t: &Tokens, text: &str, marks: &[String], font: FontId, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (piece, marked) in app_front::highlight::pieces(text, marks) {
        job.append(
            piece,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: if marked { t.text_strong } else { color },
                background: if marked {
                    t.emphasis
                } else {
                    Color32::TRANSPARENT
                },
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_words_are_marked() {
        let t = Tokens::of(&app_theme::builtin::light());
        let job = marked(
            &t,
            "a Budget!",
            &["budget".to_owned()],
            mono(12.0),
            t.text_secondary,
        );
        assert_eq!(job.text, "a Budget!");
        let backgrounds: Vec<Color32> = job.sections.iter().map(|s| s.format.background).collect();
        assert_eq!(
            backgrounds,
            [Color32::TRANSPARENT, t.emphasis, Color32::TRANSPARENT],
            "only the word"
        );
    }

    #[test]
    fn bindings_split_into_caps() {
        assert_eq!(caps("Ctrl+K"), [Some("Ctrl"), Some("K")]);
        assert_eq!(caps("g i"), [Some("g"), None, Some("i")]);
        assert_eq!(caps("Ctrl++"), [Some("Ctrl")]);
        assert_eq!(caps("#"), [Some("#")]);
    }

    #[test]
    fn initials_of_names_and_addresses() {
        assert_eq!(initials("Alice Liddell"), "AL");
        assert_eq!(initials("bob@example.com"), "B");
        assert_eq!(initials("jane.doe@x.org"), "JD");
        assert_eq!(initials(""), "?");
    }
}
