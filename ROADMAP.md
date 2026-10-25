# dail — Roadmap

Each checkbox is roughly one agent-sized change. A box is ticked only when all gates in `AGENTS.md §6` are green.
Command names refer to the catalogue in [`ARCHITECTURE.md §8`](ARCHITECTURE.md#8-initial-command-catalogue-mvp).

Legend: **[core]** app-core · **[io]** app-io · **[sync]** app-sync · **[mcp]** · **[cli]** · **[gui]** · **[tk]** app-testkit · **[front]** app-front · **[tui]** app-tui
· **[mobile]** app-mobile · **[android]** app-android · **[kt]** the Kotlin app (`android/`)

---

## Wave 1 — MVP: "a daily-drivable client for Hostinger + Gmail, fully agent-controllable"

**Exit criteria**
- One Hostinger and one Gmail (App Password) account work side by side.
- Sync, read, triage, compose/send and search all work offline-tolerant.
- Every catalogue command works via GUI/palette, `dail exec`, `dail batch` and MCP. The parity test is green.
- An agent attached via `dail mcp` to a running GUI can triage the inbox and draft replies. Sending needs user approval.
- A replay log from a real session reproduces the same state offline.

### M0 — Foundation
- [x] Scaffold all crates (`[lints] workspace = true`, workspace deps), `xtask verify` running the gates from AGENTS.md §6
- [x] [core] typed IDs, `Ctx`, `Origin`, `AppState` skeleton, `Engine::{dispatch, ingest}`, `Outcome`, `CommandReceipt`, `CoreError`
- [x] [core] `CommandKind`, `CommandSpec`, `Command::specs()` (exhaustive), history stack with `undo`/`redo`
- [x] [core] JSONL replay format + replay test harness; first fixture
- [x] [tk] `MemoryBackend` (fake IMAP/SMTP), `MemoryStore`, fixture loader
- [x] [sync] `Session` single-writer loop, `Backend` trait, effect executor skeleton, virtual clock for tests
- [x] [cli] `dail exec`, `dail batch` (file/stdin, `--keep-going`, `--backend memory`), `dail schema`; `tests/smoke/basic.jsonl`
- [x] [mcp] stdio server; tools generated from registry with kind annotations; registry ⇔ MCP ⇔ CLI parity test
- [x] CI-ready: `cargo deny check` passes with `deny.toml`

### M1 — Accounts
- [x] [core] `Account`, `ProviderPreset {hostinger, gmail, custom}`, `add_account` / `update_account` / `remove_account` (Destructive) / `test_account`
- [x] [io] `config.toml` read/write (no secrets), profile dirs via `directories`
- [x] [sync] keyring `StoreSecret` / `DeleteSecret`; `VerifyAccount` (IMAP LOGIN + SMTP AUTH, TLS via rustls; crypto from app-tls since ADR 0003)
- [x] [cli] alias `dail account add --preset gmail --email …` (password from prompt or `--password-stdin`, never argv)
- [x] Live test (ignored): verify Hostinger + Gmail with `DAIL_IT_*` creds

### M2 — Sync (read path)
- [x] [sync] `ListMailboxes` with SPECIAL-USE + preset mapping + name heuristics → `MailboxesListed`
- [x] [sync] `FetchEnvelopes` (initial window N, incremental by `uid_next`, UIDVALIDITY reset) → `EnvelopesFetched`
- [x] [io] `RedbStore` (tables, schema_version, migrations), content-addressed blobs; `PersistState` effect
- [x] [core] threading (Message-ID / In-Reply-To / References, subject fallback) — pure + tests
- [x] [core] `sync_now`, `list_mailboxes`, `list_threads`; poll `Tick` scheduling; `SyncStatus` in state
- [x] [sync] flag resync for the cached window; lazy `FetchBody` + top-20 unread prefetch
- [x] [io] MIME: text/plain + text/html → text (`html2text`), attachments list, charsets; `.eml` corpus snapshot tests

### M3 — Read & triage
- [x] [core] View: `open_mailbox`, `select_threads`, `open_thread`, `close_thread`; Query: `get_thread`, `get_message`
- [x] [core] `mark_read`/`mark_unread`, `star`/`unstar` with inverse + `PendingOp`
- [x] [core] `archive_threads`, `trash_threads`, `move_threads` (preset-aware: Gmail All Mail, Hostinger Archive auto-create)
- [x] [sync] `RunRemoteOp`: UID STORE / UID MOVE (COPY+EXPUNGE fallback), retries with back-off, reconcile on success/failure
- [x] [core] undo cancels a still-pending op instead of issuing a compensating op; proptest undo round trip over random op sequences
- [x] [core] `list_pending_ops`, `retry_pending_ops`, `discard_pending_op`; failures visible in state
- [x] Offline test: triage with the backend down → ops queue → backend up → server state matches

### M4 — Compose & send
- [x] [core] `Draft`; `create_draft`, `reply`, `reply_all`, `forward` (quoting, `In-Reply-To`/`References`, Re:/Fwd:), `update_draft`, `discard_draft`
- [x] [core] `attach_file` / `remove_attachment` (path recorded, read by runtime); size limit check
- [x] [io] `mail-builder` rendering of drafts (plain text, attachments, Message-ID from Ctx)
- [x] [sync] Drafts: local-first, `SaveDraft` (APPEND, replacing older copies) to Drafts on idle/close
- [x] [core] `send_draft` (Outbound, `send_at = now + undo_send_ms`), `cancel_send`; tick-driven dispatch → `SmtpSend`
- [x] [sync] SMTP send; APPEND to Sent unless the preset says the server saves it (Gmail); `SendFailed` → draft restored with error
- [x] Virtual-clock tests: undo inside the window cancels; after the window it's irreversible

### M5 — Search
- [x] [core] query language parser (`from: to: subject: is: has:attachment in: before: after: "…" -x`) + tests
- [x] [core] `search` over the cached envelopes (Query), `set_search`/`clear_search` (View)
- [x] [sync] server fallback: IMAP `UID SEARCH` translation; Gmail `X-GM-RAW` passthrough; merge results into cache

### M6 — GUI
- [x] [gui] app shell: `Session` on a background tokio runtime, frame command queue, `request_repaint` on `StateChanged`
- [x] [gui] three-pane layout (collapsible sidebar, virtualized thread list, reader), light/dark, one accent colour
- [x] [gui] command palette (Ctrl+K): nucleo fuzzy over available specs + mailboxes + recent threads; second-step arg pickers
- [x] [gui] keymap from `CommandSpec.default_keys` (`j/k e # s u r a f c / z g i`), shortcut hints in palette
- [x] [gui] composer (chips, attachments via `rfd`, send with undo toast)
- [x] [gui] account wizard (preset picker → email → password → test → sync)
- [x] [gui] status bar: sync state, pending ops, errors, undo toast
- [x] [gui] reader: text rendering of HTML, link targets shown, "open original" in browser
- [x] [core] View/App commands `open_composer`, `focus_pane` (`open_palette`, `close_palette`, `set_setting` landed in M0)

### M7 — Agent surface
- [x] [sync] single-instance lock + IPC listener (Unix socket / named pipe); `exec`/`batch`/`mcp` auto-attach
- [x] [mcp] resources (`dail://mailboxes`, `dail://mailbox/{id}/threads`, `dail://thread/{id}`, `dail://state/view`)
- [x] [mcp] `untrusted_content` envelopes everywhere; server instructions on prompt injection and approvals
- [x] [core] `ApprovalRequest` for `Outbound` from `Origin::Mcp`; `approve_request`/`reject_request` (GUI/CLI origin only); `--allow-send`
- [x] [gui] approvals tray (approve / edit / reject)
- [x] Audit: replay log with origin, redacted bodies/secrets; `dail replay <log>` command
- [x] E2E script: agent triages inbox and drafts a reply via MCP against the memory backend (CI)

---

## Wave 1.5 — TUI: "mail from a phone or an iPad over SSH"

A terminal front-end, `dail tui`, for reading, triaging and answering mail from a mobile SSH client (Termius, Blink),
and for testing: under `tmux` an agent can type into it (`send-keys`) and read the screen (`capture-pane -p`), which
the GUI does not allow without desktop automation. Same engine, registry, keymap and palette flows as the GUI; one pane
at a time on a phone, the GUI's three panes on a wide terminal. Every item updates `ARCHITECTURE.md` in the same change
(a new §7.4 TUI, and §3, §5.4, §9, §11 and §12 where it touches them).

**Exit criteria**
- On a phone in portrait (40 columns, 12 rows and up), on an iPad and in an 80×24 terminal, `dail tui` reads mail,
  triages (archive, trash, move, star, read/unread, undo/redo), searches, composes, replies, forwards, sends with the
  undo window, and approves or rejects an agent's send request.
- It attaches to the profile's running owner (the GUI or `dail mcp`), so an agent's changes show up live. With no owner
  it opens the profile itself and lets `dail exec` and `dail mcp` attach to it.
- Every registry command can be reached by a default key that a plain terminal delivers or through the palette, with
  the GUI palette's exceptions (parity test).
- Email content cannot send control sequences to the terminal (test). The TUI never opens a browser or a file dialog:
  over SSH they would open on the host.
- It works inside tmux and mosh. Quitting or a dropped connection saves the drafts; a send still inside its undo window
  is cancelled visibly, never silently.
- The GUI and the TUI share the palette flows, the keymap and the session bridge. All gates are green, and the TUI
  tests run headless (no TTY in CI).

**Decisions** (recorded in ADR 0004, the first item of T0)
- **Crates.** `app-tui` (← app-core, app-sync, app-front) uses `ratatui` 0.30, its `ratatui::crossterm` re-export
  (crossterm 0.29, so the two never drift) and `ratatui-textarea` 0.9: all MIT, MSRV ≤ 1.88. A new `app-front`
  (← app-core only: no toolkit, no tokio) takes the toolkit-free half of app-gui — palette flows, fuzzy ranking,
  keymap, formatting, status and approval projections — so that neither front-end re-derives the other's rules. The
  TUI never links egui; app-gui and app-tui do not depend on each other.
- **Session.** Attach first, like `dail mcp` (ARCHITECTURE §5.4); else own the profile and listen on its socket.
  Attached front-ends share one `ViewState`: what the phone opens, the desktop shows, and the other way round.
- **Origin.** A new user origin `tui`. It may approve agent sends, and an IPC client may claim it, like `cli`.
- **Layout** is a function of `ViewState` and the terminal size. A resize never dispatches a command: the view is
  shared with the GUI and with agents.
- **Keys.** One keymap, from `CommandSpec.default_keys`. Assume a plain xterm-like terminal: Ctrl+Enter arrives as
  Enter and Ctrl+Shift+letter as Ctrl+letter, F-keys and Esc may be missing (iPad keyboards), Ctrl+B and Ctrl+A belong
  to tmux and screen, and there is no clipboard escape (OSC 52) and no kitty keyboard protocol. TUI-local keys only
  scroll, show help (`?`), stand in for Escape (`q`, Backspace) or quit.
- **Terminal hygiene.** Email text is sanitized before it is drawn; no OSC 8 links, no window title. The terminal's own
  colours (default foreground and background, bold, dim, reverse; `NO_COLOR` respected) and single-width glyphs only,
  so that light and dark terminal themes both work. In TUI mode `tracing` writes to a file in the profile's `logs/`,
  because stderr is the screen.
- **Tests.** ratatui's `TestBackend` with a real `Engine`, like `app-gui/tests`; `insta` snapshots of the buffer at
  40×20, 80×24 and 120×40; the clock injected. Look at it under tmux:
  `tmux new -d -s t -x 50 -y 20 'cargo run -p app-tui --example demo'`, then `tmux send-keys -t t j Enter` and
  `tmux capture-pane -p -t t`.

**Out of scope** (Wave 2, for both front-ends): multi-select, collapsed quotes and signatures, HTML rendering.

**Known limitations**
- A profile opened from a plain SSH login often cannot reach the OS keyring (no D-Bus session, or a locked keyring), so
  it cannot log in; the status line shows the keyring error. Attach to a GUI that runs in the desktop session, or start
  the TUI inside tmux from the desktop session and `tmux attach` over SSH.
- HTML is rendered at 100 columns (`app_io::mime::TEXT_WIDTH`) and senders wrap plain text near 72, so text wraps
  raggedly on a phone. Reflowing paragraphs is a follow-up.
- An attached TUI reads the whole state snapshot again after each change. That is fine for a few thousand messages;
  deltas come later.

### T0 — Groundwork (no TUI yet; the GUI works as before and its tests stay green)
- [x] ADR 0004 (the decisions above); `app-front` and `app-tui` in ARCHITECTURE §3 and in the dependency rules of AGENTS.md §2
- [x] [front] Scaffold `app-front` and move the toolkit-free helpers out of app-gui: `format`, `reader::links`, the body tracking (`Bodies`, working on receipts), `statusbar::{problems, sync_text}`, `approvals::cards`, `composer::parse_addr`, `sidebar::mailbox_name`, receipt → notice text and undo label
- [x] [front] Palette logic: `flow`/`Ask`/`Choice`/`AskKind`, `targets`, `current_account` and the nucleo ranking of commands, mailboxes and recent threads; the flow walk of the palette parity test moves with it
- [x] [front] Keymap: a toolkit-neutral `Chord` (modifiers + named key or character), `bindings()`, `resolve()` and the sequence state (`g i` within 1.5 s, the time passed in); app-gui maps egui events onto it (`tests/it/keys.rs` stays green)
- [x] [core] `Origin::Tui`, a user origin (`is_user`: it may approve agent sends); [sync] IPC clients may claim `tui` (still not `gui` or `runtime`); tests in `app-core/tests/it/agents.rs` and `app-sync/tests/it/ipc.rs`
- [x] [sync] IPC `watch`: on a connection of its own the owner sends a `changed` notification after state changes (coalesced); `Connection::subscribe()` gives local and attached front-ends the same `watch::Receiver<Arc<AppState>>` (attached: `state` is read again after a notification, one read at a time)
- [x] [sync] Move `Bridge` from app-gui to app-sync: it runs over a `Connection`, submits with the front-end's origin, wakes the front-end through a callback, tags each result for it, and shuts the session down only if it owns it; opening the browser stays in app-gui (`tests/it/bridge.rs` stays green)

### T1 — Terminal shell
- [x] [tui] Scaffold `app-tui`: `Tui`, a projection like `Shell` (`draw(frame, &state)`, `input(event, &state) -> Vec<Request>`, the clock injected); test support (`TestBackend`, a real `Engine`, `insta` buffer snapshots); `examples/demo.rs` like the GUI's (memory backend, `basic` fixture, account synced; `open`, `compose`, `agent`)
- [x] [tui] Terminal lifecycle: raw mode, alternate screen and bracketed paste, restored on quit, on error and on panic; one input queue for keys, resizes, pastes, bridge wake-ups and a 1 s tick while a countdown or an age is shown; redraws coalesced; a dead terminal, SIGHUP or SIGTERM (console close on Windows) shuts the session down cleanly (drafts saved)
- [x] [cli] `dail tui` (the global flags apply): attaches to the profile's owner, or owns the profile and listens; an attached TUI whose owner goes away says so and exits; without accounts it shows the `dail account add` line to run in another shell (the account then appears live); plain `dail` starts the TUI on Linux when neither `DISPLAY` nor `WAYLAND_DISPLAY` is set and it runs in a terminal; `tracing` goes to a log file

### T2 — Read
- [x] [tui] Adaptive layout: ≥ 120 columns sidebar, list and reader (or composer); 80–119 list and reader, the mailboxes in the list's place while the sidebar has the focus; < 80 one pane: the mailboxes while the sidebar has the focus, else the composer, else the open thread, else the list. Below 40×12 a "terminal too small" notice. One header line and one status line; borders only from 80 columns. Snapshots at the three test sizes
- [x] [tui] Mailboxes (`listing::mailboxes`: accounts, roles, unread counts) and the thread list (`listing::visible_threads` and `ThreadSummary`: unread `●`, starred `★`, attachment, sender, subject, date); the cursor is `view.selected`, as in the GUI, and the list scrolls to keep it visible
- [x] [tui] Reader: bodies through the shared body tracking (`get_thread {include_body: true}`), a header per message (from, to, cc, date), the text, the attachments and *Links (N)* with their full targets; Space/PageDown and `b`/PageUp scroll, and ↑/↓ too when the reader is the only pane or has the focus (`j`/`k` still move between threads); no browser and no "open original"
- [x] [tui] Untrusted text: every widget draws email text through one sanitizer (C0 and C1 controls, ESC, DEL and bidi overrides replaced, tabs expanded). Test: a fixture whose subject, names and body carry `ESC ]52;…`, `ESC [2J`, `\r` and U+202E leaves none of them in the buffer of the list, the reader, the palette or the approvals

### T3 — Act
- [x] [core] Registry keys that a plain terminal delivers, next to the existing ones (the GUI gets them too): `send_draft` `Ctrl+S` (for Ctrl+Enter), `redo` `Shift+Z` (for Ctrl+Shift+Z), `open_palette` `:` (for Ctrl+Shift+P), `toggle_sidebar` `[` (for Ctrl+B), `sync_now` `Ctrl+R` (for F5). A TUI test checks that every command with default keys has one that a plain terminal delivers
- [x] [tui] Keys through the app-front keymap (`j k Enter o e # s u v z / Tab Escape c r a f`, the `g` sequences, arguments from the view, a missing argument opens the palette at that step); TUI-local keys only as above: `?`, scrolling, `q`/Backspace as Escape outside text fields, `q` on the top level and Ctrl+C to quit. A paste outside a text field is dropped, never run as keys
- [x] [tui] Palette overlay (`Ctrl+K`, `:`): fuzzy over the commands (with their keys), the mailboxes, recent threads and "Search mail for …"; second-step pickers from the app-front flows; query results in a scrollable read-only view marked as untrusted data; the parity walk covers the TUI palette
- [x] [tui] Status line: `syncing…` / `synced 2m ago`, pending and failed ops (retry from the palette), store, keyring, login and sync errors, the last undoable action with `z`, *Sending in N s*; rejections and runtime failures show there for a few seconds, like the GUI's toasts. `?` lists every registry binding and the TUI-local keys
- [x] [tui] Agent send requests: a banner while `state.approvals` is not empty; the requests view shows each request in full (account, every recipient, the first lines of the text, every attachment with its full path and its size or state); approve (after a confirmation), edit (opens the composer) or reject

### T4 — Compose & send
- [x] [tui] Composer for `c`/`r`/`a`/`f` and `open_composer`: To/Cc/Bcc (`parse_addr`; invalid text stays in the field), Subject, the body in `ratatui-textarea` and the quote read-only below it; typing goes back as one `update_draft` per pause, field change or key command, like the GUI; an agent's edit reloads the fields unless there is unsaved typing. The registry's Ctrl chords win over the editor's own shortcuts, as in the GUI
- [x] [tui] `$VISUAL`/`$EDITOR` for the body (a key in the composer's footer): the TUI leaves the alternate screen and stops reading input while the editor runs; the text goes through a private temporary file (0700 directory, 0600 file, removed afterwards) and comes back as one `update_draft`; without an editor a notice says so
- [x] [tui] Send (`Ctrl+S` or the palette) with the undo window (*Sending in N s*, `z` cancels); `attach_file` from a typed absolute path of a file on the host (`~/` expanded; no file dialog), attachment chips with checking, size or error; quitting a TUI that owns the session while a send is scheduled asks first, because closing inside the undo window cancels the send (ARCHITECTURE §4.4)

### T5 — On the device
- [x] [tui] Attached end-to-end, like `app-gui/tests/it/bridge.rs`: a `Tui` over an IPC client of a running session shows an agent's archive without polling, and its `approve_request` goes through with origin `tui`
- [ ] Device check with Termius on an iPhone and an iPad (Blink if at hand), against the demo and a live profile: portrait, landscape and rotation, the rows left with the on-screen keyboard up, Esc, Ctrl and arrows from the extra-keys bar, paste, tmux and mosh, a dropped connection. What works and what does not goes into ARCHITECTURE §7.4; adjust the breakpoints and keys

### Stretch
- [x] [tui] Mouse and touch: the wheel (a swipe) scrolls, a tap selects or opens; a key turns mouse capture off, so that the terminal's own selection can copy a link
- [x] [tui] Account setup: preset → address (servers for custom) → masked password (straight into `add_account` as a `Secret`, the buffer cleared) → `test_account` → `sync_now`. Until then `dail account add` in a shell does it

---

## Wave 2 — Polish: "pleasant, fast, robust"

**Reading & rendering**
- [x] HTML subset renderer in egui (paragraphs, headings, lists, blockquotes, links, tables-lite)
- [x] Inline images opt-in per sender: `cid:` parts of the message drawn in place once the user allows the sender
  (`show_images_from` / `hide_images_from`); remote images stay off
- [x] Conversation view: collapsed quoted text, per-message expand, signature folding
- [x] Attachments: save / save-all to a folder (`save_attachments`, user only: agents don't write files), never
  overwriting a file
- [x] Attachments: preview images and text in the reader; PDFs open in the browser's viewer (a private temporary
  copy). Other types are saved, not opened: there is no "open with" for attacker files
- [x] Attachments: drag files into the composer to attach them
- [ ] Attachments: drag out of the reader — blocked: eframe 0.37 / winit 0.30 cannot start an OS drag; revisit when egui can

**Sync & performance**
- [x] IMAP IDLE push for INBOX (+ fallback polling)
- [x] CONDSTORE fast resync: only the flags changed since the last resync (`CHANGEDSINCE`), expunges from a UID search
- [ ] QRESYNC: VANISHED instead of the UID search. Needs `ENABLE QRESYNC` on its own connection (it turns every later
  EXPUNGE response into VANISHED, which async-imap's expunge does not parse) and VANISHED read from async-imap's bounded
  unsolicited channel
- [x] Full mailbox backfill in background with progress; configurable window per mailbox
- [x] 100k-message mailboxes: scale measurements (`app-core` and `app-gui` `scale` tests, `--release --ignored`); the
  fetch merge was quadratic (change lists, placeholder lookup): a 50k first fetch went from 13 s to 0.2 s
- [x] Incremental threading: the engine keeps each account's `Threader` between merges (a backfill batch at 100k cached
  messages went from ~66 ms to ~8 ms, an incremental fetch from 91 ms to 15 ms)
- [x] List projection kept between state changes (`AppState.revision`): a GUI frame with 100k threads went from ~46 ms
  to ~0.5 ms (the rows and the header's unread count were recomputed every pass)
- [x] Store compaction: redb `compact()` and garbage collection of blobs no message refers to (`compact_store`)
- [x] Full-text search index (`tantivy`, native only) over downloaded bodies (ADR 0005)
- [x] Search ranking + highlighting: `search {order: relevance}` in score order; the search words marked in the list and the reader
- [x] Per-account effect workers (one slow or unreachable server no longer delays the other accounts), store writes kept in order

**Productivity**
- [x] Multi-select + bulk ops (shift/ctrl click, `x` select, select-all-matching)
- [x] Contacts harvested from mail → recipient autocomplete (`list_contacts`; the GUI composer suggests them)
- [x] TUI: recipient completion from `list_contacts` (Tab in a recipient field)
- [x] Identities/aliases per account, signatures, default From per mailbox
- [x] Unified inbox across accounts; per-account colour tag
- [x] Palette: recents/frecency, arg flows for all commands, inline help from doc comments
- [x] Configurable keymap (`config.toml` + `set_keybinding` command), vim/Gmail presets

**UX & platform**
- [x] Onboarding: autoconfig over HTTPS (`autoconfig.<domain>`, `/.well-known/autoconfig`, Mozilla ISPDB) for
  unknown domains: `discover_servers`, *Find servers* in the wizards (ADR 0006)
- [ ] Onboarding: DNS SRV records (RFC 6186) as one more source — split from autoconfig: needs a pure-Rust DNS resolver
  crate (a new dependency class, its own ADR)
- [x] Desktop notifications (new mail, send failed, approval needed) while the window is in the background;
  `notifications` setting
- [ ] Optional tray icon — split from notifications: a StatusNotifierItem over D-Bus on Linux (the GTK-based `tray-icon`
  links C libraries) and `Shell_NotifyIcon` on Windows, plus hiding/restoring eframe's window from it
- [x] Error & retry UX: actionable messages (auth failed → re-enter password with `set_password`; quota; TLS
  ciphers and certificates; network → retry), in the GUI status bar, the composer, the TUI and `dail account password`
- [x] Accessibility pass (AccessKit labels, focus order, high-contrast theme), font scaling, themes: named list and
  mailbox rows, high-contrast dark/light (`theme` 3/4), `text_scale` setting
- [x] i18n scaffolding (UI strings table): `app_front::i18n` keys with English templates and a pseudo-locale
  (`DAIL_LANG=pseudo`); the shared status texts use it
- [ ] i18n: move the GUI's and TUI's own labels into the strings table; a first real translation
- [ ] RTL layout — split from i18n, blocked: egui 0.37 has no bidirectional text or mirrored layouts (RTL mail shows in
  logical order); bidi controls from mail already cannot reorder the UI
- [x] Packaging: `.deb` for Ubuntu (`cargo deb -p app-cli`: binary, desktop entry, icon; `packaging/`); the GitHub
  Actions matrix CI (Ubuntu + Windows) exists already
- [ ] Packaging: AppImage and MSI, and a release workflow that builds all three on tags — split from the `.deb`: needs
  `appimagetool` and WiX on the CI runners, which this machine can't run or check; `cargo-dist` makes neither `.deb`
  nor AppImage, so the tool choice is open
- [x] Crash-safe store with backups; `dail doctor` diagnostics command (durable atomic writes, daily backups of
  `config.toml` + `state.redb`, a cache that does not open restored from the newest backup)

**Build & dependencies**
- [x] No C in the build: replace rustls's `ring` with `app-tls`, a crypto provider for dail's TLS client in safe Rust
  (ChaCha20-Poly1305, X25519, RSA/ECDSA verification; ADR 0003, `docs/plan-tls.md`)

**Agent ergonomics**
- [x] MCP prompts (e.g. "triage inbox", "summarize thread") shipped as templates: `triage_inbox`, `summarize_thread`,
  `draft_reply`
- [x] Streamable HTTP MCP transport (localhost, token-protected) in addition to stdio: `dail mcp --http` (ADR 0007)
- [x] Rich receipts: diffs of changed entities for agents; `dry_run: true` flag for every Mutation — `Engine::preview`:
  the receipt, effects by name and the changed fields of each affected entity, on `dail exec --dry-run`, batch lines
  and every data-changing MCP tool
- [ ] Diffs in the receipts of real (not dry-run) commands too — split from the dry runs: needs the affected entities'
  JSON before each dispatch, a cost every command would pay; to be measured at 100k first

## Wave 3 — Visual refresh: "the mockups, built from a theme library"

The GUI restyled after the Stitch mockups in `docs/ui/`, on top of a toolkit-free library of design tokens. The plan,
the review of the mockups and the recorded choices (D1–D10) are in
[`docs/requests/2026-10-21-ui.md`](docs/requests/2026-10-21-ui.md); each item below names its section there. The IDs
are the build order: part A before part B (B uses the kit), UI-5 first in part B, UI-11 before UI-12. Part D items are
independent.

**Exit criteria**
- Widgets read colour roles and metrics only: the style check's allowlist is empty (UI-10).
- `dail-dark`, `dail-light`, `dail-dark-hc` and `dail-light-hc` pass the contrast audit, and the user picks them by
  name (`set_theme`, the settings page). Old `theme` 3 and 4 values keep working.
- Geist, JetBrains Mono and the Material Symbols subset are bundled with their licences; no emoji glyph is left in the
  GUI's own chrome.
- The AccessKit names do not change; every GUI test stays green or moves with its widget. Each part B item reports
  before and after screenshots against `docs/ui/*/screen.png` (plan §5).

### A — The library
- [x] UI-1: ADR 0008 and the `app-theme` crate: colour roles, type scale, metrics, built-in themes, contrast audit
- [x] UI-2 [gui]: Fonts: Geist and JetBrains Mono (variable) in `app-theme/assets/fonts/`, three weights through
  `FontTweak::coords`, egui's fonts as fallbacks, `TextStyle` mapped onto the type scale; licence texts in the `.deb`;
  binary size measured before and after (release `dail`: 37 583 824 → 38 000 464 bytes, +407 KB)
- [x] UI-3 [gui]: Icons: a Material Symbols Outlined subset and `app_theme::Icon` replace every emoji glyph; role icons
  total over `MailboxRole`; ARCHITECTURE §7.1 (the missing `✓`) (release `dail` +37 KB, 38 038 224 bytes)
- [x] UI-4 [gui]: egui adapter (`app_gui::theme` → `Tokens`, `Visuals` per theme, chrome visuals), component kit
  `app_gui::ui` replacing `widgets.rs`, `gallery` example, style check (`tests/it/style.rs`) with an allowlist

### B — The surfaces
- [x] UI-5 [gui]: Frame: sidebar (brand, *Compose*, nav rows, *Settings*), 56 px rail, top bar (search pill → palette,
  sync state, status chip); the status bar goes away and its parts move (D7, D8); ARCHITECTURE §7.1
- [x] UI-6 [gui] [front]: Thread list: 48 px header with unread badge, cozy (48) and compact (40) rows, footer with key
  hints, undo label and count; `scale` tests before and after (100k threads: 0.76 → 0.73 ms a frame)
- [x] UI-7 [gui]: Reader: 48 px toolbar, `headline_xl` subject, sender card, collapsed messages, body, attachment
  cards, links, empty state
- [x] UI-8 [gui]: Composer: header with save state, labelled hairline fields, chips, `body_lg` body, footer, drop zone
- [x] UI-9 [gui] [front]: Palette: `overlay` frame, sections for an empty query (grouping helper in
  `app_front::palette`), icons, key caps, footer
- [x] UI-10 [gui]: Overlays: toasts (fixes the 1.70:1 error toast), approvals tray, account wizard, query window,
  menus and tooltips; the style check's allowlist is empty

### C — Settings
- [x] UI-11 [core] [io] [front] [gui]: `set_theme { mode?, light?, dark? }` (Mutation, agents allowed); settings
  `theme_light`, `theme_dark`, `list_density`; legacy `theme` 3/4 mapped in config and engine; replay fixture
  `set_theme.jsonl`; ARCHITECTURE §7.1, §8
- [x] UI-12 [core] [front] [gui] [tui]: Settings page: `open_settings { section? }` (`Ctrl+,`, `g ,`) and
  `close_settings` (View); tabs Appearance, Keyboard, General, Accounts; the TUI's key/value lists; ARCHITECTURE §7.1,
  §7.4, §8

### D — Optional
- [x] UI-13 [gui]: Reply composer docked under the conversation (a presentation of `view.composer`, no new command)
- [x] UI-14 [core] [io] [sync] [tk] [gui]: Snippets in the list rows: `Envelope.snippet` from fetched bodies, untrusted
  in MCP/CLI output, a "preview" density (72 px)
- [x] UI-15: System fonts for scripts the bundled fonts lack (eframe `system_fonts`; deny and no-C gates, Windows too)
- [x] UI-16 [tui]: Opt-in TUI colours from `app-theme` (`tui_colors`); amends ADR 0004

---

## Wave 4 — Android: "a native phone app on the same engine"

dail as an Android app (Android 10+; phones first, tablets, foldables and Chromebooks with two or three panes) on the
same engine, registry, keymap, palette flows and agent policy as the desktop. Kotlin and Jetpack Compose draw screen
models that a new Rust crate, `app-mobile`, computes from `AppState` — the TUI's pattern, with JSON frames instead of a
terminal buffer — and do what only Android can: text input through any IME, TalkBack, notifications, background work,
the Keystore, file pickers and intents. The plan, the options weighed, the platform facts and the review of the agent
part are in [`docs/plans/2026-10-22-android.md`](docs/plans/2026-10-22-android.md); ADR 0009 (the first item of A0)
records its decisions. Every item updates `ARCHITECTURE.md` in the same change (a new §5.5 Android host and §7.5
Android, and §2, §3, §8, §9, §11 and §12 where it touches them).

**Exit criteria**
- On an Android 10+ phone, a Hostinger and a Gmail account work side by side: read, triage (swipes, checked threads,
  undo and redo), search, compose with any keyboard (swipe typing, CJK, voice), reply, forward, attachments in and out,
  send with the undo window. Tablets and foldables show two or three panes.
- Leaving the app never loses work: a send inside its undo window still leaves (or is cancelled visibly), and queued
  triage and drafts reach the servers. New mail notifies within the background interval (30 min by default), or within
  about a minute with push turned on.
- Every registry command is reachable from the mobile palette (parity walk). In debug builds, an agent drives and
  reads the app through the adb bridge.
- Email content cannot spoof the UI: control and bidi characters are replaced, links show their full target before
  they open, remote images never load. A shared file or a `mailto:` link only creates a draft, and no intent can attach
  the app's private files.
- Passwords live only in Keystore-protected storage; nothing secret in logs, logcat, backups or the audit log.
- Kotlin holds no rules: it draws frames, returns events and carries out host actions. The APK carries no native code
  but dail's Rust library (16 KB pages, target API 36). All desktop gates stay green, plus the `android` gate and the
  emulator job.

**Decisions** (ADR 0009, from the plan; chosen with the user on 2026-10-22)
- **UI.** Kotlin + Jetpack Compose over a Rust presenter. Not egui: winit 0.30 on NativeActivity has no IME
  composition, and GameActivity compiles C++. Not UniFFI: its Kotlin bindings load JNA's prebuilt C library.
- **Crates.** `app-mobile` (← app-core, app-front, app-theme; no toolkit, no tokio, no JNI): `Mobile` with
  `frame(&state) -> Frame`, `event(UiEvent, &state) -> Vec<Request>`, `replies` and `externals`; every model
  `Serialize + JsonSchema`. `app-android` (← app-core, app-io, app-sync, app-mobile; cdylib `libdail.so`): the `Host`
  and six JNI calls (`open`, `event`, `frame`, `blob`, `background`, `close`) through `jni` 0.22, carrying the JSON the
  command bus speaks. Its `[lints]` allow `unsafe` only in `src/jni.rs` (the exports); every other crate keeps
  `forbid`.
- **Origin.** The app is the user's graphical front-end, in-process like the GUI: `Origin::Gui`. Background checks
  submit as `runtime`.
- **Process.** One session per process, opened lazily and kept while the process lives (redb holds every change). A
  foreground service carries a send from `send_draft` to its result; a one-off job flushes queued work when the app
  leaves the screen; a periodic JobScheduler job checks mail (`background_sync` 1, the default, every
  `background_interval_mins` = 30); push (`background_sync` 2) is a `specialUse` foreground service around the
  engine's IMAP IDLE watches, like Thunderbird for Android's.
- **Platform.** minSdk 29 (Android 10), compile and target SDK 36; arm64-v8a and x86_64; no Google Play Services,
  Firebase or WorkManager. The first release is a signed APK per ABI on GitHub releases.
- **Secrets and files.** keyring 4 with `android-native-keyring-store` (Keystore-wrapped, in SharedPreferences); the
  profile in `filesDir/profiles/default`, backups off; picked and shared files are copied into the app before
  `attach_file`; saving goes through the Storage Access Framework, never "open with".
- **Agents.** No agent channel in release builds, because no on-device client could use one: Gemma 4 on the phone gets
  4k–10k tokens of context, dail's MCP tool list is ~22k, and the system assistant reaches apps through AppFunctions,
  which only privileged apps may call (plan §4.7). A desktop agent triages the same mailboxes from the desktop. A
  debug-only adb bridge (`adb shell content call …`, shell UID only) submits commands as `cli` or `mcp`; with
  `uiautomator dump`, `screencap` and `input` an agent drives and reads the app like the TUI under tmux.
- **Tests.** `app-mobile` frames over a real `Engine` as `insta` JSON snapshots at three window sizes; contract
  fixtures that the Kotlin types decode strictly; Compose tests on the JVM (Robolectric) over the host-built
  `libdail.so` and the memory backend; the Rust tests on an emulator; instrumented smoke tests.

**Out of scope** (plan §10): an agent channel and approvals on the phone, Google Play and F-Droid, iOS (a SwiftUI host
over `app-mobile`), OAuth2 (with the desktop item), PDF viewing and "open original", widgets, Wear OS, at-rest
encryption (SEC-1), 32-bit ARM.

**Known limitations**
- Periodic checks wait for Doze's maintenance windows, and some vendors' battery managers stop background work until
  the user exempts the app; push is the remedy, at the cost of a permanent notification.
- If Android kills the process inside a send's undo window anyway, the send comes back cancelled (ARCHITECTURE §4.4)
  and the app says so in a notification.
- Agents work on the desktop: their triage reaches the phone through the servers, and their send requests are approved
  on the desktop.
- User-installed CA certificates are not trusted (webpki roots, as on the desktop).
- No in-app updates: Obtainium can follow the GitHub releases.

### A0 — Groundwork (no Android app yet; the desktop works as before)
- [x] ADR 0009 (the decisions above, from plan §1–§4); `app-mobile` and `app-android` in ARCHITECTURE §3 and in the
  dependency rules of AGENTS.md §2; AGENTS.md §0 (targets), §1 (channel parity on Android), §5 (Kotlin, the lint
  exception) and §6 (the `android` gate)
- [x] [front] [io] Move what the new crates share, behaviour unchanged: the sanitizer `app_tui::text::{is_unsafe,
  clean, clean_lines}` → `app_front::text`; the rows' accessible names (`app_gui::list::row_name`, the sidebar's) →
  `app_front::a11y`; the pane choice of `app_tui::layout::arrange` → `app_front::panes`; `app_gui::images::decode`
  (formats, size limits) → `app_io::images`. The GUI's and TUI's tests stay green
- [x] `cargo xtask android check`: clippy (`-D warnings`) for `aarch64-linux-android` and `x86_64-linux-android` over
  the crates the app links (the rustup targets only: clippy never links, so cargo-ndk and the NDK wait for A1's
  library build); the `android` gate (`cargo xtask verify android`, reported as not run without
  the NDK); a CI job
- [x] `cargo xtask android test`: the test binaries of app-core, app-io, app-tls and app-sync built for
  `x86_64-linux-android` and run on an emulator through adb (redb, tantivy's mmap, getrandom, tokio and TLS on
  bionic); a CI emulator job

### A1 — The Rust side (no UI yet)
- [x] [mobile] Scaffold `app-mobile`: `Mobile`, `Frame`, `UiEvent`, `External`, `Request`/`Purpose`; window classes
  (compact < 600 dp: one pane, as the TUI below 80 columns; medium: list and detail; expanded: drawer, list and
  detail); the action table; the clock injected; tests over a real `Engine` with `insta` JSON snapshots at 360×800,
  700×1000 and 1280×800 dp; `examples/frames.rs` prints the demo's frames
- [x] [mobile] Contract: the tests write every snapshot frame and event as JSON fixtures into
  `android/app/src/test/resources/contract/` (a stale fixture fails; `DAIL_UPDATE_CONTRACT=1` rewrites them); the JSON
  Schema of `Frame` and `UiEvent`
- [x] [android] Scaffold `app-android` (cdylib + rlib, `libdail.so`): `Host` opens a profile directory
  (`ProfileDirs::at`) with `LiveBackend`, or the memory backend behind the `demo` feature, and runs `Bridge::own` with
  `Origin::Gui`, the presenter and `event`/`frame`/`blob`/`background`; tests on the desktop, without JNI
- [x] [android] JNI: `JNI_OnLoad` and the six calls (`jni` 0.22), handles that are small integers, the wake-up
  callback through attached threads (`Listener.alert` comes with the notifications, A5), panics caught at the boundary, errors as JSON; `tracing` to logcat (`warn`
  in release) and to `<profile>/logs/android.log`; the lint exception confined to `src/jni.rs` (a test pins the
  crate's `[lints]` to the workspace's but for `unsafe_code`)
- [x] [android] Secrets: keyring 4's `android-native-keyring-store` as the default store (ndk-context set up in
  `open`); `deny.toml` for its dependencies; a live round trip on a device (ignored test); the Kotlin Keystore vault
  behind `SecretStore` only if the crate fails review

### A2 — Android shell
- [x] [kt] Gradle project `android/`: version catalog, Kotlin, Compose, compile and target SDK 36, minSdk 29, ABI
  splits; a Gradle task builds `libdail.so` with cargo-ndk (16 KB pages); `DailApplication` holds the `Host`;
  `MainActivity` draws frames taken on a single "dail-ui" thread (edge to edge, predictive back from `frame.back`);
  flavours `live` and `demo` (memory backend, `basic` fixture, like the GUI's demo example)
- [x] [kt] The Gradle side of `cargo deny`: dependency verification (checksums), a licence allowlist, a formatter and
  Android Lint with warnings as errors; `cargo xtask android apk-check`: nothing but `libdail.so` (or an allowlisted
  prebuilt) under `lib/`, 16 KB alignment, the permissions pinned; all of it in `cargo xtask verify android`
- [x] [kt] Debug adb bridge (debug builds only): a content provider that answers only the shell UID — `exec` (origin
  `cli`), `exec_agent` (origin `mcp`), `state`, `frame`; the instrumented tests use it, and so do agents, with
  `uiautomator dump`, `screencap` and `input` (plan Appendix D)
- [x] [mobile] [kt] Theme: `app-theme`'s tokens in the frame (roles, type scale, metrics; only when they change) mapped
  onto Material 3; Geist, JetBrains Mono and the Material Symbols subset from `crates/app-theme/assets/fonts`, with
  their licences on an About screen; the light, dark and high-contrast themes; the mode follows `theme` and the system

### A3 — Read & triage
- [x] [mobile] [kt] Drawer (modal on phones, permanent on wide screens, a rail after `toggle_sidebar`): accounts with
  their colour tags, mailboxes with role icons and unread counts, *All inboxes*, *Settings*, *Add account*; opening it
  is `focus_pane sidebar`, as in the TUI
- [x] [mobile] [kt] Thread list: rows in the three densities (sender, subject, snippet, date, unread, star, attachment,
  account tag), a window of rows around what is visible (a `scale` test: a frame over 100k threads within budget), pull
  to refresh (`sync_now`), empty states; the top bar's sync text and a banner with the first problem and its fix
  (`app_front::status`); the undo snackbar (`LastAction`, `undo`)
- [x] [mobile] [kt] Gestures and checked threads: swipe left archives, right trashes; a long press checks
  (`check_threads`) and shows the bulk bar (archive, trash, read, unread, star, uncheck); haptic feedback
- [x] [mobile] [kt] Reader: collapsed and expanded messages, HTML drawn natively from `app_core::document`
  (paragraphs, headings, lists, quotes, preformatted text, tables, links), plain text with folds; the toolbar (archive,
  trash, star, read, *Move…* opens the palette at its step), the previous and next conversation; *Links (N)*; a tap on
  a link shows its full target first (*Open* in the browser, *Copy*); Back is the `Escape` binding
- [x] [mobile] [kt] Attachments out and images: image and text previews in the app (bytes through `blob`, decoded by
  `app_io::images`); *Save* and *Save all* through the Storage Access Framework (names from `safe_file_name`, never
  overwriting); no "open with"; inline `cid:` images for senders in `images_from` (`show_images_from`)
- [x] [mobile] Untrusted text: every string from mail goes through `app_front::text` before it enters a frame. Test
  like `app-tui/tests/it/untrusted.rs`: ESC, C1 controls, `\r` and U+202E in subjects, names, addresses, bodies, link
  texts and attachment names leave none of them in any frame, at every window class
- [x] [mobile] [kt] Search and the palette: the top bar's search opens the palette (commands, mailboxes, recent
  threads, *Search mail for …*), like the desktop's search pill; second-step pickers from `app_front::palette`; query
  results in a read-only view marked as untrusted; search results with highlights and the server search's state; the
  parity walk reaches every registry command by taps

### A4 — Compose & send
- [x] [mobile] [kt] Composer: From (identities), To/Cc/Bcc chips (`parse_addr`) with contact suggestions
  (`list_contacts`), subject, the body in a native text field (any IME), the quote read-only and folded; typing goes
  back as one `update_draft` per pause, field change or command; a change from elsewhere reloads a field unless it has
  unsaved typing; *Discard*
- [x] [kt] [mobile] Attachments in: the system file picker; each file copied into `filesDir/outgoing/<uuid>/`
  (`safe_file_name`, 25 MiB) and attached by its path; chips with checking, size or error; copies that no draft names
  are removed at start
- [x] [mobile] [kt] Send with the undo window (*Sending in N s · Undo*), `cancel_send`; a foreground service from the
  scheduled send to `MessageSent` or `SendFailed` (`shortService` from Android 14, `dataSync` before); *Not sent* once
  for a send that a restart cancelled
- [x] [kt] `mailto:` links and the share sheet (text, files) open a new draft in the composer — never a send;
  `file://` URIs and the app's own content URIs refused, shared files copied like picked ones (tests)

### A5 — Background & notifications
- [x] [core] Settings `background_sync` (0 while open, 1 periodic, 2 push; default 1) and `background_interval_mins`
  (15–720, default 30) in `config.toml` and `set_setting`, read only by the Android host; replay fixture;
  ARCHITECTURE §8
- [x] [android] [kt] Leaving the app: a one-off job flushes queued triage, draft saves and attachment saves (at most
  3 min); a periodic job (network required, kept across reboots) runs `sync_now` as `runtime` and waits for the
  accounts; a network change while the app is visible syncs
- [x] [kt] [front] Notifications from `app_front::alerts` while no activity is visible (alerts carry their thread or
  draft), through the host's alert watcher and `Listener.alert` (called from an attached runtime thread): channels *New mail*, *Not sent* and *Background*; `POST_NOTIFICATIONS` asked when first needed; lock-screen
  versions without sender or subject; *Archive* and *Mark read* actions; a tap opens the thread
- [x] [kt] [android] Push: a `specialUse` foreground service (`dataSync` before Android 14) while `background_sync` is
  2, around the engine's IMAP IDLE watches, with a silent notification (*Watching N inboxes*, *Stop*); started from
  Settings and after a reboot; a link to the battery-optimization screen; the periodic job stays the fallback

### A6 — Settings, setup & keyboards
- [x] [mobile] [kt] Settings (`open_settings`): Appearance, Keyboard, General, Accounts, Background & notifications;
  account setup (preset → address → password with autofill hints → `test_account` → `sync_now`; *Find servers* for
  other domains; *re-enter password* from a problem's fix)
- [x] [mobile] [kt] Hardware keyboards: Android key events → `app_front::keymap` chords, so the desktop's keys work on
  tablets and Chromebooks; a test that every bound command has a key Android delivers

### A7 — Release & devices
- [x] Release: a signed APK per ABI from `release.yml` with a `.sha256` each (the key from repository secrets, never in
  the repository); the version code from the workspace version; R8 keep rules for the JNI and serialization classes;
  the size measured; an About screen with the licences of the fonts, the Rust crates and the Android libraries
- [ ] Device check: a Pixel, a Samsung (One UI's battery limits), a tablet or foldable, a Chromebook if at hand — Gboard
  swipe and CJK input, TalkBack, 200 % font scale, dark mode, rotation and process death, a night of push and of
  periodic checks under Doze, battery use, memory with a large cache. What works and what does not goes into
  ARCHITECTURE §7.5; adjust the breakpoints and defaults

### Stretch
- [ ] Home-screen widget (unread count) and app shortcuts (Compose, Search)
- [ ] `dail` for Termux (`aarch64-linux-android`; app-cli with the GUI made optional): the TUI in the phone's terminal.
  Termux has no keyring, so it runs a demo on the memory backend until a secret store exists there

---

## Wave 5 — Open repository and iOS: "dail on the user's iPad, built without a Mac"

Make the repository public, so that GitHub's macOS runners are free. Then run dail on iOS/iPadOS as a SwiftUI app that
draws `app-mobile`'s frames through a C ABI. This is Android's pattern (ADR 0009) with Swift instead of Kotlin. There
is no local Mac: Xcode, Swift, linking, the simulator and the IPA all run on `macos-latest` runners, and the Rust side
is checked on Linux with clippy for `aarch64-apple-ios`. The plan, the options, the crate split and the signing routes
are in [`docs/plans/2026-10-24-ios.md`](docs/plans/2026-10-24-ios.md). ADR 0010 (item I0.1) records them. Every item
updates `AGENTS.md`/`ARCHITECTURE.md` where it touches them (plan §7).

**Exit criteria**
- The repository is public, with a licence, a README and hardened workflows, and no secret anywhere in its history.
- `ci.yml` runs an `ios` job (XCFramework, simulator build, XCTest over the contract fixtures), and it is green.
- `release.yml` makes an unsigned IPA (demo and live) with a `.sha256`.
- The user has re-signed and installed it with a free Apple ID, and on their iPad has read the demo mailbox and a live
  account, triaged with undo, and reached every command from the palette.

### O — Open the repository (plan §1)
- [x] Secret scan of the whole history (gitleaks, trufflehog). Rotate and rewrite only if the user decides to
- [x] Review what strangers will see: `docs/requests`, `docs/reviews`, fixtures (real addresses and hosts)
- [x] LICENSE (the user chooses: MIT OR Apache-2.0), with `license.workspace = true` in every crate and `deny.toml` checked;
  README.md, SECURITY.md, CONTRIBUTING.md
- [x] Workflow hardening: `permissions: contents: read`, actions pinned to SHAs, Dependabot for actions, release jobs
  on `workflow_dispatch` only
- [x] The user sets the repository settings (fork-PR approval, branch protection, secret scanning and push
  protection, a `release` environment) and makes the repository public. `ci.yml` is green on the public runners

### I0 — Groundwork (no iOS code yet; desktop and Android unchanged)
- [x] ADR 0010: an iOS host over `app-mobile` through a C ABI, `app-host` split out, the lint exception, no-Mac CI.
  Ask the user for the iPad's model and iPadOS version (deployment target: iOS 17 unless it is older; the user: 17+)
- [x] `app-host`: move `host.rs`, `calls.rs`, `config.rs`, `outgoing.rs` and `error.rs` out of `app-android` as they
  are, with their tests. app-android keeps only JNI, Keystore and logcat. `cargo xtask verify` and
  `verify android` stay green

### I1 — The Rust side (no Swift yet)
- [x] Portability: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`; clippy over app-host's tree for both.
  `cargo tree -i cc` is empty, `cargo deny` passes, and the iOS keyring store is chosen (or the Swift Keychain
  fallback, plan §3)
- [x] `app-ios`: staticlib `libdail.a`, the six calls plus two frees in `src/ffi.rs` (the only unsafe module, pinned
  lints), `include/dail.h` with a test that matches it against the exports, the callback for alerts and wakes,
  `demo` feature, os_log or file logging. Tests on Linux
- [x] `cargo xtask ios check` (runs on Linux) and `ios xcframework` (macOS; otherwise "cannot run")

### I2 — CI on macOS
- [x] `ios/project.yml` (XcodeGen), an empty SwiftUI app linking `DailCore.xcframework`, and `Native.swift` opening the
  demo host and showing the first frame's title. The `ios` job in `ci.yml` (path-filtered, `macos-latest`) builds it
  for the simulator
- [x] `DailTests`: decode every contract fixture strictly; open the demo host in the simulator and take a frame
- [ ] `cargo xtask ios app`: the unsigned archive packed as `dail-<v>[-demo]-unsigned.ipa`. A `release.yml` `ios` job
  uploads both with `.sha256`. AGENTS.md §6 gains the `ios` gate

### I3 — The app
- [ ] Mailbox list, thread list, message view (text, remote images off), mark read/star/archive with undo, through
  frames and `UiEvent`s only
- [ ] The palette, with a parity test that walks every command, and account setup and settings through the palette
  flows; Keychain secrets in a live session
- [ ] Measure memory, redb and tantivy in the simulator on a large demo cache (plan §9)

### I4 — On the device
- [ ] The user installs the demo IPA, then the live one, on their iPad (free Apple ID re-signing, Developer Mode).
  The steps that worked go into ARCHITECTURE §7.6
- [ ] Device check: VoiceOver, Dynamic Type, dark mode, rotation, a hardware keyboard, being killed in the background
- [ ] (Optional, paid account) A signed ad-hoc or TestFlight build from `release.yml`: the certificate and profile from
  repository secrets in a temporary keychain (plan §6)

---

## Roadmap — future waves (unordered within themes)

**Accounts & protocols**
- OAuth2 XOAUTH2 for Gmail and Microsoft 365 (loopback redirect flow, token refresh in keyring)
- TLS tiers 2 and 3 in `app-tls`: AES-GCM (Namecheap Private Email) and P-256/P-384 key exchange (Microsoft 365,
  T-Online), with the Microsoft 365 item (`docs/plan-tls.md` §11)
- Native Gmail labels (`X-GM-LABELS`, `X-GM-THRID`); optional Gmail API backend
- JMAP backend (Fastmail, Stalwart) with push
- Exchange/M365 via Graph API

**Automation (ArtCraft dividends)**
- Rules engine: predicate + stored command batch, run on `EnvelopesFetched` (a server-side-filter alternative)
- Macros: record a slice of the command log, replay with parameters; bind to keys/palette
- Snooze, scheduled send, follow-up reminders (all `Ctx`-timed, deterministic)
- Templates/snippets with variables

**AI / agent features** (bring-your-own model through MCP; nothing hard-wired)
- Thread summaries, triage suggestions (archive/label/reply-needed) shown as reviewable proposals
- Reply drafting via MCP sampling; the user edits and sends
- Natural-language search → query language translation
- Agent-proposed rules ("you always archive X") produced as a reviewable rule draft
- Compact MCP tool profiles for small models: a few tools, short descriptions, small results (on-device Gemma 4 gets
  4k–10k tokens of context, dail's full tool list is ~22k; `docs/plans/2026-10-22-android.md` §4.7)
- Cross-device approvals: approve the desktop agent's send requests from the phone (the request travels with the
  draft's server copy in Drafts; needs its own design and threat review)

**Security & trust**
- OpenPGP sign/encrypt/verify (`pgp` crate, pure Rust); Autocrypt
- S/MIME
- DKIM/SPF/DMARC/ARC verdict display (`mail-auth`), lookalike-domain and phishing warnings
- One-click unsubscribe (RFC 8058 `List-Unsubscribe-Post`)
- At-rest encryption of the local cache (key in keyring)

**Ecosystem**
- ICS invites: parse, render, RSVP; CalDAV calendar view
- CardDAV contacts sync
- Import/export: mbox, Maildir, `.eml`; migrate from Thunderbird
- Plugin API over the command bus (out-of-process, JSON-RPC)

**Platforms**
- macOS build (Keychain via `keyring`)
- iOS parity after Wave 5: compose with attachments, background refresh, notifications, iPad multi-column, keyboard
  shortcuts, Share sheet and Files, the App Store
- Android releases on Google Play (an AAB, the `specialUse` declaration, the data safety form) and F-Droid (a build
  recipe with Rust and the NDK, reproducible builds)
- Android agent channel: AppFunctions for the system assistant once Gemini's integration opens; MCP on the phone only
  once a mobile MCP client can use a compact tool profile
- `dail serve`: headless daemon exposing the command bus over WebSocket JSON-RPC + HTTP MCP
- Web/PWA client in the browser on top of `dail serve` (revisit egui-wasm vs a DOM UI such as Dioxus then; restore the wasm gate for `app-core`)
- Direct-JMAP serverless web mode
