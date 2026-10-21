//! The style check (UI plan §3.8, UI-4): outside the kit (`src/ui/`) and the adapter
//! (`src/theme.rs`), the GUI reads colour roles and metrics from the tokens and never writes a
//! raw style value. Modules not moved over yet are on the allowlist; each part B item takes its
//! modules off, and after UI-10 the list is empty.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

/// Modules that still hold literal sizes, radii, margins and spaces.
const ALLOWLIST: &[&str] = &[
    "approvals.rs",
    "composer.rs",
    "palette.rs",
    "shell.rs",
    "toasts.rs",
    "wizard.rs",
];

/// A raw style value on `line`, if any: a constructor of colours or fonts, or a literal number
/// passed to a size, a radius, a margin, a space or a stroke.
fn raw_style(line: &str) -> Option<&'static str> {
    const CALLS: [&str; 5] = [
        "Color32::from_",
        "FontId::proportional(",
        "FontId::monospace(",
        "FontId::new(",
        "CornerRadius::same(",
    ];
    const NUMBERED: [&str; 6] = [
        ".size(",
        "corner_radius(",
        "Margin::same(",
        "Margin::symmetric(",
        "add_space(",
        "Stroke::new(",
    ];
    let code = line.split("//").next().unwrap_or_default();
    if let Some(call) = CALLS.into_iter().find(|c| code.contains(c)) {
        return Some(call);
    }
    NUMBERED.into_iter().find(|call| {
        code.match_indices(call).any(|(at, _)| {
            code[at + call.len()..]
                .trim_start()
                .starts_with(|c: char| c.is_ascii_digit() || c == '-')
        })
    })
}

fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.extension().is_some_and(|e| e == "rs") && name != "theme.rs" {
            out.push((name, std::fs::read_to_string(&path).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn widgets_read_roles_and_metrics_only() {
    let mut found = Vec::new();
    for (name, source) in sources() {
        if ALLOWLIST.contains(&name.as_str()) {
            continue;
        }
        // Unit tests at the end of a module may build values freely.
        let code = source.split("#[cfg(test)]").next().unwrap_or_default();
        for (i, line) in code.lines().enumerate() {
            if let Some(what) = raw_style(line) {
                found.push(format!("src/{name}:{}: {what}  {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "raw style values outside the kit (use the tokens or a `ui` component):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_allowlist_only_names_modules_that_need_it() {
    let sources = sources();
    for name in ALLOWLIST {
        let (_, source) = sources
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is gone: take it off the allowlist"));
        assert!(
            source.lines().any(|l| raw_style(l).is_some()),
            "{name} is clean: take it off the allowlist"
        );
    }
}

#[test]
fn the_check_sees_raw_values() {
    assert_eq!(raw_style("ui.add_space(8.0);"), Some("add_space("));
    assert_eq!(raw_style("ui.add_space(t.m.space.sm);"), None);
    assert_eq!(
        raw_style("let c = Color32::from_rgb(1, 2, 3);"),
        Some("Color32::from_")
    );
    assert_eq!(raw_style("RichText::new(x).size(18.0)"), Some(".size("));
    assert_eq!(raw_style("// ui.add_space(8.0);"), None);
}
