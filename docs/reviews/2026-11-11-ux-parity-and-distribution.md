# dail — UX parity and distribution review

Reviewed on 2026-11-11 at commit `8da4e6c` (branch `go/misc`, 22 commits ahead of `main`, latest tag `v0.1.7`). The
question: what stands between dail and a smooth, consistent experience on every front end, and between dail and Google
Play, the App Store, winget, F-Droid, apt and Homebrew? This file is a task source for `ROADMAP.md`; it changes no
roadmap item itself.

**How it was checked.** The code and docs were read. No gate was run, no device or emulator was used, and no screen was
looked at. Every finding names its code anchor. Findings marked *not seen* are conclusions from the code that a device
check must confirm. Store rules for Play and F-Droid come from `docs/reviews/2026-11-02-store-readiness.md`. The rules of
the App Store, winget, Homebrew, apt and Flathub are from prior knowledge and were **not** checked for this review;
each must be read again before its task starts. Section 8 lists what this review did not establish.

**Task format.** `ID (size) [touches]`, with sizes S, M and L as in `docs/requests/misc.md` and the tags of the roadmap
legend. Priorities: **P0** blocks daily use on a phone or a store listing, **P1** should be in 1.0, **P2** can follow.
The prefixes `PAR`, `TCH`, `DSK`, `IOS`, `DIST`, `REL` and `QA` are new and collide with no existing ID.

## 1. Verdict

At the command level dail is already consistent: 93 commands, one registry, one engine, and a palette built from the
registry on every front end. The inconsistency is one level up, at the controls. It has four causes.

1. **Each front end picks its own buttons.** Which commands get a button, a menu entry or a gesture is decided by hand
   in `app-gui`, `app-tui` and `app-mobile`. Only message cards share one list (`app_front::message_actions`, MISC-44).
   Thread, bulk, folder and account actions have no shared list, so they drifted (section 3).
2. **On touch, the palette is the fallback, and the fallback is weak.** Whatever has no button on a phone is "in the
   palette". But the palette cannot be opened from two common states (section 2), and many of its steps ask for typed
   text where a phone needs a picker (section 4, PAR-3).
3. **Finished features do not reach the devices.** All of Wave 12 (settings sync, rules) and Wave 13 is on `go/misc`
   and in no release (section 2).
4. **Distribution stops at GitHub Releases.** There is no winget manifest, no apt repository, no Homebrew formula and
   no AppStream metadata. The iOS app cannot write mail, so it cannot pass App Review.

No architectural change is needed. The frame contract, the presenter and the registry are the right places for every
fix below, and iOS inherits most touch fixes through the presenter.

## 2. The three reported problems, traced

**"The rules engine exists, but I cannot use it on Android."** Three separate causes, the first decisive:

- *Unreleased.* The Rules screen is commit `26e4410` (SET-5, 2026-11-06). It is not an ancestor of `main`
  (`git merge-base --is-ancestor 26e4410 main` fails), and releases are cut from `main`. No published APK has it. → REL-1.
- *Hard to find.* On a phone, settings are five tabs in a scrolling row and *Rules* is the fifth
  (`crates/app-mobile/src/mobile/settings.rs:20`, `SettingsScreen.kt:36`); it starts off screen on a narrow phone
  (*not seen*). *Make a rule from this message* exists only as a palette row (`palette.rs:1747`), and the palette cannot
  be opened while a message is open (below). → TCH-6, TCH-8.
- *Hard to finish.* A rule is built by chained palette prompts, one condition and one action (`palette.rs:828`). A rule
  with several conditions or actions can be made only as JSON; `editable_here` (`palette.rs:939`) then hides its
  condition and action from the editor. The settings account cannot be chosen on mobile except through the palette, and
  the phone shows no pull state (`set_settings_account` and `pull_settings` have no control in `app-mobile`). → TCH-7,
  TCH-9.

**"I can select several messages but cannot move them."** The phone's bulk bar is a fixed list: archive, trash, mark
read, mark unread, star, pin (`mobile.rs:1505`). *Move…* is offered only for the open conversation
(`mobile/reader.rs:345`). The bulk bar also replaces the whole top bar, including the palette button
(`DailScreen.kt:118`), so the palette's *Move to…*, which would act on the checked threads (`palette::targets`), cannot
be reached by touch. Junk, Not junk and Select all are missing in the same way. The desktop bulk bar has no *Move…*
button either (`app-gui/src/list.rs:344`); there the `v` key, the row's context menu and a drag to a folder do it.
→ TCH-1, TCH-3, PAR-1.

**"I cannot use the command palette on Android when a message is open."** The only touch entry to the palette is the
search button of the top bar (`mobile.rs:1180`). On a compact window the open message is a page drawn over the panes
*and the top bar* (`frame.rs:134`, `ReaderPage.kt`), and the reader's own actions do not include the palette
(`mobile/reader.rs:323`). A maximized reader clears the top bar's actions too (`mobile.rs:991`). A hardware keyboard
still opens it (Ctrl+K). This breaks the Android rule of AGENTS.md §1: release builds have the palette only, so the
palette must be reachable from everywhere. → TCH-1.

## 3. Where each action has a control

*button* = a visible control, *menu* = a context or long-press menu, *key* = a default key, *palette* = only through
the palette, *—* = not possible. The mobile column is the presenter, so it holds for Android and iOS. The TUI is
keyboard-driven, so it was checked only for settings screens, not row by row.

| Action | Desktop GUI | Mobile (touch) |
|---|---|---|
| Open the palette | key, top bar | button, but not from an open message on a phone, the bulk bar or a maximized reader |
| Move checked threads | key `v`, row menu, drag to a folder; no bulk-bar button | palette, unreachable while threads are checked |
| Junk / Not junk | bulk bar, row, key `!` | palette |
| Select all | checkbox, key | palette, unreachable while threads are checked |
| Filter the list (unread, starred, attachments) | chips | palette |
| Empty Trash / Empty Junk | list button | palette |
| Mark all read | list and folder menu | palette |
| Folder: new, rename, delete, pin, hide, focus | folder context menu | palette, with typed names |
| Rules: list, on/off, delete | settings | settings (tab 5) |
| Rules: add and edit | palette steps, simple rules only | palette steps, simple rules only |
| Make a rule from this message | palette | palette, unreachable from the open message on a phone |
| Choose the settings account | settings | palette |
| Pull settings now | palette (TUI: settings) | palette |
| Edit an account's servers | — (the palette edits the display name only, `palette.rs:1094`) | — |
| Sending addresses and signatures | palette, typed | palette, typed |
| How much mail to keep offline | palette, typed number | palette, typed number |
| Redo | key | palette |
| Approve an agent's send | panel | — (by design, ADR 0009) |
| Write mail | composer | Android: composer. iOS: a placeholder (`FrameView.swift:90`) |

## 4. Tasks: one action model for every front end

These remove cause 1. They come first because the touch tasks of section 5 consume them.

- **PAR-1 (M) [front][gui][tui][mobile] P0.** `app_front::actions`, next to `message_actions`: `thread_actions(state,
  ids)` for one or many threads (archive, trash, junk or not junk, read or unread by state, star or unstar by state, pin
  or unpin, *Move…*, *Make a rule…* for one thread), `mailbox_actions(state, id)` (new subfolder, rename, delete, pin,
  hide, focus, mark all read, empty for Trash and Junk, apply rules, offline window) and `account_actions(state, id)`
  (test, password, servers, addresses, remove). Each entry is a label key, an icon name, a `Command` or a palette `Flow`,
  and a `primary` flag. The GUI's bulk bar and context menus, the TUI and the mobile presenter draw from these lists and
  keep no list of their own. *Done when* the desktop bulk bar and the phone's bulk bar show the same actions from one
  function, and a unit test pins each list.
- **PAR-2 (S) [front][gui][tui][mobile] P0.** A reach table, pinned by a test as `registry.rs` pins agent access: for
  every `CommandName` and every front end, one of `Control`, `Flow` (a control that opens a palette step), `Palette` or
  `Never`, from an exhaustive `match`. A new command then cannot compile without a decision per front end. The test
  enforces two rules: on touch no `Mutation` of category *Mail* is `Palette`, and nothing is `Never` unless an ADR says
  so. This is the durable fix for "exists on desktop, missing on the phone".
- **PAR-3 (M) [front] P1.** Palette steps never ask for typed text where the answers are a closed set. Today typed:
  `set_key_preset` ("Gmail or Vim", `palette.rs:1032`), `set_theme`, `remove_identity` (an address the account already
  has), `set_draft_from`, `set_mailbox_identity`, `set_sync_window` (a number), `set_update_policy`. Each becomes
  `AskKind::Options` or a list of the existing values, with the offline window as presets (200, 500, 1000, 5000, all).
  A test walks every flow and fails on an `AskKind::Text` whose command takes an enum or an existing entity.
- **PAR-4 (S) [front][mobile] P1.** Flows that fit the device. On mobile the shared flows of `save_attachments` and
  `attach_file` ask for an absolute path (`palette.rs:1336`); they must lead to `External::SaveFiles` and
  `External::PickFiles` instead. The mobile parity walk fails on any path prompt.
- **PAR-5 (M) [front][gui][tui][mobile][kt] P1.** Edit an account's servers. `update_account` takes `imap` and `smtp`,
  but no front end sends them: the only way to correct a host or port today is the CLI, or to remove the account and add
  it again. Reuse the wizard's server step as an *Edit servers* form, then run `test_account`.
- **PAR-6 (M) [front][gui][tui][mobile][kt] P1.** A form for sending addresses: name, address and a multi-line
  signature. The palette flow sets `name: None` and takes the signature on one line (`palette.rs:1191`). A folder's
  sending address becomes a picker of the account's addresses.
- **PAR-7 (L) [front][gui][tui][mobile][kt] P1.** A rule editor as a form, with its model in `app-front` (a `RuleDraft`:
  name, *all* or *any*, a list of conditions, a list of actions; validation from `app-core`). It replaces the chained
  prompts for add and edit on every front end, and it opens compound rules, which today can be changed only as JSON.
  Before *Save* it shows a dry run: "matches N conversations in this mailbox".
- **PAR-8 (S) [front][gui][tui][mobile] P2.** Redo and the undo history as visible entries (an *Edit* group in the
  palette's first rows and in the phone's overflow menu), since on touch the 6-second snackbar is the only undo.

## 5. Tasks: touch (Android now, iOS through the presenter)

Each task changes the presenter and the frame contract first, then Kotlin. Following Wave 11's decision, `ios/` takes
the contract change and stays green; its drawing is IOS-6.

- **TCH-1 (S) [mobile][kt][ios] P0.** The palette opens from every touch state: the open message on a phone, the bulk
  bar, the maximized reader, the composer and settings. Put it in the overflow of TCH-2. A presenter test builds a frame
  for each state and asserts that some button runs `open_palette`; the golden-path test opens the palette from an open
  message.
- **TCH-2 (M) [mobile][kt][ios] P0.** Overflow menus in the frame contract. `Reader.actions`, `Bulk.actions` and
  `TopBar.actions` are flat lists that Kotlin draws as a row of icons (`DailScreen.kt:122`, `ReaderScreen.kt:296`). The
  open message has six icons plus close and maximize; the bulk bar has six plus close and its title, on a 360 dp phone
  (*not seen*: likely cramped or clipped). Split each list by PAR-1's `primary` flag: at most four icons on a compact
  window, the rest under a ⋮ menu with text labels. The presenter decides the split by window class; Kotlin draws it.
- **TCH-3 (S) [mobile][kt][ios] P0.** The bulk bar from `thread_actions`: adds *Move…*, Junk or Not junk and *Select
  all*, shows one read toggle and one star toggle by the state of the checked threads (today both *read* and *unread*
  show, and there is no *Remove star*), and keeps the palette within reach. *Done when* the golden-path test checks two
  threads and moves them to a folder.
- **TCH-4 (M) [mobile][kt][ios] P0.** A folder's actions in the drawer: a long press on a folder (`DrawerItem` gets
  `long_press`) opens a sheet of `mailbox_actions`. New names are typed in a text step; everything else is a tap.
- **TCH-5 (S) [mobile][kt][ios] P1.** The list header: the filter chips of the desktop (unread, starred, attachments),
  and in the list's overflow *Mark all read*, *Empty Trash* or *Empty Junk* in those folders, and *Select all*.
- **TCH-6 (M) [mobile][kt][ios] P0.** Settings as a list of sections on a compact window, not five scrolling tabs:
  *Accounts*, *Rules*, *Appearance*, *General*, *Notifications & background*, *Keyboard*, *About*. *Keyboard* shows only
  while a hardware keyboard is attached (`configure`'s `keyboard`) and is otherwise the last, folded entry. Medium and
  expanded windows keep tabs or a two-pane layout.
- **TCH-7 (S) [mobile][kt][ios] P0.** Settings sync on the phone: the settings account as a row with a picker, *Pull
  now*, when settings were last pulled and stored, and the last error with its ERR code.
- **TCH-8 (S) [front][mobile][kt][ios] P0.** *Make a rule from this message* in the message card's menu and in the
  reader's overflow, through `message_actions` and `thread_actions`, so the desktop cards get it too.
- **TCH-9 (M) [mobile][kt][ios] P1.** Draw PAR-7's rule editor: a full-screen form with add and remove rows for
  conditions and actions.
- **TCH-10 (M) [mobile][kt][ios] P2.** Search apart from the palette. The top bar's magnifier is labelled *Search or
  run* and opens the command list. Give search its own field with recent searches and the filter chips, and keep the
  palette as its own entry. Decide with the user first: one entry is simpler, two are what phone users expect.
- **TCH-11 (S) [core][mobile][kt][ios] P2.** The two list swipes as settings (archive, trash, read or unread, move, none);
  today they are fixed to archive and trash (`frame.rs:294`).
- **TCH-12 (S) [kt] P2.** App shortcuts (Compose, Search) and the unread widget: the open stretch items of Wave 4.

## 6. Tasks: desktop and iOS

**Desktop**

- **DSK-1 (S) [gui] P0.** *Move…* in the bulk bar (free with PAR-1). Check that a row's context menu acts on every
  checked thread when the clicked row is checked (*not checked in this review*).
- **DSK-2 (M) [cli][gui][packaging] P1.** dail as the default mail app. Nothing accepts a `mailto:` argument on the
  desktop, `packaging/linux/dail.desktop` declares no `MimeType`, and the MSI registers no capability.
  `app_front::mailto` already parses the links for Android's share. Add `dail gui <mailto-url>` (through the IPC attach
  when an instance runs), `MimeType=x-scheme-handler/mailto;` with `%u`, and the Windows registration.
- **DSK-3 (S) [packaging] P1.** `dail` on `PATH` after the MSI. `packaging/windows/dail.wxs` installs the exe and a
  Start-menu shortcut only, so `dail mcp`, `dail exec` and `dail tui` are not found in a terminal or by an agent's MCP
  configuration. winget users expect the command.
- **DSK-4 (M) [user][ci][packaging] P1.** Sign the Windows exe and MSI. Unsigned installers meet SmartScreen warnings,
  and winget's validation scans them. Needs a certificate or a signing service, which is the user's choice and cost.
- **DSK-5 (M) [ci][packaging] P2.** arm64 builds for Linux (`.deb`, AppImage, tarball) and Windows. Every desktop
  artifact is x86_64 today.
- **DSK-6 [gui][tui] P1.** Draw PAR-5, PAR-6 and PAR-7 (their sizes are counted there).

**iOS** (after the Android launch, by Wave 11's decision; listed so the App Store goal has a complete list)

- **IOS-1 (L) [ios] P0.** The composer. The app shows a placeholder for a draft (`FrameView.swift:90`), so a user cannot
  write, reply or forward. Includes `pick_files`, `import_files` and the share sheet. App Review rejects an incomplete
  mail client.
- **IOS-2 (M) [ios][host] P0.** New-mail notifications and background refresh. Without a push server, iOS gives only
  background app refresh, at times the system chooses; the listing must say so plainly.
- **IOS-3 [user][ci] P0.** A paid Apple Developer account, signing in `release.yml` and TestFlight: the open optional
  item of Wave 5 (`ROADMAP.md:594`) becomes required.
- **IOS-4 (S) [ios][docs] P0.** `docs/store/app-store.md`, as `play-console.md`: App Privacy answers, the privacy
  manifest (`PrivacyInfo.xcprivacy`) with the required-reason APIs the Rust code touches, the export-compliance answer
  (dail ships its own TLS code from RustCrypto crates, not the system's), and a reviewer mailbox (STORE-22's).
- **IOS-5 (M) [ios][xtask] P0.** `fastlane/metadata/ios` and iPhone and iPad screenshots from `DailDemo`.
- **IOS-6 (M) [ios] P0.** Draw the contract changes of section 5 (overflow menus, folder sheet, section list, rule
  editor) and whatever `FrameView.swift:5` still lists as a title-only sheet.
- **IOS-7 [user] P0.** The device check, I4a to I4c, which is still open.

There is still no Mac: every iOS task is built and debugged through CI runs, so sizes are less certain than elsewhere.

## 7. Tasks: release, distribution and quality

**Release**

- **REL-1 (S) [user] P0.** Merge `go/misc` into `main` now, and from then on at least once per finished wave stage. Its
  22 commits hold all of Waves 12 and 13. A feature reported "done" in the roadmap should be on the user's phone within
  one merge.
- **REL-2 (S) [docs] P1.** Each roadmap item that adds a control says where it appears on each front end, or cites
  PAR-2's table. "Every front end" in SET-5 hid the fact that the phone's entry was a fifth tab.

**Distribution.** Order matters: DIST-1 and DIST-2 come before any package manager.

- **DIST-1 (M) [core][sync][cli] P0 for desktop stores.** An install channel. `InstallKind` (`app-core/src/update.rs:63`)
  knows AppImage, plain binary, `.deb`, MSI, Android and iOS. A copy installed by apt from a repository, by Homebrew or
  by Flatpak must not update itself: the `Binary` kind replaces the file in place, which corrupts a Homebrew or Flatpak
  install. Add a channel fixed at build time (an environment variable read by `build.rs`) that turns the updater into
  *notify only* or off, as STORE-15 did for Android, and amend ADR 0011. An MSI installed through winget can keep
  updating itself, since winget then sees the newer version.
- **DIST-2 (M) [packaging][xtask] P0 for Linux stores.** AppStream metadata `net.darii.dail.metainfo.xml` (summary,
  description, releases from `CHANGELOG.md`, screenshots, content rating), the desktop file under the same ID, and
  hicolor PNG icons beside the SVG. GNOME Software, KDE Discover and Flathub read it; `appstreamcli validate` joins
  the release script.
- **DIST-3 (S) [packaging][ci] P1.** winget: a manifest for the MSI (identifier, silent install, machine scope, SHA-256
  from the release), submitted to `microsoft/winget-pkgs`, and a release step that opens the update PR. After DSK-3;
  better after DSK-4.
- **DIST-4 (M) [packaging][ci] P1.** An apt repository: a signed repository built in `release.yml` from the `.deb`s and
  published on static hosting, with a `dail.sources` file and its keyring, and install lines in the README. A Launchpad
  PPA or Debian itself needs an offline source build of every crate, including the git dependency of ADR 0012, and is
  not worth it before 1.0. Uses DIST-1's channel.
- **DIST-5 (M) [packaging][ci] P1.** Homebrew: the user's own tap with a formula that builds the tagged source
  (`cargo install --locked --path crates/app-cli`), Linux only until a macOS build exists (roadmap, *Platforms*).
  `homebrew-core` asks for notability and a macOS build; treat it as later. Uses DIST-1's channel.
- **DIST-6 (L) [packaging][ci] P2.** Flathub, the store most Linux desktops show. Needs DIST-2, an offline build
  (vendored crates, the git dependency pinned), the Secret Service for the keyring from inside the sandbox, and the
  notification portal. Decide with the user whether it replaces the AppImage as the recommended download.
- **DIST-7 (S) [xtask] P1.** One source for store text. `fastlane/metadata/android` holds the title and descriptions in
  English and French. An `xtask` check keeps the `.deb` description, the metainfo, the winget manifest, the Homebrew
  `desc` and the iOS metadata in step with it, as `release versions` does for the version.
- **DIST-8 [user] P0 for Play.** Sequence only: do the P0 tasks of sections 4 and 5 before STORE-18's screenshots and
  STORE-24's closed test. Testers would otherwise report the gaps of section 2 for 14 days, and the store screenshots
  would show a UI that is about to change.

**Quality**

- **QA-1 (M) [kt] P0.** The golden-path test (STORE-9b) grows three journeys: check two threads and move them; open a
  message, open the palette, make a rule from it; open settings, find *Rules*, add a rule and switch it off.
- **QA-2 (S) [mobile] P0.** Frame budgets as presenter tests at 360×640 dp: at most four primary icons per bar, a
  palette entry in every state (TCH-1), and no settings section more than one tap from the settings root.
- **QA-3 (M) [kt][mobile] P1.** Measure smoothness instead of assuming it: the time to build, serialize and decode a
  frame per event over the `large` fixture on a mid-range phone, and dropped frames while scrolling the list and sliding
  the reader page. Budget 16 ms per frame; results into ARCHITECTURE §7.5 with STORE-23's.
- **QA-4 (S) [kt][ios] P1.** Accessibility of the new controls: 48 dp touch targets, TalkBack and VoiceOver names for
  every overflow entry, and the bars at font scales 1.3 and 2.0.
- **QA-5 [user] P0.** STORE-23's device check gets a UX section: the three journeys of QA-1 by hand, one-handed, on the
  smallest phone at hand.

## 8. Suggested order

1. **Now:** REL-1.
2. **Wave 14, "every action within reach" (before STORE-18 and STORE-24):** PAR-1, PAR-2, TCH-1, TCH-2, TCH-3, TCH-4,
   TCH-6, TCH-7, TCH-8, DSK-1, QA-1, QA-2, QA-5.
3. **Wave 15, "forms, not prompts":** PAR-3, PAR-4, PAR-5, PAR-6, PAR-7 with TCH-9 and DSK-6, TCH-5, PAR-8, QA-3, QA-4.
4. **Wave 16, "dail in the package managers":** DIST-1, DIST-2, DSK-2, DSK-3, DSK-4, DIST-3, DIST-4, DIST-5, DIST-7,
   then DSK-5 and DIST-6.
5. **Wave 17, "the App Store" (after the Android launch):** IOS-1 to IOS-7.
6. **Later:** TCH-10, TCH-11, TCH-12.

Wave 14 is about nine agent-sized changes. It delays the closed test by that much and is worth it: the closed test's
purpose is to find what this review could not.

## 9. What this review did not establish

- **Nothing was run or seen.** The cramped bars (TCH-2) and the off-screen *Rules* tab are read from code, not observed.
- **Why rules could not be used.** "Not released" is the strongest explanation. If the build in question was made from
  `go/misc`, the cause is one of the other two in section 2, and TCH-6 to TCH-8 matter more.
- **The TUI.** Only its settings screens were compared. PAR-2's table will show its real gaps.
- **Store rules outside Play and F-Droid.** App Review's requirements, winget's validation, Homebrew's notability
  thresholds and Flathub's sandbox rules are from prior knowledge. Read the current rules before IOS-4, DIST-3, DIST-5
  and DIST-6.
- **Smoothness.** No timing was measured; QA-3 exists because of that.
- **Sizes.** They are estimates by analogy with finished items of the same shape. iOS sizes are the least certain.
