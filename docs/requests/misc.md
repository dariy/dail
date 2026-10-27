# dail — Feature requests: miscellaneous

Small requests recorded with `/new-task`. MISC-1 to MISC-7 are scheduled in `ROADMAP.md` Wave 6, MISC-8 to MISC-14 in Wave 8. When an item is picked, move it to `ROADMAP.md`
(keep its ID) and build it in the AGENTS.md §3 order. IDs are stable: never reuse or renumber them.

## Summary

| ID | Feature | Size | Touches | Added |
|----|---------|------|---------|-------|
| MISC-1 | Android: list swipes do nothing | S | android/, app-mobile | 2026-10-26 |
| MISC-2 | Android: Mailboxes button as an icon button | S | android/, app-mobile | 2026-10-26 |
| MISC-3 | Mobile: About in the sidebar, with a close button | S | app-mobile, android/, ios/ | 2026-10-26 |
| MISC-4 | Pin threads to the top of the message list | M | app-core, app-io, app-sync, app-testkit, app-front, app-gui, app-tui, app-mobile, android/, ios/ | 2026-10-26 |
| MISC-5 | Mobile: swipe the message body to go to the previous/next thread | S | app-mobile, android/, ios/ | 2026-10-26 |
| MISC-6 | Mobile: pinch resizes the message pane; landscape sidebar as icons; fix Android landscape alignment | M | app-mobile, android/, ios/ | 2026-10-26 |
| MISC-7 | Mobile: close button on the open message | S | app-mobile, android/, ios/ | 2026-10-26 |
| MISC-8 | Release: the ubuntu job fails on `main` (`desktop-file-validate` missing) | S | .github/workflows/release.yml | 2026-10-27 |
| MISC-9 | Release: every release increments the app version | M | .github/workflows/, xtask | 2026-10-27 |
| MISC-10 | Android: folder tree in the left pane | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-11 | Mobile: landscape left pane switches between icon rail and icon + text tree | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-12 | iOS: About cannot be closed and covers the other panes | S | ios/, app-mobile | 2026-10-27 |
| MISC-13 | Mobile: About title with the version, mobile description | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-14 | Mobile: maximize the message pane | S | app-mobile, android/, ios/ | 2026-10-27 |

## Details

### MISC-1 — Android: list swipes do nothing

On Android, swiping a row left or right in the message list does nothing. It fails on every row: the row slides and
shows the archive / trash icon, but on release nothing happens (the thread stays, no undo snackbar). Other platforms
are not affected. Wave 4 shipped this as done
(`ROADMAP.md`: "swipe left archives, right trashes"), so this is a regression or a gap in the device path: the swipe
should archive (left) or trash (right) the thread, with undo.

- **Commands:** none (existing `archive_threads` / trash via the presenter's swipe `UiEvent`).
- **Touches:** `android/` (`ui/ListScreen.kt` gesture handling), possibly `app-mobile` (swipe event → command).
- **Open questions:** none. The gesture is recognised, so the fault is after release: the swipe event is not sent, or
  the presenter drops it.

### MISC-2 — Android: Mailboxes button as an icon button

The top bar's navigation button on the list screen shows the text *Mailboxes*. Show it as an icon button (a menu
icon) instead; keep "Mailboxes" as its accessible name.

- **Commands:** none (the button already runs `focus_pane { pane: sidebar }`).
- **Touches:** `android/` (top-bar rendering of the navigation button); possibly `app-mobile` (`mobile.rs`, if the
  frame must carry an icon hint for button `menu`).
- **Open questions:** none.

### MISC-3 — Mobile: About in the sidebar, with a close button

On mobile, the About entry sits in the top-right corner. Move it to the left panel (sidebar/drawer), as the entry
directly after *Settings*. The About pane must also have a visible way to close it (a close / back button in its top
bar), not only the system Back gesture.

- **Commands:** none (About is a presenter-local UI event, `mobile.rs` `About`; closing reuses the presenter's back
  handling).
- **Touches:** `app-mobile` (`mobile.rs` sidebar entries and top-bar actions, `frame.rs` About frame with a close
  action), `android/` (`ui/DailScreen.kt`), `ios/` (SwiftUI drawer and About view); contract frames under
  `android/app/src/test/resources/contract/frames/` (`about.json`, `drawer_360x800.json`).
- **Open questions:** none.

### MISC-4 — Pin threads to the top of the message list

Let the user pin a thread in the message list. Pinned threads show at the top of the list, above the date-sorted
threads, until they are unpinned.

- **Commands:** new `pin_threads { ids }` and `unpin_threads { ids }`, `Mutation` (undoable, like `star`), agent
  access `any`. The list projection orders pinned threads first.
- **Decisions:** the pin lives on the server, as the IMAP keyword `$Pinned` (Gmail exposes keywords as
  labels), so it syncs like `\Flagged`. A pin is global: a pinned thread is at the top of every list that shows it.
  Pinned threads are sorted by date among themselves, newest first.
  If the server does not allow custom keywords (`PERMANENTFLAGS` without `\*`), the pin is kept on the device only.
  Hostinger allows them: `live_it::hostinger_allows_custom_keywords` (`app-sync`, ignored) passed on 2026-10-26.
- **Touches:** `app-core` (pinned state, commands, list ordering, registry, replay fixture), `app-io` (persisting the
  pin), `app-sync` + `app-testkit` (keyword STORE and fetch), `app-front` / `app-gui` / `app-tui` / `app-mobile` (pin
  action, keybinding, pin marker), `android/`, `ios/`.
- **Open questions:** none.

### MISC-5 — Mobile: swipe the message body to go to the previous/next thread

In the mobile message pane, remove the navigation bar ("up · n of m · down"). Instead, a horizontal swipe on the
message body goes to the previous or next thread in the list, as those buttons do now.

- **Commands:** none (reuses the thread navigation the buttons already run).
- **Touches:** `app-mobile` (`frame.rs` / `mobile.rs` message frame: drop the pager, keep prev/next actions),
  `android/` (`ui/DailScreen.kt`), `ios/` (SwiftUI message view); contract frames under
  `android/app/src/test/resources/contract/frames/`.
- **Decisions:** swipe left = next thread, swipe right = previous. Long lines wrap. If the content is wider than the
  viewport (an image, a wide table), a horizontal drag scrolls the content first; once its edge is reached, the
  continued drag changes the thread.
- **Open questions:** none.

### MISC-6 — Mobile: pinch resizes the message pane; landscape sidebar as icons; fix Android landscape alignment

1. **Pinch on the message body changes the layout, not the text size.** Pinch-out (zoom in) enlarges the message pane
   by shrinking or hiding the other open panes (sidebar, message list). Pinch-in (zoom out) does the opposite:
   it brings the panes back, one step per gesture.
2. **Landscape sidebar collapses to icons.** In landscape, the left pane (mailboxes) shrinks to an icon rail instead of
   full width.
3. **Android landscape alignment is wrong.** Check and fix the landscape layout on Android. The user cannot check the
   iPhone, so the iOS landscape layout needs the same check in the simulator (CI).

- **Commands:** none expected; the pane steps are view state in the `app-mobile` presenter (UI events → layout). If
  agents must drive the pane layout, a `View` command (all agents) would be needed — decide when picked.
- **Touches:** `app-mobile` (layout/breakpoints, pane-step events, frames), `android/` (pinch gesture, icon rail,
  landscape fix), `ios/` (same).
- **Decisions:** the pane step persists across threads (it does not reset per message).
- **Android landscape misalignment (as observed):** the left mailbox pane is too wide, and its text is not aligned
  with the rest. Mailbox names start at the horizontal position of the word "ago" in the "synced 1m ago" line. That
  line is aligned correctly. The pane text must start at the same left edge as that line.
- **Open questions:** none.

### MISC-7 — Mobile: close button on the open message

Put a close (×) button in the top-right corner of the open message, next to its title. Tapping it closes the
message: only the message list is left (plus the left pane, if the layout shows it). This replaces the requested
"swipe right closes the message" gesture, because MISC-5 already uses that swipe to go to the previous thread.

- **Commands:** none expected; closing goes through the presenter's existing view path (as `back` does now). If no
  `View` command clears the selected thread, add one (kind `View`, all agents).
- **Touches:** `app-mobile` (`frame.rs`: a `close` button on the message frame; `mobile.rs`: its event), `android/`
  (`ui/DailScreen.kt`), `ios/` (SwiftUI message view); contract frames and snapshots.
- **Open questions:** none.

### MISC-8 — Release: the ubuntu job fails on `main`

The `release` run of 2026-10-27 (run 40566169189, `workflow_dispatch` on `main`) fails in the `ubuntu` job after the
`.deb` is built: `desktop-file-validate command is missing, please install it` (appimagetool validates the
`.desktop` file). The run of 2026-10-25 (40160816747) failed too; 2026-10-20 passed.

- **Fix:** install `desktop-file-utils` in the ubuntu job (beside the other apt packages), or in the AppImage script's
  prerequisites check so a local build reports it too. Add it to the documented release prerequisites.
- **Done when:** a dry-run `release` dispatch on `main` is green in every job (read it with `gh run view`).
- **Open questions:** none.

### MISC-9 — Release: every release increments the app version

Every release so far is `0.1.0`. A release must carry a new version without hand edits.

- **Decision (default, change if the user objects):** a `prepare-release` `workflow_dispatch` with input
  `bump: patch | minor | major` (default `patch`). It reads the current version, runs
  `cargo xtask release bump <next>` (R5: every version site + CHANGELOG stub; Android `versionCode` must grow too),
  commits `workspace: release vX.Y.Z` to `main` and pushes the tag `vX.Y.Z`, which starts `release.yml` (R2). The
  guard job stays. A new `cargo xtask release next <patch|minor|major>` prints the next version (unit-tested).
- **Constraints:** the push needs a token that triggers workflows (a GitHub App token or deploy key in the `release`
  environment; `GITHUB_TOKEN` pushes do not start other workflows). Branch protection on `main` must allow it, or
  the job opens a PR and the tag is pushed on merge — check the repository settings when picked.
- **Done when:** a dispatch produces `v0.1.1` end to end, and the in-app update check (Wave 7) offers it to `0.1.0`.

### MISC-10 — Android: folder tree in the left pane

The desktop sidebar is a folder tree since UX-2 (with collapse, UX-3). The Android left pane is still a flat list.
Show the same tree: nested folders indented under their parent, a chevron to collapse/expand that sends
`collapse_folders` / `expand_folders` (persisted, UX-1).

- **Commands:** none new; reuse `app_front`'s tree projection in the `app-mobile` sidebar frame (rows get `depth`,
  `has_children`, `collapsed`).
- **Touches:** `app-mobile` (sidebar frame), `android/` (`ui/DailScreen.kt`), `ios/` (same frame, so the iPad drawer
  gets it too); contract frames and snapshots.
- **Open questions:** none.

### MISC-11 — Mobile: landscape left pane, icon rail or icon + text tree

MISC-6 made the landscape left pane an icon rail. The user must be able to switch it between *icons only* and
*icons + text* (the full tree, MISC-10). Android and iOS.

- **Commands:** a persisted view preference. Prefer a `View` command (agents `any`) such as `set_sidebar_style
  { style: "icons" | "full" }` stored in `[view]` (UX-1), so it survives a restart; decide when picked whether the
  desktop uses it too (it may ignore it).
- **UI:** a toggle button at the top or bottom of the rail (expand / collapse chevron) and an entry in the palette.
- **Touches:** `app-core` (if the command is added), `app-io` (`[view]`), `app-mobile`, `android/`, `ios/`.
- **Open questions:** none.

### MISC-12 — iOS: About cannot be closed

On iOS, when About is open there is no way to close it, and the other panes seem to open underneath it. MISC-3 added
a close button in the About top bar; on iOS it is missing or not reachable, and About stays on top while the
presenter has moved on.

- **Fix:** the SwiftUI About view must draw the frame's close action, and the view stack must follow the frame (About
  shown only while the frame says so). Add a `DailTests` UI test: open About, close it, the list is visible; open
  About, open a mailbox from the drawer, About is gone.
- **Touches:** `ios/` (About view, root navigation), `app-mobile` if the frame lacks the action.
- **Done when:** CI's `ios` job is green with the new test (read with `gh run view`).
- **Open questions:** none.

### MISC-13 — Mobile: About title and text

The mobile About pane shows: first line `× About dail (<version>)` (the close button, then the title with the running
version), second line a short description such as "Email client for IMAP and SMTP accounts." Drop "keyboard-driven,
agent-controllable": neither applies on a phone (no MCP, no keyboard-first use). The desktop text is unchanged.

- **Touches:** `app-mobile` (`theme.rs` about text, About frame title), `android/`, `ios/`; contract frame
  `about.json` and snapshots.
- **Open questions:** none.

### MISC-14 — Mobile: maximize the message pane

There must be an explicit way to maximize the message content pane (full screen: no sidebar, no list, minimal top
bar), and to restore it. Pinch (MISC-6) is not discoverable enough.

- **UI:** a maximize / restore button in the message top bar beside the close (×) of MISC-7; double-tap on the
  title bar may toggle it too.
- **Commands:** none expected; it is the largest pane step of MISC-6 in the presenter (persists across threads, as
  MISC-6 decided).
- **Touches:** `app-mobile`, `android/`, `ios/`; contract frames.
- **Open questions:** none.
