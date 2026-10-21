//! The bundled fonts (UI-2): six families, egui's fonts as fallbacks, weights by token.

use crate::support::{apply, gui, synced_state};
use app_core::{Command, SettingKey};
use app_gui::theme;
use app_theme::TypeScale;
use egui::{FontFamily, FontId, TextStyle};

#[test]
fn fonts_are_installed_with_fallbacks() {
    let mut h = gui(synced_state());
    h.run();
    let families = h.ctx.fonts(|f| f.families());
    for name in [
        "sans", "sans-500", "sans-600", "mono", "mono-500", "mono-600",
    ] {
        assert!(
            families.contains(&FontFamily::Name(name.into())),
            "{name} in {families:?}"
        );
    }
    let defs = h.ctx.fonts(|f| f.definitions().clone());
    assert_eq!(
        defs.families[&FontFamily::Proportional]
            .first()
            .map(String::as_str),
        Some("Geist-400")
    );
    assert_eq!(
        defs.families[&FontFamily::Monospace]
            .first()
            .map(String::as_str),
        Some("JetBrains Mono-400")
    );
    // Latin, Greek (from the fallback: Geist has none) and Cyrillic, in every weight.
    for name in ["sans", "sans-600", "mono"] {
        let font = FontId::new(13.0, FontFamily::Name(name.into()));
        for sample in ["Hello", "Ελληνικά", "Кириллица"] {
            assert!(
                h.ctx.fonts_mut(|f| f.has_glyphs(&font, sample)),
                "{name}: {sample}"
            );
        }
    }
}

#[test]
fn text_styles_follow_the_type_scale() {
    let mut h = gui(synced_state());
    h.run();
    let style = h.ctx.global_style();
    let scale = TypeScale::DEFAULT;
    assert_eq!(style.text_styles[&TextStyle::Body].size, scale.body_md.size);
    assert_eq!(
        style.text_styles[&TextStyle::Button].family,
        FontFamily::Name("sans-500".into())
    );
    assert_eq!(
        style.text_styles[&TextStyle::Heading].family,
        FontFamily::Name("sans-600".into())
    );
    assert_eq!(
        style.text_styles[&TextStyle::Monospace].family,
        FontFamily::Name("mono-500".into())
    );
}

#[test]
fn unread_rows_are_semibold() {
    let scale = TypeScale::DEFAULT;
    // The text helper: a token at 600, as an unread row's sender.
    assert_eq!(
        theme::font_at(&scale.body_md, 600).family,
        FontFamily::Name("sans-600".into())
    );
    assert_eq!(theme::semibold(), FontFamily::Name("sans-600".into()));
    assert_eq!(
        theme::font(&scale.label_sm).family,
        FontFamily::Name("mono-500".into())
    );
}

#[test]
fn every_icon_has_a_glyph_in_both_fills() {
    let mut h = gui(synced_state());
    h.run();
    for filled in [false, true] {
        let font = theme::icon_font(18.0, filled);
        for icon in app_theme::Icon::ALL {
            assert!(
                h.ctx.fonts_mut(|f| f.has_glyph(&font, icon.char())),
                "{icon:?} (filled: {filled})"
            );
        }
    }
}

#[test]
fn every_mailbox_role_has_its_icon() {
    use app_core::MailboxRole;
    use app_theme::Icon;
    let roles = [
        (Some(MailboxRole::Inbox), Icon::Inbox),
        (Some(MailboxRole::Sent), Icon::Send),
        (Some(MailboxRole::Drafts), Icon::Draft),
        (Some(MailboxRole::Trash), Icon::Delete),
        (Some(MailboxRole::Archive), Icon::Archive),
        (Some(MailboxRole::Junk), Icon::Report),
        (Some(MailboxRole::All), Icon::AllInbox),
        (Some(MailboxRole::Flagged), Icon::Star),
        (None, Icon::Folder),
    ];
    for (role, icon) in roles {
        assert_eq!(theme::role_icon(role), icon, "{role:?}");
    }
}

#[test]
fn apply_installs_the_fills_of_the_chosen_theme() {
    let mut h = gui(synced_state());
    h.run();
    for (setting, id, dark) in [
        (1_u64, "dail-light", false),
        (2, "dail-dark", true),
        (3, "dail-dark-hc", true),
        (4, "dail-light-hc", false),
    ] {
        apply(
            &mut h,
            Command::SetSetting {
                key: SettingKey::Theme,
                value: setting.into(),
            },
        );
        h.run();
        let surface = app_theme::builtin::get(id).unwrap().content;
        let v = h.ctx.global_style().visuals.clone();
        assert_eq!(v.dark_mode, dark, "{id}");
        assert_eq!(v.panel_fill, theme::color(surface.canvas), "{id}");
        assert_eq!(v.window_fill, theme::color(surface.base), "{id}");
        assert_eq!(v.error_fg_color, theme::color(surface.danger), "{id}");
        assert_eq!(theme::tokens(&h.ctx).id, id);
    }
}

#[test]
fn the_gallery_draws_every_theme() {
    for theme in app_theme::builtin::all() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            app_gui::ui::gallery::show(ui, &theme);
        });
        out.textures_delta.clear();
    }
}
