//! The contrast audit: the role pairs the components draw, against WCAG 2.x minimums.

use crate::{Surface, Theme, contrast};

/// Text (WCAG 1.4.3, AA).
pub const TEXT_MIN: f64 = 4.5;
/// Focus rings and control outlines (WCAG 1.4.11).
pub const NON_TEXT_MIN: f64 = 3.0;

/// A role pair below its minimum.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// `content` or `chrome`.
    pub group: &'static str,
    pub fg: &'static str,
    pub bg: &'static str,
    pub ratio: f64,
    pub min: f64,
}

/// Every pair of `theme` that fails, in both surface groups. Empty means the theme passes.
pub fn audit(theme: &Theme) -> Vec<Problem> {
    let mut problems = Vec::new();
    for (group, s) in [("content", &theme.content), ("chrome", &theme.chrome)] {
        for (fg, bg, min) in pairs(s) {
            let ratio = contrast(fg.1, bg.1);
            if ratio < min {
                problems.push(Problem {
                    group,
                    fg: fg.0,
                    bg: bg.0,
                    ratio,
                    min,
                });
            }
        }
    }
    problems
}

type Role = (&'static str, crate::Rgba);

fn pairs(s: &Surface) -> Vec<(Role, Role, f64)> {
    let base = ("base", s.base);
    let canvas = ("canvas", s.canvas);
    let raised = ("raised", s.raised);
    let hover = ("hover", s.hover);
    let selected = ("selected", s.selected);
    let emphasis = ("emphasis", s.emphasis);
    let inverse_bg = ("inverse_bg", s.inverse_bg);
    let mut out = Vec::new();
    let all_bgs = [base, canvas, raised, hover, selected, emphasis];
    for fg in [
        ("text_strong", s.text_strong),
        ("text", s.text),
        ("text_secondary", s.text_secondary),
    ] {
        out.extend(all_bgs.map(|bg| (fg, bg, TEXT_MIN)));
    }
    // Never drawn on `emphasis`.
    for fg in [("text_muted", s.text_muted), ("danger", s.danger)] {
        out.extend([base, canvas, raised, hover, selected].map(|bg| (fg, bg, TEXT_MIN)));
    }
    for fg in [
        ("inverse_text", s.inverse_text),
        ("inverse_danger", s.inverse_danger),
    ] {
        out.push((fg, inverse_bg, TEXT_MIN));
    }
    out.extend([base, raised, selected].map(|bg| (("focus", s.focus), bg, NON_TEXT_MIN)));
    out.extend([base, raised].map(|bg| (("border_strong", s.border_strong), bg, NON_TEXT_MIN)));
    out
}
