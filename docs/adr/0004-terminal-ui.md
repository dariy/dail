# 0004 — A terminal front-end, `app-tui`, and a shared `app-front`

- Status: accepted
- Date: 2026-10-18
- Plan: `ROADMAP.md`, Wave 1.5

## Context

The user wants to read and answer mail from a phone or an iPad over SSH (Termius, Blink). The egui GUI can't do that,
and it can't be tested by an agent without desktop automation either. A terminal UI works over SSH, and under `tmux`
an agent can type into it (`send-keys`) and read the screen (`capture-pane -p`).

A second front-end must not re-derive the GUI's rules. Today the palette flows (how each command gets its arguments),
the fuzzy ranking, the keymap (bindings from `CommandSpec.default_keys`, the `g i` sequences, arguments from the
view), the formatting, the status and approval projections and the reader's body tracking all live in `app-gui`, mixed
with egui code. The `Bridge` that runs a `Session` behind a UI thread is in `app-gui` too, and it takes a `Session`, so it
can't attach to another process.

A terminal is also a hostile output device. Email text that contains `ESC`, C1 controls or bidi overrides can clear the
screen, rewrite the window title, set the clipboard (OSC 52) or make a link look like another one. Over SSH, a browser or
a file dialog would open on the host, not on the phone.

## Decision

- **Crates.**
  - `app-front` (← `app-core` only; no toolkit, no tokio) holds the toolkit-free half of `app-gui`: palette flows,
    fuzzy ranking (`nucleo-matcher`), keymap, formatting, status and approval projections, body tracking, address
    parsing and receipt texts. Both front-ends use it.
  - `app-tui` (← `app-core`, `app-sync`, `app-front`) uses `ratatui` 0.30, its `ratatui::crossterm` re-export
    (crossterm 0.29, so the two never drift) and `ratatui-textarea` 0.9. All are MIT, MSRV ≤ 1.88 and pure Rust.
  - `app-gui` and `app-tui` do not depend on each other. The TUI never links egui.
  - `app-cli` gains `app-tui` and the `dail tui` subcommand.
- **Session.** The `Bridge` moves to `app-sync` and runs over a `Connection`: it submits with the front-end's origin,
  wakes the front-end through a callback, tags each result for it, and shuts the session down only if it owns it. The
  TUI attaches first, like `dail mcp` (ARCHITECTURE §5.4); else it owns the profile and listens on its socket. Attached
  front-ends share one `ViewState`. An IPC `watch` notification tells attached clients that the state changed.
- **Origin.** A new user origin, `tui`. It may approve agent sends (`Origin::is_user`), and an IPC client may claim it,
  like `cli`. Clients still can't claim `gui` or `runtime`.
- **Layout** is a function of `ViewState` and the terminal size. A resize never dispatches a command, because the view
  is shared with the GUI and with agents.
- **Keys.** One keymap, from `CommandSpec.default_keys`. The TUI assumes a plain xterm-like terminal: Ctrl+Enter
  arrives as Enter, Ctrl+Shift+letter as Ctrl+letter, F-keys and Esc may be missing (iPad keyboards), Ctrl+B and
  Ctrl+A belong to tmux and screen, and there is no OSC 52 and no kitty keyboard protocol. The registry gets keys that
  such a terminal delivers next to the existing ones. TUI-local keys only scroll, show help (`?`), stand in for Escape
  (`q`, Backspace) or quit.
- **Terminal hygiene.** Every widget draws email text through one sanitizer (C0 and C1 controls, ESC, DEL and bidi
  overrides replaced, tabs expanded). No OSC 8 links, no window title. Only the terminal's own colours (default
  foreground and background, bold, dim, reverse; `NO_COLOR` respected) and single-width glyphs, so light and dark
  terminal themes both work. In TUI mode `tracing` writes to a file in the profile's `logs/`, because stderr is the
  screen. The TUI never opens a browser or a file dialog.
- **Tests.** ratatui's `TestBackend` with a real `Engine`, like `app-gui/tests`; `insta` snapshots of the buffer at
  40×20, 80×24 and 120×40; the clock injected. No TTY is needed in CI.

## Consequences

- `AGENTS.md §2` and `ARCHITECTURE.md §3` list the two crates and their edges. `deny.toml` must accept crossterm's
  dependencies (all pure Rust; on Windows `crossterm_winapi`/`winapi`).
- Moving code out of `app-gui` changes no behaviour: the GUI's tests stay green, and the palette's parity walk moves
  to `app-front` so that both front-ends are checked against the same flows.
- The TUI shares the GUI's limits (no multi-select, no collapsed quotes, no HTML rendering). Over a plain SSH login
  the OS keyring is often unreachable (no D-Bus session); the status line shows the keyring error, and attaching to a
  GUI in the desktop session is the workaround.
- An attached TUI reads the whole state snapshot again after each change notification. That is fine for a few
  thousand messages; deltas come later.
