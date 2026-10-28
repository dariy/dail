# dail — Feature requests: miscellaneous

Small requests recorded with `/new-task`. MISC-1 to MISC-7 are scheduled in `ROADMAP.md` Wave 6, MISC-8 to MISC-16 in Wave 8, MISC-17 to MISC-24 in Wave 9. When an item is picked, move it to `ROADMAP.md`
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
| MISC-9 | Release: a merged PR releases a new, incremented version | M | .github/workflows/, xtask | 2026-10-27 |
| MISC-10 | Mobile: folder tree in the left pane (Android, iPad portrait drawer) | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-11 | Mobile: landscape left pane switches between icon rail and icon + text tree | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-12 | iOS: About cannot be closed and covers the other panes | S | ios/, app-mobile | 2026-10-27 |
| MISC-13 | Mobile: About title with the version, mobile description | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-14 | Mobile: maximize the message pane | S | app-mobile, android/, ios/ | 2026-10-27 |
| MISC-15 | CI: run only on PRs into `main` and on `main` | S | .github/workflows/ | 2026-10-27 |
| MISC-16 | Repository: protect `main` (PRs only, merged by admins) | S | repository settings, docs | 2026-10-27 |
| MISC-17 | Mobile: the open message is a sheet; a drag down closes it | M | app-mobile, android/, ios/ | 2026-10-28 |
| MISC-18 | Mobile: the message is a page; pulling past its ends turns the conversation | M | app-mobile, android/, ios/ | 2026-10-28 |
| MISC-19 | Mobile: JPG and PDF attachments shown in the app, with zoom | L | app-io, app-host, app-mobile, android/, ios/, docs/adr | 2026-10-28 |
| MISC-20 | Mobile: the message list lines up with the menu icon and the title | S | android/, ios/ | 2026-10-28 |
| MISC-21 | Mobile: Compose as an icon button | S | android/, ios/ | 2026-10-28 |
| MISC-22 | Palette: query results as views, not raw JSON | M | app-front, app-mobile, app-gui, app-tui, android/, ios/ | 2026-10-28 |
| MISC-23 | Mobile: folder tree alignment (one column per level, half-icon indent) | S | android/, ios/ | 2026-10-28 |
| MISC-24 | Mobile: settings content on the title's left edge | S | android/, ios/ | 2026-10-28 |

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
- **Replaced (2026-10-28):** MISC-18 turns the conversation with a pull past the top or the end of the body. On a
  phone's page a swipe right goes back.

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
- **Later (2026-10-28):** MISC-18 brings the swipe right back on phones: the message is a page there, and a swipe
  right closes it. The × stays.

### MISC-8 — Release: the ubuntu job fails on `main`

The `release` run of 2026-10-27 (run 40566169189, `workflow_dispatch` on `main`) fails in the `ubuntu` job after the
`.deb` is built: `desktop-file-validate command is missing, please install it` (appimagetool validates the
`.desktop` file). The run of 2026-10-25 (40160816747) failed too; 2026-10-20 passed.

- **Fix:** install `desktop-file-utils` in the ubuntu job (beside the other apt packages), or in the AppImage script's
  prerequisites check so a local build reports it too. Add it to the documented release prerequisites.
- **Done when:** a dry-run `release` dispatch on `main` is green in every job (read it with `gh run view`).
- **Open questions:** none.

### MISC-9 — Release: a merged PR releases a new, incremented version

Every release so far is `0.1.0`. Merging a PR into `main` must publish a release with a higher version, without hand
edits. Pushes to `main` other than merges cannot happen (MISC-16).

- **Decision:** a workflow on `push` to `main` (the merge commit) computes the next version, runs
  `cargo xtask release bump <next>` (R5: every version site + CHANGELOG stub; Android `versionCode` grows), commits
  `workspace: release vX.Y.Z [skip ci]`, pushes the commit and the tag `vX.Y.Z`; the tag starts `release.yml` (R2,
  guard job kept). The bump is `patch` by default; a PR label `release:minor` / `release:major` (or `release:skip`
  for docs-only merges) overrides it. A new `cargo xtask release next <patch|minor|major>` prints the next version
  (unit-tested). The bump job must not trigger itself (skip its own commits).
- **Constraints:** the bump commit and tag are pushed with a GitHub App token (or deploy key) held in the `release`
  environment: `GITHUB_TOKEN` pushes start no workflows, and the App must be on the bypass list of the `main`
  ruleset (MISC-16), as the only actor allowed to push to `main` directly. Concurrency group so two merges in a row
  produce two versions in order.
- **Done when:** merging a PR produces `v0.1.1` end to end, and the in-app update check (Wave 7) offers it to `0.1.0`.

### MISC-10 — Mobile: folder tree in the left pane

The desktop sidebar is a folder tree since UX-2 (with collapse, UX-3). The Android left pane is still a flat list.
On iPad in portrait the left pane opens as a detached (overlay) window; it must show the same tree.
Show the same tree: nested folders indented under their parent, a chevron to collapse/expand that sends
`collapse_folders` / `expand_folders` (persisted, UX-1).

- **Commands:** none new; reuse `app_front`'s tree projection in the `app-mobile` sidebar frame (rows get `depth`,
  `has_children`, `collapsed`).
- **Touches:** `app-mobile` (sidebar frame), `android/` (`ui/DailScreen.kt`), `ios/` (same frame, so the iPad drawer
  portrait overlay and the landscape pane get it too); contract frames and snapshots.
- **Done when:** the iPad portrait overlay shows the tree in a `DailTests` check on an iPad simulator (CI `ios` job).
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

### MISC-15 — CI: run only on PRs into `main` and on `main`

`ci.yml` has `pull_request:` without a branch filter, so it runs for PRs into any branch;
`gradle-verification.yml` likewise. Limit both to `pull_request: branches: [main]`; `ci.yml` keeps
`push: branches: [main]` and `workflow_dispatch`. Scheduled workflows (`advisories`, `toolchain`) and `release.yml`
(tags) are unchanged. Update AGENTS.md §6 / ARCHITECTURE.md where they describe the triggers.

- **Open questions:** none.

### MISC-16 — Repository: protect `main`

Nobody pushes to `main` directly; changes arrive by PR, and only admins / the owner can merge.

- **Decision:** a repository ruleset on `main` (`gh api repos/{owner}/{repo}/rulesets`): block deletion and force
  pushes, require a pull request, require the `ci` status checks (the job names of `ci.yml`), restrict who can
  update `main` with the bypass list = repository admins + the release App of MISC-9 (the App only for its bump
  commit). Store the ruleset JSON in `.github/rulesets/main.json` and apply it with a documented `gh api` command,
  so the setting is reviewable.
- **Order:** apply after MISC-9's App exists, or the release bump cannot push. Applying it is an outward-facing
  repository change: confirm with the user before running the `gh api` call.
- **Done when:** `git push origin main` from a non-bypass actor is rejected, and a PR merge by an admin works.
- **Open questions:** none.

### MISC-17 — Mobile: the open message is a sheet; a drag down closes it

On a phone (a compact window, one pane), opening a message swaps the list for the message and the top bar for
"← Message". Nothing moves: the message is simply there. When it closes, the list is built again and comes back
scrolled to the top. The open message must behave as a panel that rises from the bottom over the list, and a drag
down must close it.

- **Commands:** none (the panel closes with `close_thread`, as the × of MISC-7 does).
- **Decisions:**
  - Compact windows only. On medium and expanded windows the message stays a pane beside the list, and a maximized
    message (MISC-14) keeps its buttons.
  - Presenter: on a compact window with an open thread, the frame keeps the list pane (`layout.panes = [list]`) and
    carries the message as a sheet over it (a new optional `layout.sheet`, `reader`). The top bar is the list's. No
    floating button while the sheet is up. Back closes the sheet, as it closes the message now.
  - Hosts: a full-height sheet over the panes and the top bar (below the status bar), with a grab handle above the
    subject row. It slides up on open (about 250 ms). A drag down follows the finger: from the handle or the subject
    row at any time, from the body only when the body is scrolled to its top (the body's own scroll comes first).
    Released past a quarter of the height, or flung down, it closes (the × action) and slides down; otherwise it
    springs back. The list under the sheet keeps its scroll position and is hidden from TalkBack and VoiceOver.
  - With the system's "remove animations" or Reduce Motion, the sheet appears and goes without the slide.
  - The horizontal swipe of MISC-5 / MISC-18 and this vertical drag: the dominant axis decides.
- **Touches:** `app-mobile` (`frame.rs` `Layout`, `mobile.rs` `frame()`), `android/` (`ui/DailScreen.kt`,
  `ui/ReaderScreen.kt`), `ios/` (`Views/FrameView.swift`, `Views/ReaderPane.swift`); contract fixtures
  (`DAIL_UPDATE_CONTRACT=1`) and the `thread_360x800` snapshot.
- **Done when:** a presenter test: on a compact window an open thread gives panes `[list]`, sheet `reader`, the list's
  top bar and no floating button; medium and expanded frames are unchanged. On Android and iOS a drag down closes the
  message and the list is where it was.
- **Open questions:** none.
- **Superseded in part (2026-10-28):** MISC-18 replaces the sheet with a page that slides in from the right and
  goes back with a swipe right (`layout.page`). The list keeping its place, hiding it from TalkBack and VoiceOver,
  no floating button, and the motion rules carry over.

### MISC-18 — Mobile: the message is a page; pulling past its ends turns the conversation

MISC-5 made a horizontal swipe on the message go to the previous or next conversation. Today the swipe acts only
when the finger lifts: the next conversation then replaces the old one in place, with no motion. The change must
animate like turning a page.

Before it was built, the phone's navigation model was chosen (2026-10-28) between two:

1. **Panes in a row** (folders, list, message). The message slides in from the right, a swipe right goes back, and
   vertical swipes turn the conversation.
2. **The list as the base.** The message is a sheet from the bottom (MISC-17), a drag down closes it, and horizontal
   swipes turn the conversation.

The first won, amended.

- **Why.** No formal standard covers mail-client screens, but the platforms' guidelines do:
  - Apple's HIG keeps sheets for "a simple task" ("for complex or prolonged user flows, consider alternatives to
    sheets"). For a hierarchy it expects a Back button, with a swipe from the side as its shortcut.
  - Material 3's list-detail layout shows the detail in place of the list on a compact window, and Back returns to
    the list.
  - NN/g: "Do not use a bottom sheet to replace typical page-to-page user flows."

  No major client reads mail in a sheet: Gmail, Outlook, Apple Mail, Proton and Thunderbird open a page. Paging
  sideways fights the system's Back: K-9 users on Pixels hit the edge Back while swiping between messages, and
  iOS 26 goes back on a swipe right anywhere.

  `app_front::panes` already shows one pane of the same row on narrow windows (as in the TUI), and the desktop shows
  three. That is one spatial model at every width, so a resize (foldables, split screen, iPad windows) only changes
  how many panes show.

  Sources: [HIG: Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets),
  [HIG: Gestures](https://developer.apple.com/design/human-interface-guidelines/gestures),
  [Android: list-detail](https://developer.android.com/develop/ui/compose/layouts/adaptive/list-detail),
  [NN/g: Bottom sheets](https://www.nngroup.com/articles/bottom-sheet/),
  [K-9: swipe vs Back](https://forum.k9mail.app/t/new-message-swipe-conflicts-with-pixel-previous-screen-functionality/5907),
  [NetNewsWire: iOS 26 back swipe](https://discourse.netnewswire.com/t/ios-26-swipe-anywhere-to-go-back/92).
- **Commands:** none. The swipe right runs the reader's close (`close_thread`), and a pull runs the frame's previous
  or next action (`open_thread`).
- **Decisions:**
  - **Presenter.** On a compact window an open thread keeps `layout.panes = [list]` and carries the message as
    `layout.page = reader` (MISC-17's `sheet`, renamed). `Reader.previous` and `next` become
    `Neighbour { open, sender, subject, date }`:
    - `open` is the button: `open_thread`, and TalkBack's and VoiceOver's custom action.
    - `sender`, `subject` and `date` are the neighbour's peek, built by the same helper as the list rows, so they
      are cleaned as the rows are.
    - A neighbour is absent at the ends of the list.
  - **The page.** It slides in from the right over the list and the top bar (250 ms).
    - A horizontal drag anywhere follows the finger. Past a third of the width, or flung right, the page slides out
      and closes the message. Otherwise it springs back.
    - A block's own horizontal scroll goes first. The dominant axis decides against the body's scroll.
    - A frame that drops the page over the list (the ×, Back, a thread that goes away) slides it out too.
    - The list keeps its scroll position and is hidden from TalkBack and VoiceOver. VoiceOver's escape gesture
      closes the page.
    - The snackbar, a modal drawer, the link dialog and what takes the panes' place (About, the setup, the palette,
      a result, a preview) show over the page, which waits under them in place. (Under MISC-17's sheet they were
      hidden.)
  - **Turning.**
    - At the bottom of the body, pulling further up shows the next conversation's peek. Only a drag counts, never a
      fling's momentum. Past about 80 dp/pt the peek says "Release to open".
    - Released there, the page slides up and out, the peek slides in, and the full conversation replaces it when
      its frame arrives.
    - At the top, a pull down does the same for the previous conversation.
    - Short of the threshold the page springs back. With no neighbour, the system's own overscroll shows.
  - **No swipe-left action pane for now.** Android's right-edge Back gesture is there, and the actions stay visible
    under the subject. The ˄˅ buttons stay undrawn (MISC-5); `Neighbour.open` keeps them one host change away.
  - **Medium and expanded windows** keep the message as a pane beside the list. The pull works there, and a
    horizontal swipe does nothing.
  - **Reduce Motion / animations off.** The page appears and goes without the slide, and the turn is a cross-fade.
- **Touches:**
  - `app-mobile`: `frame.rs` (`Layout`, `Reader`, `Neighbour`), `mobile.rs` (the row text), `mobile/reader.rs`
    (`neighbours`).
  - `android/`: `ui/ReaderPage.kt`, `ui/ReaderScreen.kt`, `ui/DailScreen.kt`, `ui/Frame.kt`.
  - `ios/`: `Views/ReaderPane.swift`, `Views/FrameView.swift`, `Frame.swift`.
  - The contract fixtures and the `thread_360x800` snapshot.
- **Done when:**
  - Presenter tests: a compact window gives panes `[list]` and page `reader`; the peeks equal the list rows and are
    absent at the ends; `tests/it/untrusted.rs` walks a poisoned peek.
  - On both apps: the page slides in, a swipe right goes back to the list where it was, a short drag springs back,
    and a pull past either end turns the conversation.
- **Open questions:** none.

### MISC-19 — Mobile: JPG and PDF attachments shown in the app, with zoom

JPG and PDF attachments must open inside dail and zoom in and out.

Today Android shows an attached image fitted to the screen, without zoom. An image larger than 4096 px on a side is
refused ("Loading or not a picture dail can show."), and that is many photos from current phones (12 MP, 4032 ×
3024, still fits; 24 MP and more does not). EXIF orientation is ignored, so a portrait photo can show sideways. A PDF
shows "dail does not show this kind of file". iOS draws every attachment as a placeholder (name, type and size):
Swift never calls `dail_blob`.

- **Commands:** none.
- **Decisions:**
  - Images (`app_io::images`, shared with inline pictures): an image larger than `MAX_SIDE` is decoded downscaled to
    fit, not refused, and the decoder stays within `MAX_ALLOC` (for JPEG a DCT-scaled decode, such as
    `jpeg-decoder`'s `scale`; choose when picked, pure Rust). EXIF orientation is applied (`image`'s
    `ImageDecoder::orientation`).
  - PDF in pure Rust with `hayro` (`#![forbid(unsafe_code)]`), not Android's `PdfRenderer` (PDFium, C++) or iOS's
    PDFKit: an attacker's PDF is parsed only by memory-safe code, as images and HTML already are, by one renderer for
    both apps that `cargo test` covers. A new `app_io::pdf` beside `images`: the page count and sizes, and a page
    rasterized at a given width to RGBA (the images' format). Limits on pages, pixels per page (`MAX_SIDE`), memory
    and time. Rendering runs on a blocking task, so an error or a panic shows "This page could not be shown." with
    *Save*. No links, forms, JavaScript or remote loads.
  - `app-host`: `BlobRequest` gains a PDF's pages and a PDF page at a width. `app-mobile`: `PreviewKind::Pdf`.
  - Zoom in both apps: pinch, double tap (fit ↔ 2.5×), pan while zoomed. PDF pages scroll vertically, with
    "page n of m". Past the bitmap's resolution the host asks for the visible page again at the zoomed width (at most
    `MAX_SIDE`). *Save* stays the only way to another app.
  - iOS: `Native.swift` and `Host.swift` call `dail_blob`. The preview draws images, text and PDF pages with the same
    zoom, in place of the placeholder.
  - ADR 0012 comes first: `hayro` against the system renderers (safety, quality, speed, app size), the limits, and
    the licences of any fonts `hayro` embeds (they go into About). `cargo deny` must pass the new tree.
- **Touches:** `app-io` (`images.rs`, new `pdf.rs`), `app-host` (`config.rs` `BlobRequest`, `host.rs` `blob`),
  `app-mobile` (`mobile/reader.rs` `preview_kind`, `frame.rs`, `theme.rs` licences), `android/`
  (`ui/PreviewScreen.kt`), `ios/` (`Native.swift`, `Host.swift`, a preview view), workspace dependencies,
  `deny.toml` if needed, `docs/adr/0012-*.md`.
- **Done when:** `app-io` tests: a generated 6000 × 4000 JPEG decodes to at most 4096 px a side within `MAX_ALLOC`; a
  JPEG with EXIF orientation 6 comes out upright; a generated one-page PDF renders at the asked width with ink on
  it; a truncated or malformed PDF is an error, not a panic or a hang. A contract fixture for a PDF preview frame.
  Both apps open a camera JPG and a PDF and zoom them with pinch and double tap.
- **Open questions:** none (the renderer was decided on 2026-10-28: pure Rust).

### MISC-20 — Mobile: the message list lines up with the menu icon and the title

The message list must line up with the top bar: with the menu (burger) icon or with the "Inbox" title.

Today on Android a row starts 12 dp in, then the account's colour bar (7 dp) and a 20 dp gutter for the unread dot,
so the sender starts at about 39 dp. The top bar's menu icon is at 16–40 dp and its title at about 56 dp (Material's
top app bar), so the text lines up with neither. On iOS the rows use the system list insets, and the text starts at
about 32–36 pt. The title's position depends on the width of the navigation button's symbol.

- **Commands:** none.
- **Decisions:**
  - Two keylines, the same in the top bar and in the panes: the icon column (the menu icon, 16 to 40 dp/pt from the
    edge) and the text keyline (the title's start, 56 dp/pt). In the list, the unread dot or the check box is centred
    in the icon column. Sender, subject and snippet start on the text keyline. The account's colour bar sits at the
    very edge. The server line, the empty state and the placeholders follow the keylines too.
  - iOS's top bar puts the navigation button in a fixed 24 pt slot at 16 pt, so its title starts at 56 pt too.
  - The keylines are constants in one place per app (`Keylines.kt`, `Metrics.swift`), described in ARCHITECTURE.md's
    mobile section. MISC-23 and MISC-24 use them. In multi-pane windows they count from the pane's edge.
- **Touches:** `android/` (`ui/ListScreen.kt`, a new `ui/Keylines.kt`), `ios/` (`Views/FrameView.swift` `TopBarView`,
  `Views/ListPane.swift`, a new `Views/Metrics.swift`).
- **Done when:** on a 360 × 800 emulator and an iPhone simulator, the first letter of the first sender is under the
  first letter of "Inbox" (±1 dp/pt) and the unread dot is under the menu icon's centre (screenshots).
- **Open questions:** none.

### MISC-21 — Mobile: Compose as an icon button

The floating button says "Compose" in text: an extended button on Android, a capsule with the icon and the word on
iOS. It must be an icon only.

- **Commands:** none; the frame already carries the `edit` icon.
- **Decisions:** a round button with the pen icon. The accessible name stays "Compose" (TalkBack, VoiceOver), and
  Android shows it as a tooltip on long press.
- **Touches:** `android/` (`ui/DailScreen.kt`), `ios/` (`Views/FrameView.swift`).
- **Done when:** both apps draw the icon alone, and TalkBack and VoiceOver read "Compose".
- **Open questions:** none.

### MISC-22 — Palette: query results as views, not raw JSON

A query chosen in the palette (*Show message*, *Show thread*, *List threads*, *Search mail*, *List mailboxes*, *List
contacts*, *Show pending changes*, *Show send requests*, *Show settings*, *Show view state*, *Test account
connection*, *Find the mail servers of an address*) shows its receipt as pretty JSON, read-only. That is true on every
front end: mobile (`app-mobile` `mobile/sheet.rs`), the GUI (`shell.rs` `show_result`) and the TUI (`palette.rs`).
Each must show as a view people read, with taps that go somewhere. Asked for mobile; the desktop is in scope too
(decided on 2026-10-28), so no front end shows raw JSON unless the user asks for it.

- **Commands:** none; taps run existing `View` commands.
- **Decisions:**
  - A new `app_front::result` (toolkit-free, shared by the GUI, the TUI and mobile, ADR 0004) builds a `ResultDoc`
    from the command's name, the receipt's data and the state: sections of rows (title, detail, meta, marks, an
    optional command a tap runs) and labelled fields. An exhaustive `match` over the query commands (no `_ =>`), so a
    new query does not compile without its view. Data the view does not know falls back to labelled fields, never
    braces.
  - *Show message*: From, To, Cc, Date, Subject, mailbox, size, flags, Message-ID and attachments, then *Open
    conversation* (`open_thread`).
  - *Show thread*, *List threads*, *Search mail*: rows drawn as the list draws them (senders, subject, date, unread,
    star, pin, clip). A tap opens the conversation (`open_thread`).
  - *List mailboxes*: the folder tree with unread and total counts and the sync window. A tap opens the mailbox
    (`open_mailbox`).
  - *List contacts*, *Show pending changes*, *Show send requests*: rows (name and address, counts, last date; what,
    where, attempts, last error; the request).
  - *Show settings*, *Show view state*: grouped labelled fields.
  - *Test account connection*, *Find the mail servers*: a checklist (IMAP and SMTP, each ✓ or ✗ with the server's
    answer), and the servers found.
  - *Raw data* keeps the JSON one tap away, for whoever wants what an agent sees.
  - Untrusted content: every string goes through `app_front::text::clean`. Nothing is interpreted, links are not
    live, and nothing remote loads. The note still says that email content is untrusted.
  - Mobile: `ResultView` gains sections, fields and the *Raw data* toggle (presenter state), with `lines` only while
    raw. Kotlin and Swift draw the rows in the list's style. The GUI's result window and the TUI's result view draw
    the same document.
- **Touches:** `app-front` (new `result.rs`), `app-mobile` (`mobile/sheet.rs`, `frame.rs`), `android/` (the result
  screen in `ui/DailScreen.kt`), `ios/` (`ResultPane` in `Views/FrameView.swift`), `app-gui` (`shell.rs`), `app-tui`
  (`palette.rs`); `insta` snapshots, contract fixtures.
- **Done when:** an `insta` snapshot per query command over the demo state in `app-front`. *Show message* and *List
  threads* from the palette show views on Android, iOS, the GUI and the TUI, with no JSON unless *Raw data*. An
  untrusted-content test covers the document (control and bidi characters are cleaned). The parity walks still pass.
- **Open questions:** none.

### MISC-23 — Mobile: folder tree alignment

In the drawer's folder tree (MISC-10), a folder with subfolders has a chevron before its icon. A folder without
subfolders has none, so its icon starts where a parent's chevron is.

1. **One level, one column.** When leaves and parents are on the same level, their titles must line up. Today a
   leaf's title starts a chevron's width (Android 32 dp, iOS 24 pt) left of a parent's: leaves start under the `>`.
2. **Half an icon per level.** A child folder's icon must sit half an icon right of its parent's icon. Today each
   level indents 16 dp/pt from the row's start, so a child without subfolders sits left of its parent's icon
   (Android 16 dp, iOS 8 pt) and a child with subfolders 16 dp/pt right of it.

- **Commands:** none; the frame already carries `depth` and `fold` (MISC-10).
- **Decisions:** every folder row reserves the chevron's column when the account has a folder with subfolders, as the
  desktop sidebar does (`app-gui/src/sidebar.rs`, `gutter`). Each level indents half an icon (12 dp/pt for the
  24 dp/pt icon). *All inboxes* and the footer (*Settings*, *About*, *Add account*) keep their icons in the top
  level's icon column. The chevron keeps its touch target. The account row and the icon rail are unchanged.
- **Touches:** `android/` (`ui/DrawerScreen.kt`), `ios/` (`Views/DrawerPane.swift`).
- **Done when:** in the demo app with `Work`, `Work/Clients` and `Lists/rust` created (palette *Create folder*), on
  both platforms (screenshots): the titles of one level start on one line, leaves and parents alike, and each child's
  icon starts half an icon right of its parent's.
- **Open questions:** none.

### MISC-24 — Mobile: settings content on the title's left edge

The settings page's content must line up with its title, "Settings". Today on Android the section tabs start at
Material's 52 dp edge (their labels at about 68 dp), and the captions and rows at 16 dp, while the title starts at
about 56 dp after the back arrow. On iOS the grouped list's insets put the rows at about 32–40 pt.

- **Commands:** none.
- **Decisions:** the first tab's label, the captions, the rows, their hints, and the chips and buttons that wrap below
  them all start on the text keyline (MISC-20). On a one-pane window that is the title's start; beside the drawer,
  the same distance from the pane's edge.
- **Touches:** `android/` (`ui/SettingsScreen.kt`), `ios/` (`SettingsPane` in `Views/SetupView.swift`).
- **Done when:** on a phone (screenshots, both platforms) the first letters of the first tab, of each caption and of
  each row are under the "S" of "Settings" (±1 dp/pt).
- **Open questions:** none.
