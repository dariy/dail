# 0008 — `app-theme`: design tokens and built-in themes

- Status: accepted
- Date: 2026-10-21
- Plan: `docs/requests/2026-10-21-ui.md` (UI-1; recommendations D1–D10 in §8)

## Context

The GUI's look lives in `app_gui::theme` (a ten-colour `Palette`) and in about 140 literal sizes, radii, margins and
spaces spread over its modules. The visual refresh after the mockups (`docs/ui/`) needs a library of style elements
that themes are made of, a dark and a light theme now, and a way to add colour schemes later without touching
widgets. A future web front-end and the TUI (UI-16) may read the same tokens.

## Decision

- A new crate, `crates/app-theme`, with no internal dependency and `serde` as its only external one. Not an
  `app-front` module: tokens need nothing from `app-core`, and the fonts and icons (UI-2, UI-3) are assets the
  front-end logic crate should not carry. Edges: `app-theme ← (nothing internal)`; `app-gui` takes it in UI-4,
  `app-tui` only with UI-16.
- **Three layers.** Primitives (grey values, type scale, spacing, radii) → roles per theme (`Surface`) → components
  (`app_gui::ui`, UI-4), which read roles and metrics only. The egui adapter converts a `Theme` once per change.
- **Roles.** `base`, `canvas`, `raised`, `hover`, `selected`, `emphasis`, `scrim`, `border_subtle`, `border`,
  `border_strong`, `focus`, `text_strong`, `text`, `text_secondary`, `text_muted`, `inverse_bg`, `inverse_text`,
  `danger`, `inverse_danger`. A `Theme` has two groups of them: `content` (list, reader, dialogs) and `chrome`
  (sidebar, top bar). A flat theme sets them equal; `dail-light` has dark chrome (D2).
- **Built-ins.** `dail-dark`, `dail-light`, `dail-dark-hc`, `dail-light-hc` (`builtin::all()`, `builtin::get(id)`).
  The IDs are stable slugs stored in the settings. The `TypeScale` and `Metrics` are separate values per theme, all
  built-ins share the defaults. Row density is a user setting (UI-11), not part of a theme.
- **Contrast audit.** `audit(&Theme)` checks the role pairs components draw against WCAG 2.x (4.5:1 for text, 3:1 for
  the focus ring and control outlines), in both groups. It runs as a unit test over every built-in, and will gate
  theme files later.
- **Fonts and icons** (UI-2, UI-3) are bundled in the crate: Geist and JetBrains Mono (OFL-1.1), a Material Symbols
  Outlined subset (Apache-2.0), each with its licence text. The crate's `license` then becomes
  `(MIT OR Apache-2.0) AND OFL-1.1 AND Apache-2.0`; `deny.toml` already allows OFL-1.1.
- **Settings by name** (UI-11): the `theme` setting names a theme ID. The legacy numbers keep their meaning:
  0 system, 1 light, 2 dark, 3 `dail-dark-hc`, 4 `dail-light-hc`.

## Consequences

- A colour scheme is one `Theme` value that passes the audit; no widget, command or setting changes.
- One more workspace crate (small, builds in seconds).
- Until UI-4 nothing uses the crate; `app_gui::theme` stays the source of the GUI's colours.
