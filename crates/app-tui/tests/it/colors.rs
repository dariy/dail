//! Opt-in theme colours (UI-16): off by default, truecolor with `tui_colors = 1`, never with
//! `NO_COLOR`.

use crate::support::{Term, inbox_of, synced_state};
use app_core::{Command, SettingKey};
use ratatui::style::{Color, Modifier};

fn colored(term: &Term) -> usize {
    term.terminal
        .backend()
        .buffer()
        .content
        .iter()
        .filter(|c| matches!(c.fg, Color::Rgb(..)) || matches!(c.bg, Color::Rgb(..)))
        .count()
}

fn opened() -> Term {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut term = Term::new(state, 80, 24);
    term.apply(Command::OpenMailbox { id: inbox });
    term.apply(Command::SelectThreads {
        ids: vec![threads[0]],
    });
    term
}

#[test]
fn the_terminal_colours_are_the_default() {
    let mut term = opened();
    term.screen();
    assert_eq!(colored(&term), 0);
}

#[test]
fn tui_colors_paint_the_theme_roles() {
    let mut term = opened();
    let before = term.screen();
    assert!(
        term.terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|c| c.modifier.contains(Modifier::REVERSED)),
        "the cursor row is reversed"
    );
    term.apply(Command::SetSetting {
        key: SettingKey::TuiColors,
        value: 1.into(),
    });
    assert_eq!(term.screen(), before, "the text does not change");
    let buffer = term.terminal.backend().buffer();
    assert_eq!(
        colored(&term),
        buffer.content.len(),
        "every cell is painted"
    );
    let dark = app_theme::builtin::dark();
    let rgb = |c: app_theme::Rgba| Color::Rgb(c.r, c.g, c.b);
    // A body cell has the canvas fill; the cursor row is a selection, not reversed.
    let middle = &buffer.content[usize::from(buffer.area.width) * 12 + 40];
    assert_eq!(middle.bg, rgb(dark.content.canvas));
    assert!(
        buffer
            .content
            .iter()
            .all(|c| !c.modifier.contains(Modifier::REVERSED))
    );
    assert!(
        buffer
            .content
            .iter()
            .any(|c| c.bg == rgb(dark.content.selected))
    );
    // Light mode takes the light theme.
    term.apply(Command::SetSetting {
        key: SettingKey::Theme,
        value: 1.into(),
    });
    term.screen();
    let light = app_theme::builtin::light();
    let buffer = term.terminal.backend().buffer();
    let middle = &buffer.content[usize::from(buffer.area.width) * 12 + 40];
    assert_eq!(
        middle.bg,
        Color::Rgb(
            light.content.canvas.r,
            light.content.canvas.g,
            light.content.canvas.b
        )
    );
}

#[test]
fn no_color_wins() {
    let mut term = opened();
    term.tui = app_tui::tui::Tui::with_clock(crate::support::test_now).with_no_color(true);
    term.apply(Command::SetSetting {
        key: SettingKey::TuiColors,
        value: 1.into(),
    });
    term.screen();
    assert_eq!(colored(&term), 0);
}
