# dail — Architecture

Agent-first desktop email client. Every action is a serializable `Command`. The GUI, the CLI, batch scripts and the
built-in MCP server are four equal front-ends to one deterministic engine.

- Rules for contributors and agents: [`AGENTS.md`](AGENTS.md)
- Delivery plan: [`ROADMAP.md`](ROADMAP.md)

---

## 1. Principles

1. **Headless first.** If an action can't be done from `dail exec`, it doesn't exist yet.
2. **Sans-IO core.** `app-core` is a pure state machine: `(State, Command|Event, Ctx) → (State', Receipt, Effects)`.
3. **Local-first, optimistic.** The UI reflects intent immediately. The network catches up through a pending-op queue.
4. **One registry, many surfaces.** Palette, CLI help, MCP tools and keybindings all come from `Command::specs()`.
5. **Replayable.** Commands and events are logged as JSONL. Bugs are reproduced by replaying a log, not by clicking around.
6. **Email is hostile input.** Content never gains authority over the agent or the app.
7. **Minimal UI.** Three panes, one accent colour, keyboard everything, Ctrl+K for the rest.

## 2. Scope (current)

| | Now | Later |
|---|---|---|
| Platforms | Ubuntu (X11/Wayland), Windows, Android 10+ (ADR 0009); iOS/iPadOS 17+ in progress (Wave 5, ADR 0010) | macOS, Web/PWA |
| Protocols | IMAP4rev1 (+UIDPLUS, MOVE, SPECIAL-USE if offered), SMTP submission | IDLE/CONDSTORE/QRESYNC, JMAP, Gmail API |
| Auth | Password / App Password (keyring-stored) | OAuth2 XOAUTH2 (Gmail, M365) |
| Providers | Hostinger Mail, Gmail, generic custom with HTTPS autoconfig (ADR 0006) | DNS SRV discovery |

## 3. Workspace topology

```mermaid
graph LR
  core[app-core<br/>pure state machine]
  io[app-io<br/>MIME · config · store]
  sync[app-sync<br/>Session · IMAP · SMTP · IPC]
  mcp[app-mcp<br/>MCP server]
  gui[app-gui<br/>egui]
  front[app-front<br/>palette · keymap · projections]
  tui[app-tui<br/>ratatui]
  cli[app-cli<br/>`dail` binary]
  tk[app-testkit<br/>fake transport]
  tls[app-tls<br/>TLS crypto provider]
  theme[app-theme<br/>design tokens]
  mobile[app-mobile<br/>touch presenter]
  host[app-host<br/>mobile host · six calls]
  android[app-android<br/>Android host · JNI]
  ios[app-ios<br/>iOS host · C ABI]

  io --> core
  sync --> core
  sync --> io
  sync --> tls
  mcp --> sync
  mcp --> core
  gui --> sync
  gui --> core
  gui --> front
  gui --> theme
  tui --> sync
  tui --> core
  tui --> front
  tui --> theme
  front --> core
  cli --> mcp
  cli --> gui
  cli --> tui
  cli --> sync
  cli --> io
  cli -.ADR 0001.-> tk
  tk -.dev.-> sync
  mobile --> core
  mobile --> front
  mobile --> theme
  host --> mobile
  host --> sync
  host --> io
  host --> core
  android --> host
  ios --> host
  android --> sync
  android --> io
  android --> core
  android -.demo, ADR 0001.-> tk
```

There is a single binary, `dail` (`app-cli`). It hosts every mode:

```text
dail gui                         # start desktop app (default when no subcommand)
dail tui                         # terminal UI (ADR 0004); attaches to a running owner of the profile
                                 # (plain `dail` on Linux without a display, in a terminal, too)
dail exec archive_threads --args '{"ids":["…"]}'
dail batch script.jsonl          # or: … | dail batch -
dail schema [--command NAME]     # JSON Schema of Command/Event (what MCP exposes)
dail replay <log> [--state]      # replay an audit log offline through a fresh engine
dail doctor                      # check the profile: config, store, backups, keyring, TLS
dail mcp [--allow-send] [--http 127.0.0.1:PORT]   # MCP over stdio (or HTTP on loopback, ADR 0007); attaches to a running GUI
dail account add --preset gmail --email me@gmail.com   # alias for exec add_account
dail account password --email me@gmail.com           # alias for exec set_password
```

`app-front` (ADR 0004) is the toolkit-free half of the front-ends: palette flows and ranking, the keymap, formatting,
the status and approval projections and the body tracking. It depends on `app-core` only (no toolkit, no tokio), so the
GUI and the TUI share these rules and neither re-derives the other's. `app-gui` and `app-tui` don't depend on each
other. Modules: `format` (dates in UTC, addresses, sizes), `body` (the reader's body tracking: which
`get_thread {include_body: true}` to submit — `Bodies::rich()` adds `rich_body` for the GUI —, the bodies taken from its
receipt with their `document`, `links`), `status` (sync text, problems,
pending ops), `approvals` (the cards of agents' send requests), `compose` (`parse_addr`, recipient fields, the draft's
status, `sendable`), `fold` (`split`: a plain-text body's own text, `-- ` signature and trailing quoted part —
`>` lines with their attribution, or an Outlook header block on), `mailbox` (labels), `notice` (the latest undo label, the send countdown, rejection texts),
`keymap` (chords, bindings, `resolve`, the sequence state) and `palette` (the flows, rows, `nucleo-matcher` ranking
and `Palette`, the state of an open palette without drawing: query, cursor, second step; `choose` returns the
commands to submit or keeps the next step), `text` (the sanitizer for untrusted text: `is_unsafe`, `clean`,
`clean_lines`; control and bidi characters become U+FFFD), `a11y` (the accessible names of thread and nav rows, read
by AccessKit and TalkBack) and `panes` (which panes show for a width class: `Slot`, `Width`, `choose`; the TUI maps
columns onto it). `app_io::images` decodes untrusted images with the shared limits (PNG, JPEG, GIF, WebP; larger than 4096 px a
side decoded downscaled to fit, JPEGs DCT-scaled by `jpeg-decoder`; 96 MiB; upright by EXIF orientation) into RGBA; `app_io::pdf` rasterizes untrusted PDF pages with `hayro` (ADR 0012): `pages` (sizes in points as shown) and
`render(data, page, width, tile)` (a whole page within 4096 px a side, or a tile of at most 1024 px cut from a page at
up to 16384 px wide) into the same RGBA, within 32 MiB and 1000 pages, under `catch_unwind`. `app-sync` re-exports
`images` as `app_sync::images` for the GUI, which does not depend on `app-io`.

`app-mobile` and `app-android` (ADR 0009, Wave 4) are the Android front-end. `app-mobile` (← app-core, app-front,
app-theme; no toolkit, no tokio, no JNI) turns `AppState` into JSON screen models ("frames") and touch events into
commands, as `app-tui` turns it into a terminal buffer. `app-host` (← app-core, app-io, app-sync, app-mobile;
ADR 0010) hosts the session behind six calls, JSON in and out, with no platform code; it is shared with the iOS host.
`app-android` (← app-host, app-core, app-io, app-sync; cdylib `libdail.so`) adds the JNI exports, the Keystore store
and logcat for the Kotlin/Compose app in `android/`, which draws the frames and holds no rules. `app-ios` (← app-host,
app-core, app-io, app-sync; staticlib `libdail.a`, §7.6) does the same for the SwiftUI app through a C ABI. None of these crates
is a dependency of app-gui, app-tui or app-cli.

`app-theme` (ADR 0008) holds the design tokens without a toolkit: colour roles in two surface groups (content and
chrome), the type scale, the metrics, the built-in themes (`dail-dark`, `dail-light`, `dail-dark-hc`,
`dail-light-hc`) and the WCAG contrast `audit`, a unit test over every built-in. It depends on `serde` only. It also
bundles the fonts (`fonts`: Geist and JetBrains Mono, variable in `wght`, with their OFL texts). `app-gui` registers
each font once per weight (400, 500, 600) as the families `sans`, `sans-500`, `sans-600`, `mono`, `mono-500` and
`mono-600`, with egui's own fonts as fallbacks, and maps egui's `TextStyle`s onto the type scale. The icons are a
Material Symbols Outlined subset (`icon`: `Icon` names the glyphs; families `icons` and `icons-filled`). The GUI paints
them (`ui::icon`), so screen readers never read a private-use character; the widget around carries the name.
Mailbox roles map onto icons in `theme::role_icon`, an exhaustive match. Emoji in mail still come from egui's emoji
fonts. Scripts that none of these fonts cover (CJK, Arabic, Devanagari, …) load from the OS fonts on demand (eframe's
`system_fonts`: `egui_system_font_provider` over `fontique`, which opens fontconfig at run time on Linux and uses
DirectWrite on Windows; UI-15).

## 4. Core model (`app-core`)

### 4.1 Data flow

```mermaid
sequenceDiagram
  participant FE as Front-end (GUI / CLI / MCP / batch)
  participant S as Session (app-sync)
  participant E as Engine (app-core)
  participant X as Effect executor (IMAP/SMTP/Store)

  FE->>S: submit(Command, Origin)
  S->>E: dispatch(cmd, Ctx{now_ms, id_seed, origin})
  E-->>S: Outcome{receipt, effects}
  S-->>FE: receipt (JSON)
  S->>X: run(effects)  [async, retried]
  X-->>S: Event (EnvelopesFetched, OpFailed…)
  S->>E: ingest(event, ctx)
  E-->>S: Outcome{receipt, effects}
  S-->>FE: StateChanged notification
```

`Session` is the only place that owns an `Engine`. It is single-writer: commands and events are serialized through
one `tokio::mpsc` queue, so the engine needs no locks.

### 4.2 Types

```rust
// ids.rs — newtypes over Uuid / u64; all Copy + Ord + Serialize + JsonSchema
AccountId, MailboxId, MessageId, ThreadId, DraftId, OpId, ApprovalId

// state.rs
pub struct AppState {
    pub accounts:  IndexMap<AccountId, Account>,
    pub mailboxes: IndexMap<MailboxId, Mailbox>,
    pub messages:  IndexMap<MessageId, Envelope>,     // metadata only; bodies via Store
    pub threads:   IndexMap<ThreadId, Thread>,
    pub drafts:    IndexMap<DraftId, Draft>,
    pub outbox:    IndexMap<OpId, PendingOp>,          // remote ops not yet confirmed
    pub approvals: IndexMap<ApprovalId, ApprovalRequest>, // agent sends awaiting the user
    pub view:      ViewState,                          // palette_open, mailbox, selected threads, checked threads, open thread,
                                                       // expanded: {MessageId: bool} (reader overrides), search,
                                                       // composer (open draft), focus: Pane, sidebar_collapsed
    pub settings:  Settings,
    pub status:    StatusState,                        // runtime health: persist_error, keyring_error, sync (per account)
    pub update:    UpdateState,                        // updates of dail (Wave 7, update.rs): host {version, target, install},
                                                       // status: idle|checking|available|downloading|ready|installing|installed|failed,
                                                       // last_check_ms, next_check_ms; the policy is settings.updates
    pub revision:  Revision,                           // counts engine inputs; not serialized, always compares equal:
                                                       // front-ends keep projections until it moves
}

pub struct Account  { id, display_name, email, preset: ProviderPreset, imap: Endpoint, smtp: Endpoint,
                      secret: SecretRef, status: AccountStatus /* Unverified|Verified{at_ms}|Failed{error,at_ms} */,
                      identities: Vec<Identity { email, name?, signature? }> /* aliases; the own address's name/signature */ }
pub struct Endpoint { host, port: u16, security: Security /* Tls|StartTls — no plaintext */ }
pub struct SecretRef { service: "dail", user: "account/<id>" }   // keyring entry; the secret is never in state
pub struct Secret(String) // write-only command/effect field: Debug and Serialize print "<redacted>"
pub struct ImapLogin { login, server: Endpoint, secret: SecretRef }  // carried by mail effects; Account::imap_login()
pub struct SmtpLogin { login, server: Endpoint, secret: SecretRef }  // carried by SmtpSend; Account::smtp_login()
pub struct Mailbox  { id, account, path: String, delimiter: Option<String>, role: Option<MailboxRole>,
                      uid_validity: Option<u32>, uid_next: Option<u32>, unread: u32, total: u32,
                      highest_modseq: Option<u64> /* CONDSTORE, as of the last flag resync */,
                      sync_window: Option<u32> /* newest messages to cache; 0 = all; None = settings */,
                      history_complete: bool /* the oldest server message is cached */,
                      last_fetch_ms: Option<i64> /* UX-6 */,
                      custom_keywords: Option<bool> /* Some(false): keeps no keywords, pins stay local (MISC-4) */ }
pub struct RemoteMailbox { path, delimiter, special_use: Option<MailboxRole>, selectable } // as LISTed
pub struct SyncStatus { pending: u32 /* sync effects in flight */, last_sync_ms: Option<i64>,
                        last_error: Option<SyncIssue { error, mailbox: Option<MailboxId>, at_ms }>,
                        watch: WatchState /* off|watching|retrying{at_ms, error}|unsupported: IMAP IDLE, §5.2 */ }
// StatusState { persist_error, keyring_error, sync: IndexMap<AccountId, SyncStatus>, next_poll_ms: Option<i64>,
//               bodies_in_flight: BTreeSet<MessageId>, drafts_saving: BTreeSet<DraftId>,
//               searches: Vec<ServerSearch { query /* key */, account, pending, hits: Vec<MessageId>, last_error, updated_ms }>,
//               backfilling: BTreeSet<MailboxId> /* a FetchOlder in flight */,
//               index_searches: Vec<IndexSearch { query, hits, error, updated_ms }> /* full-text, at most 8 */,
//               compaction: Option<Compaction /* running{started_ms}|done{at_ms, removed_blobs, freed_bytes}|failed{at_ms, error} */>,
//               saves: Vec<AttachmentSave { message, dir, state: saving|saved{files}|failed{files, error}, updated_ms }> }
pub enum  MailboxRole { Inbox, Sent, Drafts, Trash, Archive, Junk, All, Flagged }
pub struct ApprovalRequest { id, draft: DraftId, account, origin: Origin, requested_ms }  // an agent's send, §4.6
pub enum  Origin { Gui, Tui /* dail tui, ADR 0004 */, Cli, Batch, Mcp, McpAllowSend /* dail mcp --allow-send */, Runtime }
pub struct RemoteEnvelope { uid, message_id_hdr, in_reply_to, references, from, to, cc, subject, date_ms, flags, size,
                      has_attachments }  // as fetched; into_envelope() adds id/account/mailbox/thread
pub struct Envelope { id, account, mailbox, uid: u32, message_id_hdr: Option<String>, in_reply_to, references,
                      thread, from: Vec<Addr>, to, cc, subject: String, date_ms: i64, flags: Flags,
                      size: u32, has_attachments: bool, body: BodyState /* NotFetched|Fetched{blob}|Failed */,
                      snippet: Option<String>,      // ≤ 140 chars of the text, set from BodyFetched (UI-14)
                      moved_to: Option<MailboxId> }  // mailbox/uid = server location (uid 0: placeholder); shown in
                                                     // location() = moved_to.unwrap_or(mailbox)
pub struct Thread   { id, account, messages: Vec<MessageId> /* oldest first */, last_date_ms: i64, subject: String }
pub struct Addr     { name: Option<String>, email }   pub struct Flags { seen, flagged, answered, draft,
                      pinned /* the keyword `$Pinned`, MISC-4 */ }
pub struct Draft    { id, account, reply_to: Option<MessageId>, message_id_hdr: String /* stable, from the draft ID */,
                      in_reply_to, references, to, cc, bcc: Vec<Addr>, subject, body_text,
                      quote: Option<Quote { message, style: Reply|Forward, header: String }>,
                      created_ms, updated_ms, saved_ms: Option<i64> /* updated_ms of the server copy */,
                      save_error: Option<String>, attachments: Vec<AttachmentRef>,
                      send_at: Option<i64> /* scheduled send */, sending: bool, send_error: Option<String> }
pub struct AttachmentRef { path /* absolute */, name /* last path component */,
                      state: Checking|Ready{size, content_type}|Failed{error} }   // file read by the runtime
pub struct PendingOp{ id, account, op: RemoteOp, state: OpState /* Queued|Running|Failed */, attempts: u32,
                      last_error: Option<String>, created_ms, next_attempt_ms: Option<i64>, awaiting_sync: bool }
pub enum  RemoteOp  { SetFlag { flag: FlagKind /* Seen|Flagged */, value: bool, messages: Vec<MessageId> },
                      Move { to: MailboxId, messages }, Copy { to, copies: Vec<CopyRef { source, copy }> },
                      FlagMailbox { mailbox, flag, value } /* every message on the server, UX-7 */ }
pub enum  RemoteStep{ Store { path, uid_validity, uids, flag, value },     // resolved when the op starts
                      Move { path, uid_validity, uids, to: String, create }, Copy { … same … },
                      StoreAll { path, uid_validity, flag, value } /* UID STORE 1:* */ }
```

Folder tree (UX-2): `app_front::mailbox::tree(state, account) -> Vec<FolderNode { key: FolderKey { account, path },
label, depth, mailbox: Option<MailboxId>, role, children, unread, unread_below }>` is shared by the GUI, the TUI and the
sidebar keys. Role folders come first, flat, in role order; the others nest by `path.split(delimiter)`, sorted
case-insensitively. A parent that is not cached (`\Noselect`, never listed) is a node without a mailbox (it can't be
opened); since role folders never become such parents, Gmail's `[Gmail]` vanishes by itself. The inbox's children go to
the top level (servers such as Dovecot put every folder below `INBOX.`); the other role folders keep theirs.
`unread_below` sums the folders below. `app_front::mailbox::visible_rows` drops what `view.collapsed` hides (UX-3):
each account caption shows its sync state (`app_front::status::account_state`, UX-9: a busy dot while it syncs, a
failed dot with the cleaned error on hover, `offline` while changes wait after failed attempts; the TUI writes `⟳`, `⚠`,
`offline`); a folder row's context menu offers *New subfolder…*, *Rename…*, *Pin to Favourites*/*Unpin*, *Hide folder*/*Show
folder*, *Focus this mailbox*, *Mark all as read* and *Delete…* (the steps that ask go through the palette:
`palette::new_subfolder`, `rename_folder`, `delete_folder`; UX-14); the GUI draws a chevron on parents and account captions (click: fold; a click on a parent that can't be opened folds
it too), a collapsed row shows `unread_below` faintly, and a collapsed account still shows the open mailbox. `Mailbox::parent_path()` / `depth()` give agents the same hierarchy in
`list_mailboxes`.

Mailbox roles come from `mailbox::assign_roles`: `INBOX` is always the inbox, then the SPECIAL-USE attribute, then the
preset's folder names (`ProviderPreset::folder_roles`, e.g. `[Gmail]/Sent Mail`, `INBOX.Sent`), then name heuristics on
the last path segment (`Sent Items`, `Deleted Messages`, `Spam`, …). Each role goes to at most one mailbox. `\Noselect`
entries are not cached. On `MailboxesListed` the engine keeps the IDs (and sync position) of known paths, gives new
paths IDs from `Ctx::id_seed`, drops vanished mailboxes and clears the account's `SyncStatus.last_error`.
`SyncStatus.pending` counts the account's `ListMailboxes`/`FetchEnvelopes` in flight (incremented when the engine emits them,
decremented by `MailboxesListed`, `EnvelopesFetched` or `SyncFailed`); when it drops to 0, `last_sync_ms` is set.
`remove_account` also drops the account's cached mailboxes. Whenever the cache drops a mailbox or thread, `ViewState` forgets it too (open
mailbox, selection, open thread).

Identities: a new draft is sent from the identity the answered message was addressed to (`To`/`Cc`), else the
mailbox's (`Mailbox.identity`: the open mailbox for `create_draft`, the original's for replies), else the account's own
address; an alias sets `Draft.from` (name: the identity's, else the account's). That identity's signature is added below
the draft's text after a `-- ` line. `outgoing()` sends `From` the draft's address. The approval card shows it.
Drafts are local-first: every draft command changes `AppState.drafts` and persists it at once (`MailDelta.drafts`).
`reply`/`reply_all`/`forward` fill recipients (`draft::reply_recipients`: the sender, or the recipients of one's own
message; reply-all adds `To`/`Cc`; the account's own address and duplicates are dropped), a single `Re:`/`Fwd:` prefix,
`In-Reply-To` (replies) and `References` (original's references + its `Message-ID`). The core can't read bodies, so the
draft keeps a `Quote` (the original message and its attribution line or forward header block, dates as
`YYYY-MM-DD HH:MM UTC`); the runtime puts the original's text below the draft's own when it renders it, and replying
starts the original's body download if it isn't cached. Undo of `discard_draft` is `restore_draft{draft}`, which puts the
same draft (same ID) back with every attachment `checking` again (`InspectAttachment`: a check is only taken from the
runtime, and the file may have changed). A draft's `Message-ID`, `In-Reply-To` and `References` hold only IDs without
whitespace or control characters (`draft::is_message_id`): a reply leaves out the original's IDs that aren't, and
`restore_draft` refuses them. `attach_file` records only an absolute path (Unix or Windows form) and emits
`InspectAttachment{draft, path}`; the runtime checks that it is a readable regular file and reports `AttachmentInspected{size,
content_type}` (type guessed from the extension, `app_io::mime::content_type_for`) or `AttachmentFailed{error}`. A file that
would take the draft's attachments over `draft::MAX_ATTACHMENT_BYTES` (25 MiB) becomes `failed`. The file is read again
whenever the draft is rendered. Undo of `remove_attachment` attaches the path again at its old position (and re-checks it).

Gmail exposes the same message in several folders. `MessageId` is per `(mailbox, uid)`. Threads group messages
across folders by `Message-ID`/`References` (a JWZ-lite threading algorithm, pure, in `app-core::threading`), per account.
`Threader` indexes the cached messages of an account (the engine keeps one per account between merges, with the
`index_epoch` it was built at; removing a message or adding a placeholder bumps the epoch and the next merge builds it
afresh, so a kept index always equals a new one: `threading::the_kept_threading_index_matches_a_fresh_one`); `place(key)` returns `New`, `Join(thread)` or
`Merge{into, from}`. A message joins the thread of a message with the same `Message-ID` (a copy in another folder), of an
ancestor named in its `In-Reply-To`/`References`, of a cached reply naming it, or of a sibling sharing an ancestor; if
these are several threads they merge. Without links, a subject with a reply/forward prefix (`Re:`, `Fwd:`, `AW:`, `SV:`, …,
`Re[2]:`) joins the latest thread with the same normalized subject if it was active within 30 days. Threads never split.

### 4.3 Command / Event / Effect

- **`Command`** — intent from a front-end (§8 catalogue). `#[serde(tag = "cmd", content = "args", rename_all = "snake_case")]`.
- **`Event`** — a world fact, produced only by the runtime: `AccountVerified`, `AccountVerifyFailed`,
  `SecretStored`, `SecretDeleted`, `SecretFailed`, `MailboxesListed{account, mailboxes}`,
  `EnvelopesFetched{account, mailbox, uid_validity, uid_next?, total, unread, envelopes: Vec<RemoteEnvelope>}`,
  `BodyFetched{message, blob, has_attachments, snippet?}`, `BodyFetchFailed{message, error}`,
  `FlagsChanged{account, mailbox, uid_validity, from_uid, to_uid, flags: Vec<UidFlags>, existing?, highest_modseq?}`, `RemoteOpSucceeded{op}`, `RemoteOpFailed{op, error, permanent}`,
  `AttachmentInspected{draft, path, size, content_type}`, `AttachmentFailed{draft, path, error}`,
  `AttachmentsSaved{message, files}`, `AttachmentsSaveFailed{message, files, error}`,
  `MailboxWatched{account, mailbox, changed}`, `WatchFailed{account, mailbox, error, unsupported}`,
  `OlderFetched{account, mailbox, uid_validity, envelopes, complete}`, `BackfillFailed{account, mailbox, error}`,
  `StoreCompacted{removed_blobs, freed_bytes}`, `CompactFailed{error}`, `IndexSearched{search, hits}`,
  `IndexSearchFailed{search, error}`, `ServersDiscovered{email, imap, smtp, source}`, `DiscoveryFailed{email, error}`, `Closing`,
  `DraftSaved{draft, revision}`, `DraftSaveFailed{draft, error}`, `DraftDeleted{draft}`, `DraftDeleteFailed{account, draft, error}`, `MessageSent{draft, sent_copy_error?}`, `SendFailed{draft, error}`, `SyncFailed{account, mailbox?, error}`, `Tick{now_ms}`,
  `SearchResults{account, mailbox, uid_validity, search, started_ms, mailboxes, envelopes}`, `SearchFailed{account, mailbox, search, started_ms, mailboxes, error}`,
  `KeywordsRefused{account, path}` (MISC-4), `FolderManaged{account, action}`, `FolderFailed{account, action, error}` (both list the account's folders again; a
  failure stays in `SyncStatus.folder_error` until a folder change succeeds — listings don't clear it), `Persisted`, `PersistFailed{error}`. JSON form: `{"event": "<name>", "args": {…}}`.
- **`Effect`** — a request to the world: `VerifyAccount{account, login, imap, smtp, secret}`, `ListMailboxes{account, imap: ImapLogin}`, `FetchEnvelopes{account, mailbox, path, imap, uid_validity?, since_uid?, limit}`,
  `ResyncFlags{account, mailbox, path, imap, uid_validity, from_uid, to_uid, changed_since?}`,
  `FetchBody{account, message, path, imap, uid_validity, uid}`, `RunRemoteOp{op, account, imap, steps: Vec<RemoteStep>}`, `InspectAttachment{draft, path}`, `SaveAttachments{message, blob, parts, dir}`, `SaveDraft{draft, account, imap, path, revision, message: OutgoingMessage}`,
  `DeleteDraft{draft, account, imap, path, message_id}`, `SearchMailbox{account, mailbox, path, imap, search, started_ms, mailboxes, query: Query, gmail_raw, limit}`,
  `SmtpSend{draft, account, smtp: SmtpLogin, message: OutgoingMessage,
  sent_copy?: SentCopy{imap, path}}`, `StoreSecret{account, secret, value: Secret}`,
  `DeleteSecret{account, secret}`, `PersistState{delta}`, `ScheduleTick{at_ms}`, `WatchMailbox{account, mailbox, path, imap}`,
  `FetchOlder{account, mailbox, path, imap, uid_validity, before_uid, limit}`, `CompactStore{keep}`, `SearchIndex{search, words, limit}`, `DiscoverServers{email, domain}`, `ManageFolder{account, imap, action: create{path}|rename{from, to}|delete{path}}`. JSON form: `{"effect": "<name>", "args": {…}}`.
  `Effect::failure_event` maps a failed effect to the `Event` that records the failure in state.
- **`CommandReceipt`** — `{ ok, name, kind: Option<CommandKind>, affected: Vec<EntityRef>, data: serde_json::Value, undo_label: Option<String> }`.
  Query results travel in `data`. `kind` is `null` for receipts of ingested events. Rejected inputs return a
  `CoreError` instead and leave state untouched.
- **`Preview`** (`Engine::preview`, dry runs): the command dispatched on a clone of the engine — the receipt's fields,
  `dry_run: true`, `would_run` (effect names only: their arguments can hold message text) and `changes`: per affected
  entity the top-level fields that differ, `{entity, untrusted_content: {before, after}}` (`before: null` for a new
  entity, `after: null` for a removed one). Same validation and agent policy as `dispatch`; nothing changes, runs, is
  recorded in history or written to the audit log. Sessions run it in their loop (`SessionClient::preview`, IPC method
  `preview`).
- **IDs** — newtypes over `Uuid`, derived from `Ctx::id_seed` through `IdGen` (the n-th ID of a call is `seed + n`).

### 4.4 Undo / redo semantics

| Kind | Mechanism |
|---|---|
| `Mutation` | `HistoryEntry { forward: Command, inverse: Vec<Command> }`. Undo dispatches the inverse, which also emits compensating remote ops. If the original op was never sent (`queued` with `attempts == 0`), the change is taken out of it instead of round-tripping (§5.2). |
| `Outbound` | `send_draft` sets `send_at = now + settings.undo_send_ms` (default 10 s) and emits `ScheduleTick{send_at}`; the draft can't be edited or discarded meanwhile. Undo (= `cancel_send`) before then makes it an editable draft again. The first `Tick` at or after `send_at` sets `sending` and emits `SmtpSend`; from then on it is irreversible: every history step whose command (or inverse) names the draft (`Command::draft_id`) is dropped, so later undos never stumble over it. With `undo_send_ms = 0` it leaves at once and is not recorded. A restart cancels scheduled and interrupted sends (with a `send_error`) instead of sending behind the user's back. |
| `Destructive` | Requires `confirm: true`. Not undoable. The receipt says so. |
| `Query` / `View` | Not recorded. |
| `undo` / `redo` | Kind `Mutation`, but never recorded themselves. A `HistoryEntry` keeps the `id_seed` of the original dispatch, and redo re-applies `forward` with it, so re-created entities (e.g. a redone `add_account`) get their old IDs back. Undo moves the entry to the redo stack; any new recorded mutation clears it. Inverses are applied as one transaction. |

Undo history is per session, in memory, capped (default 200). Undo is a `Command`, so agents can undo too.

### 4.6 Approval requests
An agent can prepare a draft but not send it. `send_draft` from `Origin::Mcp` checks the draft like any send
(`not_sendable`), then records an `ApprovalRequest` in `state.approvals` (one per draft; asking again returns the same
request) and returns `{state: "awaiting_approval", approval}`; nothing is scheduled and nothing is recorded in history. Only
the user's origins (`gui`, `tui`, `cli`, `batch`; `Origin::is_user`) may decide (`approve_request`/`reject_request` are `user_only`
in the registry, §4.5): `approve_request` sends the draft as it is then,
exactly like the user's own `send_draft` (undo-send window, history entry *Send* whose undo is `cancel_send`; the entry's
forward command is that `send_draft`, so a redo by an agent only asks again); `reject_request` drops the request and sets the
draft's `send_error` to "the user declined to send it" (not recorded). Agents get `origin_not_allowed`. A request goes away
when its draft is discarded, sent by the user directly or its account removed, and when an agent edits the draft
(`update_draft`, `attach_file`, `remove_attachment` from an agent origin; the receipt lists `approvals_withdrawn`), so the
user never approves text they haven't seen; the user's own edits keep it. An agent's undo of a `cancel_send` asks too.
`dail mcp --allow-send` submits as `Origin::McpAllowSend`, whose sends leave without approval. Requests persist in the
`approvals` table (`MailDelta.approvals` / `removed_approvals`); a snapshot drops requests whose draft is gone.

### 4.5 Command registry

```rust
pub struct CommandSpec {
    pub name: &'static str,        // "archive_threads" — equals the serde tag
    pub title: &'static str,       // "Archive" — palette label
    pub category: Category,        // Mail, Navigation, Compose, Account, App
    pub kind: CommandKind,
    pub agents: AgentAccess,       // allowed | approval | user_only (§9); `CommandName::agent_access()`
    pub default_keys: &'static [&'static str], // ["e"]
    pub available: fn(&AppState) -> bool,      // palette/menu filtering; MCP returns a structured error
}
// spec.description() is the variant's doc comment, read from the JSON Schema (single source).
```

`Engine::dispatch` refuses a `user_only` command from every origin but the user's (`gui`, `tui`, `cli`, `batch`) with
`origin_not_allowed`, before it changes anything. The commands that `undo`/`redo` apply come from the engine's own
history and are not checked again, so an agent can undo the user's `discard_draft` (which applies `restore_draft`). See §9
and ADR 0002.

`CommandName` is a field-less mirror of `Command`. `CommandName::ALL` enumerates the registry; `spec()`, `example()`,
`agent_access()` and `Command::command_name()` are exhaustive matches, and the `registry_is_complete` test checks `ALL`
against the `Command` JSON Schema. `CommandName::args_schema()` is the variant's `args` schema with subschemas inlined (the
MCP input schema). Variants without arguments are written `Name {}` so that `args` is always an object; `Command::from_value`
accepts a missing `args` as `{}`.

The **parity tests** (`app-core/tests/it/registry.rs`, `app-mcp/tests/it/parity.rs`, `app-cli` `exec` unit tests and the
`mcp_stdio_lists_and_calls_tools` end-to-end test) assert that every `CommandSpec.name`:

- appears in the MCP tool list,
- round-trips through `dail exec` JSON parsing,
- has a JSON Schema,
- is offered by the palette and builds a command of the same name (`app-front/tests/it/palette.rs`, and through the
  TUI palette by keys in `app-tui/tests/it/palette.rs`; exceptions:
  `open_palette`/`restore_draft` hidden, `add_account` → wizard, `update_draft` → composer).

## 5. Runtime (`app-sync`)

### 5.1 Session
`Session::open(profile_dir, Backend)` loads config and state snapshot from the Store, then spawns:

- the **engine task**: the single-writer loop over `mpsc<Input{Command|Event}>`;
- the **effect executor**: one worker task that runs the store and network effects of every account one at a time, in
  order (the backend keeps one IMAP session per account; SMTP connects per submission; retries and back-off are engine
  state, §5.2);
- the **ticker**: polls INBOX every `settings.poll_secs` (default 120 s) and fires scheduled sends. Polling is engine state:
  `sync_now` sets `status.next_poll_ms` and emits `ScheduleTick`; a due `Tick` fetches the INBOX of every idle account (or lists
  its mailboxes if no INBOX is cached) and schedules the next poll; without accounts polling stops. Early ticks do nothing.
  One-shot CLI runs abort the timer on shutdown, so they never poll;
- the **IPC listener** (GUI and `dail mcp`, §5.4).

`Backend` is a trait object with `Live` (IMAP/SMTP + redb) and `Memory` (app-testkit) implementations. Besides `execute(effect)` and
`load_state()` it offers `get_blob(id)`: the core can't read blobs, so `Session::submit` resolves the body references
(`body: {state: "fetched", blob}`) of a `get_thread`/`get_message` receipt with `include_body` itself (`app_sync::bodies`): it
parses each blob with `app_io::mime::parse_message` and puts `{text, text_source, attachments}` into the message's
`untrusted_content.body`, dropping the blob ID. A `get_thread` with `rich_body` also gets the `document` of each HTML body
(`parse_message_with_document`, §6); agents don't ask for it, `text` holds the same content.
`LiveBackend::new(store, secrets, verifier, imap)` takes a `Store` (`ProfileStore` in production), a `SecretStore` (`KeyringSecrets`:
the OS keyring via `keyring`, called on blocking threads), an `AccountVerifier` (`NetVerifier`) and an `ImapConnector`
(`NetImap`: TCP → TLS or STARTTLS → greeting → LOGIN, 30 s timeout). `VerifyAccount` reads
the password from the keyring and runs IMAP (connect → TLS or STARTTLS → greeting → LOGIN → LOGOUT) and SMTP
(connect → TLS or STARTTLS → EHLO → AUTH PLAIN/LOGIN → QUIT) concurrently, each with a 30 s timeout. Credentials are only
sent after TLS is established; a refused or unsupported STARTTLS fails verification. Errors name the protocol and server
(`IMAP imap.gmail.com:993: TLS handshake failed: …`) and never contain the password. A backend performs
every effect except `ScheduleTick`, including `PersistState` against its `Store`, and loads the initial state
(`load_state`). `RunRemoteOp` (`app_sync::imap::run_steps`) asks for CAPABILITY, then per step `SELECT`s the mailbox read-write, checks
UIDVALIDITY and runs `UID STORE ±FLAGS.SILENT`, `UID MOVE` (without MOVE: `UID COPY` + `UID STORE +FLAGS.SILENT (\Deleted)` +
`UID EXPUNGE`; without UIDPLUS a plain `EXPUNGE`, around which the messages another client flagged `\Deleted` — found with
`UID SEARCH DELETED` — lose the flag and get it back, so only dail's are expunged) or `UID COPY`, with UID sets collapsed
into ranges (`1:3,7`); `create` first sends `CREATE` and ignores its failure (usually "already exists"). It reports `RemoteOpSucceeded` or `RemoteOpFailed{error, permanent}`:
`permanent` when the server refused (`NO`/`BAD`, changed UIDVALIDITY), not when the connection failed or broke (those are
retried on a fresh session once, like every mail effect).
`SmtpSend` renders the message twice (`app_sync::outgoing::render`: quoted blob from the store, attachments read from disk):
without `Bcc` for submission and with it for the Sent copy. Submission goes through an `SmtpSender` (`NetSmtp` by default,
`LiveBackend::with_smtp` for tests): lettre over rustls with webpki roots, `tls` = TLS wrapper, `starttls` = STARTTLS required,
AUTH PLAIN/LOGIN with the keyring password, `MAIL FROM` the account address and `RCPT TO` every To/Cc/Bcc address once,
120 s timeout; errors read `SMTP host:port: …` and never contain the password. A refused or failed submission is
`SendFailed` (nothing is stored). After acceptance the copy is `APPEND`ed to `sent_copy` with `(\Seen)`; its failure becomes
`MessageSent{sent_copy_error}`.
`MemoryBackend` applies the same steps to its fixture (fresh UIDs from `FakeMailbox::uid_next`).
`--backend memory` exists for smoke tests and demos.

Mail effects (`app_sync::imap`) run on an `ImapSession` (an async-imap session over a boxed stream, so tests drive it
through `tokio::io::duplex` with a scripted server). `LiveBackend` keeps one session per account between effects: a
cached session is checked with `NOOP` before use and replaced if the server dropped it; an operation that fails on a
reused session is retried once on a fresh one; a session that saw an error is discarded. Each operation has a 120 s
timeout. Mail-effect failures are reported as `SyncFailed` events, never with the password in the message. A string
in a command line is either a mailbox name (async-imap refuses CR/LF in those) or quoted with `imap::quoted` (`"` and `\`
escaped, control characters replaced by spaces), so neither a search query nor a Message-ID can end the line and inject
another command.
`ListMailboxes` runs `LIST "" "*"` and maps RFC 6154 attributes (and Gmail's legacy `\Spam`/`\AllMail`/`\Starred`) to
roles; `\Noselect`/`\NonExistent` entries are reported as not selectable. A listing without `INBOX` is treated as a
lost connection, so a truncated response can never wipe the cached mailboxes (IMAP guarantees an INBOX; another backend
needs its own check).

Effect execution: store/network effects run **in order on lanes** (`session::Lane`): each account's server and
keyring effects (`VerifyAccount`, `StoreSecret`, listings, fetches, ops, draft saves, sends, …) on a worker of their
own, everything local (`PersistState`, attachments, the full-text index, compaction) on one more, and lookups of other
servers (`DiscoverServers`, ADR 0006: seconds per try) on a third, so store writes never overtake each other and a slow or unreachable server delays only its own account. Workers start on first use and
shutdown waits for all of them. `ScheduleTick` and `WatchMailbox` effects run concurrently beside them (timers and
watches wait for the world). A failed
effect becomes `Effect::failure_event(error)` and is ingested like any other event. `Session::flush()` waits until all
effects but timers and watches (and the effects they cause) have been ingested; `shutdown()` ingests `Closing` (unsaved drafts are saved
now), flushes, then stops the engine task.
`Ctx` is built from injected `Clock` (`SystemClock`, `VirtualClock` on tokio's paused time for tests) and `SeedSource`
(`RandomSeeds`, deterministic `CounterSeeds`). State changes are published on a `watch` channel (`Session::subscribe`); its
first value is the engine's state after `Engine::new` (which re-queues running ops and cancels sends from before a restart).

### 5.2 Sync model (MVP)
1. `LIST` with SPECIAL-USE gives mailboxes and roles. Roles fall back to the preset mapping and then to name heuristics.
   `MailboxesListed` emits one `FetchEnvelopes` per selectable mailbox.
2. For each mailbox: `EXAMINE` (read-only) and `UID SEARCH UNSEEN` (for `total`/`unread`). If the effect's `uid_validity` matches the
   server's and `since_uid` is set, `UID FETCH since:*` (skipped when `UIDNEXT <= since`; the `n:*` quirk that returns the last
   message is filtered out); otherwise `FETCH (exists-N+1):*`, the newest `N = settings.initial_window` (default 500). Either way at
   most the newest `limit` envelopes are returned. Fetched items: `UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (MESSAGE-ID
   IN-REPLY-TO REFERENCES FROM TO CC SUBJECT DATE CONTENT-TYPE)]`, turned into a `RemoteEnvelope` by
   `app_io::mime::remote_envelope` (mail-parser: RFC 2047 subjects/names, `Date` → ms, `has_attachments` = `multipart/mixed` or a
   single non-text part). On `EnvelopesFetched` the engine drops the mailbox's cached messages first if `UIDVALIDITY` changed,
   updates only the flags of known UIDs, and threads new messages (`Threader`; merges fold threads together). `uid_next` becomes
   `max(server UIDNEXT, highest fetched UID + 1)`, so the next fetch is incremental. Changes persist as one `MailDelta`.
   A vanished mailbox or a removed account takes its messages (and emptied threads) with it.
3. Flag resync for the cached window: after an incremental `EnvelopesFetched` (not a first fetch or a UIDVALIDITY reset) the
   engine emits `ResyncFlags` for `lowest..=highest` cached UID (counted in `SyncStatus.pending`). The runtime runs
   `UID FETCH from:to (UID FLAGS)` and reports `FlagsChanged`: cached UIDs in the range get their flags updated, cached UIDs the
   server no longer has were expunged and are dropped (with emptied threads). A resync whose `uid_validity` no longer matches
   the cache is ignored. CONDSTORE (`imap::fetch_flags`): a server that announces it is opened with `SELECT … (CONDSTORE)`
   and the event carries its `highest_modseq`, which the mailbox keeps; the next `ResyncFlags` passes it as
   `changed_since`, and the runtime fetches `UID FETCH from:to (UID FLAGS) (CHANGEDSINCE m)` (only the messages changed
   since) plus `UID SEARCH UID from:to`, reported as `existing`: cached UIDs that are neither listed nor existing were
   expunged, the others keep their flags. The modseq is stored only from a successful resync, so a failed one never skips
   changes; a new UIDVALIDITY forgets it. QRESYNC (VANISHED instead of the search) is a separate roadmap item.
4. Bodies are lazy: `FetchBody` runs when a thread is opened (`open_thread`, which records it in `status.bodies_in_flight`), for
   `get_thread`/`get_message` with `include_body` (a Query: the download is not recorded, but one already in flight is not
   repeated) and as a prefetch of the newest 20 unread INBOX messages
   without a body after each INBOX `EnvelopesFetched`. `status.bodies_in_flight` prevents duplicate requests. The runtime
   fetches `BODY.PEEK[]` (stays unread), checks UIDVALIDITY, stores the raw RFC 822 with `Store::put_blob` and reports
   `BodyFetched` (the envelope's `body` becomes `fetched{blob}` and `has_attachments` is corrected from the full parse,
   `app_io::mime::has_attachments`, and `snippet` set from `app_io::mime::snippet`; the engine cleans it again with
   `app_core::snippet`: quoted lines, controls and bidi formatting dropped, whitespace collapsed, at most 140 chars) or `BodyFetchFailed` (`body` becomes `failed{error}`). Bodies don't count as sync pending.
5. Remote ops: triage commands change the cache first and queue one `PendingOp` per account in `state.outbox` (persisted with
   the cache, in the same `MailDelta`). Each account runs one op at a time, oldest first: when an op starts the engine resolves
   its messages' *current* server locations into `RemoteStep`s (`Store` per mailbox, UIDs ascending) and emits
   `RunRemoteOp`; an op with nothing left to do (messages expunged meanwhile) completes at once. `RemoteOpSucceeded` drops the
   op and starts the next. `RemoteOpFailed` keeps the local change, records `attempts`/`last_error` and retries with back-off
   computed by the engine (`ops::retry_delay_ms`: 2 s, doubling, at most 5 min): the op stays `queued` at the head of its
   account's queue with `next_attempt_ms`, and a `ScheduleTick` for that time is emitted; a `Tick` starts every op that is
   due. An op the server *rejected* (`permanent`) becomes `failed` after `ops::MAX_ATTEMPTS` (8) failures and no longer blocks
   the queue; an op that could not reach the server keeps retrying, so a long time offline never loses or fails triage. A successful sync step
   of the account (`MailboxesListed`, `EnvelopesFetched`) means the server is reachable again: waiting ops run at once. While an op is in the outbox, flags reported by `EnvelopesFetched`/`FlagsChanged` get the op's
   change applied on top, so a sync never flips a local change back. A session that starts from a snapshot re-queues ops that
   were `running`. Server side: `UID STORE`, `UID MOVE` (fallback `COPY` + `STORE \Deleted` + `UID EXPUNGE`), `APPEND`.
6. Moves (`archive_threads`, `trash_threads`, `move_threads`): an envelope's `mailbox`/`uid` always name its server location;
   a pending move only sets `moved_to`, and every listing (`list_threads`, `get_*`) uses `location()`. When the op succeeds
   the message becomes a *placeholder* in the target (`uid` 0, still shown where the user put it) and the engine fetches
   the target: the fetched envelope with the same `Message-ID` (or, without one, subject + date + size) is *located* — the
   placeholder takes its UID and keeps its ID, flags and body — instead of being added as a new message. A placeholder
   that lands next to a cached copy of itself (Gmail: archiving into All Mail) takes that copy's UID and replaces it. If
   local flags differ from the server's when a placeholder is located, `SetFlag` ops push them. An op that needs a
   placeholder's UID first fetches its mailbox and waits (`awaiting_sync`, blocking the queue); placeholders still unknown
   after that sync are dropped. Moving out of a mailbox with the `all` role into anything but Trash/Junk is a *copy*
   (Gmail: add a label; All Mail keeps the message): a placeholder is created in the target and a `Copy` op queued.
   Sources per thread, grouping copies by `Message-ID`: archive takes copies shown in Inbox, Junk or role-less folders;
   trash takes everything but Trash/All/Flagged; `move_threads` everything but Sent/Drafts/All/Flagged; if a message has
   no such copy and none in the target, its All Mail copy is taken (trash, move). Named `messages` always move. Undo is
   `move_threads{messages, to: <where they were shown>}` per source.
7. Unsent ops are edited, not answered: an op is *unsent* while it is `queued` with `attempts == 0`. Setting a flag takes the
   message out of an unsent op that sets it the other way (no new op). Moving a message takes it out of an unsent move; if it
   then goes back to its server location and no other op still moves it, nothing is queued. Moving a copy placeholder back
   to where its source is shown drops it from its unsent copy and deletes the placeholder. Ops left empty are removed.
   Receipts list these messages in `cancelled`. This is how undo cancels a pending op; it also coalesces quick toggles. Targets of pending ops (and of `moved_to`) are never
   dropped by a mailbox listing.

8. Drafts (local-first): every draft change persists at once and emits `ScheduleTick{now + DRAFT_SAVE_IDLE_MS}` (5 s). A `Tick`
   saves each draft that needs it (`saved_ms != Some(updated_ms)`), has been idle that long, has no attachment still being
   checked and whose account has a mailbox with the `drafts` role: `SaveDraft` carries the resolved `OutgoingMessage` and
   `revision = updated_ms`; `status.drafts_saving` prevents a second save in flight (cleared on restart). `Closing` (session
   shutdown) saves them without waiting. The runtime renders it (`Bcc` kept), `SELECT`s Drafts, finds older copies with
   `UID SEARCH HEADER Message-ID "<id>"`, `APPEND`s the new one with `(\Seen \Draft)` and then deletes the older copies (`UID STORE
   +FLAGS.SILENT (\Deleted)` + `UID EXPUNGE`, without UIDPLUS a plain `EXPUNGE` that spares other clients' deletions as
   above), so a failure never loses the server copy.
   `DraftSaved{revision}` sets `saved_ms` (a draft edited meanwhile is saved again); `DraftSaveFailed` records `save_error` and
   retries after `DRAFT_SAVE_RETRY_MS` (60 s). Discarding a draft that is (or is being) saved emits `DeleteDraft` (effects run in
   order, so it follows a running save); `DraftDeleteFailed` shows in the account's `SyncStatus.last_error`. Undo of a discard
   restores the draft with `saved_ms: None`, so it is saved again (once its attachments are checked). The server copy is
   also fetched into the cache like any message of the Drafts mailbox. Drafts with a scheduled or running send are not saved.
9. Sending: `SmtpSend` goes to the account's SMTP server; the copy for Sent (`sent_copy`, with Bcc) is the `sent`-role mailbox
   unless the preset's `smtp_saves_sent` says the server files it itself (Gmail), or there is none. `MessageSent` removes the
   draft (and `DeleteDraft`s its server copy); a failed Sent copy shows in `SyncStatus.last_error` ("sent, but …").
   `SendFailed` makes the draft editable again with `send_error`.
10. Search: the cache has only the synced window and no body text, so `set_search` (always) and `search` (unless
   `server: false`; not again while a round for the same query runs or within `SERVER_SEARCH_FRESH_MS` = 60 s after it)
   also search the servers. The engine picks per account the synced mailboxes the query's `in:` terms name, else the
   `all`-role mailbox (Gmail: All Mail), else every mailbox but Trash and Junk, and emits one `SearchMailbox` per mailbox
   with the query minus its `in:` terms (nothing if only `in:` terms remain). A round is identified by `started_ms` and knows
   its size (`mailboxes`); `set_search` records it in `status.searches` up front, `search` (a Query) only when results
   arrive. Rounds are keyed by the lower-cased canonical query and account; the latest 8 are kept, a restart forgets them.
   The runtime `EXAMINE`s the mailbox, runs `UID SEARCH` with the criteria from `app_sync::search` (`TEXT`, `FROM`, `TO`,
   `CC`, `SUBJECT`, `SEEN`/`UNSEEN`/`FLAGGED`, `HEADER Content-Type "multipart/mixed"`, `SENTBEFORE`/`SENTSINCE`, `NOT`;
   `CHARSET UTF-8` for non-ASCII; control characters in values become spaces, so a query cannot inject commands) or, with
   the preset's `raw_search` (Gmail), `X-GM-RAW` in Gmail syntax (dates as Unix seconds), and fetches the envelopes of
   the newest `SERVER_SEARCH_LIMIT` (200) hits. `SearchResults` merges them like `EnvelopesFetched` (known UIDs get flags,
   placeholders are located, the rest become threaded messages; `uid_next`/counts are untouched) unless UIDVALIDITY
   changed, and adds their IDs to the round's `hits`. `search` shows a hit when all its non-text terms match the cached
   state (text terms were matched by the server, possibly in the body). `SearchFailed` records `last_error`.
   The local full-text index (ADR 0005, `app_io::fulltext`) answers too: `search` and `set_search` with text terms emit
   `SearchIndex{search, words: the non-negated text terms, limit: 500}`; the runtime looks for messages whose subject,
   senders or body contain every word (tokenized like the indexed text, never parsed as query syntax) and reports
   `IndexSearched{search, hits}`, kept in `status.index_searches` (the latest 8 queries) and matched like server hits.
   The runtime indexes each body it stores (`FetchBody`), drops messages a `PersistState` removes, and at start indexes
   the stored bodies the index lacks on a thread of its own (`app_sync::fulltext::index_missing`). An index that can't
   be opened is deleted and rebuilt; without one, `SearchIndex` finds nothing. `MemoryBackend` keeps one in RAM.
   `search {order: relevance}` puts threads with a full-text hit first, in the order of their best hit (tantivy's score
   order; `listing::by_relevance`), then the rest newest first; `newest` (the default, and the GUI list) is by date.
11. Push (IMAP IDLE, `engine::watch`): the first successful INBOX `EnvelopesFetched` of an account emits `WatchMailbox` and
   sets `SyncStatus.watch` to `watching`. The runtime keeps a connection of its own per account for it
   (`LiveBackend.watchers`, reused between watches): `CAPABILITY` (no `IDLE` → `WatchFailed{unsupported: true}`),
   `EXAMINE`, `IDLE` until an untagged response other than `OK` (`EXISTS`, `EXPUNGE`, `FETCH`: `changed: true`) or
   `imap::IDLE_TIMEOUT` (25 min; servers drop IDLE at 29) passes (`changed: false`), then `DONE`
   (`imap::idle_once`); a reused connection that fails is replaced once. `MailboxWatched{changed: true}` fetches the INBOX
   (even mid-round: an incremental fetch twice is harmless) and watches again at once, so nothing is missed meanwhile;
   `changed: false` only watches again. `WatchFailed` sets `retrying{at_ms, error}` and a `ScheduleTick` for
   `WATCH_RETRY_MS` (60 s) later, when a `Tick` watches again; `unsupported` sets `unsupported` for the session. The INBOX
   poll skips accounts that are `watching`, so polling is the fallback for servers without IDLE and while a watch is
   down. Results for removed accounts or mailboxes are dropped. `SyncStatus.watch` is runtime state (a restart starts
   `off`). `app-testkit`'s `MemoryBackend::with_push()` wakes a watch on every `edit_server`; without it the fake server
   has no IDLE.
12. History backfill (`engine::backfill`): a mailbox keeps its newest `sync_window` messages cached (`set_sync_window`;
   `None` = `settings.initial_window`, `0` = all). Whenever fewer than `min(window, total)` are cached
   (`listing::history`) and `history_complete` is not set, the engine emits `FetchOlder{before_uid: the oldest cached UID,
   limit: min(what is missing, BACKFILL_BATCH = 200)}`, one per mailbox at a time (`status.backfilling`), after each fetch of
   the mailbox, after each batch and after `set_sync_window`. Backfill is not counted in `SyncStatus.pending`, so polls and
   the "synced" status carry on. The runtime (`imap::fetch_older`) `EXAMINE`s, finds the sequence number of `before_uid`
   (`UID FETCH u (UID)`) and fetches the `limit` messages below it by sequence number (an expunged `before_uid` falls back to
   the highest UIDs of `UID SEARCH UID 1:u-1`); `complete` when sequence number 1 is among them. `OlderFetched` merges them
   like a fetch (threaded, placeholders located) and sets `history_complete` when complete or empty; another UIDVALIDITY is
   ignored, and a new UIDVALIDITY on a fetch clears `history_complete`. `BackfillFailed` waits for the mailbox's next fetch.
   Lowering the window keeps what is cached. `list_mailboxes` reports `cached`, `window` and `backfilling`; the sidebar shows
   `↓N%` while a mailbox backfills.
13. Highlighting (`app_front::highlight`): while a search is open, its positive text and `subject:` words are marked
   (case-insensitive, on character boundaries) in the list's subjects and in the reader, in plain text and in document
   spans (`ui::marked`, on `emphasis`).

**Periodic sync (UX-6).** Everything runs in the engine's `Tick` → `poll()` path, so it is replayable
(`tests/replay/periodic_sync.jsonl`):
- *Refresh on open*: `open_mailbox` (and `focus_mailbox`) emits `FetchEnvelopes` when the mailbox's `last_fetch_ms`
  (set by `EnvelopesFetched`) is older than `stale_secs` and `status.fetching` (fetches in flight, cleared by
  `EnvelopesFetched`/`SyncFailed{mailbox}` and on restart) does not hold it; the receipt says `fetching`.
- Each due poll, per idle account: a *full round* (`ListMailboxes` → a fetch of each mailbox) when `full_sync_secs` is
  not 0 and `SyncStatus.last_full_ms` (also set by `sync_now`) is that old; else INBOX (unless IDLE watches it) and the
  open mailbox (`view.mailbox`, if not in flight).
- *Back-off*: `SyncStatus.failures` counts failed sync steps since the last success (`EnvelopesFetched` or
  `MailboxesListed` reset it); from two on, the account's polls wait `min(poll_secs × 2^(failures − 1), 15 min)` after
  the last failure.
- The status line reads `synced 2m ago · next in 1m` (`app_front::format::until`, `status.next_poll_ms`).

### 5.3 Provider presets

| Preset | IMAP | SMTP | Archive | Sent copy | Notes |
|---|---|---|---|---|---|
| `hostinger` | imap.hostinger.com:993 TLS | smtp.hostinger.com:465 TLS | move → `archive` role, else `Archive` (created on first use, inside `INBOX<delim>` if the account's folders live there) | APPEND to Sent | |
| `gmail` | imap.gmail.com:993 TLS | smtp.gmail.com:465 TLS | move INBOX → `[Gmail]/All Mail` (`archive_to_all`: the `all` role) | SMTP saves it automatically, so skip APPEND | App Password (2FA required); trash = `trash` role, else `[Gmail]/Trash`; server search via `X-GM-RAW` (`raw_search`) |
| `custom` | user-supplied | user-supplied | SPECIAL-USE `\Archive` or `Archive` | APPEND | STARTTLS on 143/587 supported |

TLS: rustls with the `app-tls` crypto provider (ADR 0003, `docs/plan-tls.md`) and `webpki-roots`
(`app_sync::tls::client_config`; `tls::smtp_parameters` installs `app_tls::provider()` as rustls's process default for
lettre, with the same roots). Plaintext connections are refused: `Security` has only `tls` and `starttls`. No crate in
the build compiles C: `deny.toml` bans `ring`, and `cc` only appears under crates that build nothing with it.

app-tls implements only what a TLS client needs, in safe Rust: TLS 1.3 and 1.2 with ChaCha20-Poly1305
(`TLS13_CHACHA20_POLY1305_SHA256`, `TLS_ECDHE_{ECDSA,RSA}_WITH_CHACHA20_POLY1305_SHA256`), X25519 key exchange, and
signature verification with ECDSA P-256/P-384 (SHA-256/384) and RSA 2048–8192 (PKCS#1 v1.5 and PSS, SHA-256/384/512).
What servers accept (measured 2026-10-18, `docs/plan-tls.md` §2):

| Algorithms | Servers |
|---|---|
| X25519 + ChaCha20-Poly1305 (app-tls today) | Gmail, Hostinger, Yahoo, iCloud, Fastmail, Zoho, GMX, AOL, Yandex, IONOS, one.com, Migadu, web.de, mailbox.org, Purelymail; mail.ru (TLS 1.2 only) |
| + AES-GCM (tier 2, ROADMAP) | + Namecheap Private Email |
| + P-256/P-384 key exchange (tier 3, ROADMAP) | + Microsoft 365 / Outlook, T-Online |

A handshake that fails for lack of a common version, suite or group (`HandshakeFailure`, `ProtocolVersion` or
`InsufficientSecurity` alerts, `PeerIncompatible`) gets the hint "the server supports none of the TLS ciphers dail
implements (TLS 1.2/1.3 with ChaCha20-Poly1305 and X25519)" in the IMAP and SMTP error
(`app_sync::tls::incompatible_server_hint`).

### 5.4 Single instance & IPC attach
One process owns a profile at a time: it holds `profile_dir/lock` (`ProfileLock`, an exclusive `File::try_lock`, released by
the OS when the process dies). Long-running owners — the GUI, `dail tui` and `dail mcp` — also listen on the profile's local socket
(`app_sync::ipc::IpcServer`, via `interprocess`: the Unix domain socket `profile_dir/dail.sock` inside the 0700 profile dir,
or on Windows the named pipe `dail-<FNV-64 of the profile path>`; a stale socket left by a crashed owner is replaced).
`dail exec`, `dail batch`, `dail account add`, `dail tui` and `dail mcp` first `ipc::claim` the profile: connect to the socket
(→ `Claim::Attached(IpcClient)`), else take the lock (→ `Claim::Owned`, open the profile here), else retry for up to 10 s
(`ProfileBusy` after that). The GUI must own the profile; it refuses to start while another process does. Front-ends talk
to a `Connection`: `Local(SessionClient)` or `Attached(IpcClient)`. `SessionClient` is a cloneable handle of a `Session`
(`Session::client`) that does not keep it alive: after `shutdown` its calls fail with `SessionClosed`. This way an agent
always drives the same state the user is looking at, and the GUI repaints live when an agent archives a thread. With
`--backend memory` a session is private unless `--profile-dir` is given, which then provides the lock and socket only.

Protocol: newline-delimited JSON-RPC 2.0, requests answered in order. `submit {origin, command}` → `{"ok": receipt}` or
`{"rejected": CoreError}`; `preview {origin, command}` → `{"ok": Preview}` or `{"rejected": …}` (a dry run); `flush` → `null`; `state` → the `AppState` snapshot. `watch` → `null`, after which the connection only
carries `{"method": "changed"}` notifications, one after each burst of state changes (coalesced by the session's `watch`
channel), until either side closes it. `Connection::subscribe()` gives every front-end the same
`watch::Receiver<Arc<AppState>>`: the session's own channel when local; when attached (`IpcClient::subscribe`), a
channel fed by a task that watches on a second connection and reads `state` again after each notification, one read
at a time. Its `changed()` fails once the owner goes away.

`app_sync::Bridge<T>` connects a front-end whose UI thread never awaits (the GUI, the TUI) to a `Connection`. The UI
calls `submit(tag, command)`; a task submits in order with the front-end's origin and returns `Reply { tag, name,
result }` through a channel the UI drains (`replies()`); `state()` is the latest snapshot of `Connection::subscribe`.
Every state change and reply calls the front-end's wake-up callback. `Bridge::own(session)` shuts the session down
(drafts saved) when the bridge closes; `Bridge::attach(conn)` leaves the owner running. `is_gone()` says that the
session went away (shut down, or the owner process ended). Only the `tui`, `cli`, `batch`, `mcp`
and `mcp_allow_send` origins may be claimed, so a local client can't pose as the GUI or the runtime (an attached
`dail tui` claims `tui`, a user origin like `cli`). The wire form of `add_account` carries the
password (it must reach the owner's keyring; the socket is user-only); everywhere else `Secret` serializes as `<redacted>`.
Transport failures are JSON-RPC errors (`-32600` bad request, `-32601` unknown method, `-32602` bad params or origin,
`-32603` session closed).

### 5.5 Updates of dail (Wave 7, ADR 0011)

Effects run on their own worker (`Lane::Update`). `app_sync::https::Https` is hyper's HTTP/1.1 client over
tokio-rustls with app-tls and the webpki roots: one GET per connection, `User-Agent: dail/<version>` and nothing else
about the user, redirects at most 5 times and only to `github.com`, `objects.githubusercontent.com` and
`release-assets.githubusercontent.com` (`ALLOWED_HOSTS`), 30 s for the response head and 60 s per body stall.
`CheckForUpdates{running}` GETs `releases/latest/download/manifest.json` (at most 64 KiB) and its `.minisig` and
hands both to `app_io::update::verify_manifest` with `UPDATE_PUBLIC_KEYS` → `UpdateChecked{manifest}` or
`UpdateFailed{reason}`. `DownloadUpdate{version, artifact}` empties the profile's `updates/`
(`LiveBackend::with_updates_dir`; only the newest download is kept), streams the artifact into `<name>.part` through
`app_io::update::ArtifactCheck` (size and SHA-256 against the manifest, failing as soon as it is too long), renames it
to the URL's last segment and makes AppImages and binaries executable → `UpdateDownloaded{version, path}`. At start the CLI
runtime of a live session ingests `UpdateHostDetected{host}` (`app_sync::install::desktop_host`: the version, the
target triple, and the install kind from `Probe`: `$APPIMAGE` → AppImage, `/usr/bin/dail` listed by dpkg → `.deb`,
under `%ProgramFiles%\dail` → MSI, else a plain binary), so a due periodic check starts then. `InstallUpdate{version,
install, path}` runs `app_sync::install::STRATEGIES`: an AppImage is copied beside `$APPIMAGE` and renamed over it, a
plain binary is swapped with `self-replace`, a `.deb` goes through `pkexec apt-get install -y <file>` (without
`pkexec` the failure names the `sudo apt-get install` command), an MSI starts `msiexec /i <file>`; the phones never
install → `UpdateInstalled{version}` or `UpdateFailed{reason}`. The memory backend answers the three effects from
`app_testkit::updates::FakeUpdates`: release `0.99.0` signed by a test key (`app-io/testdata/updates/`; the secret
key was discarded and no build trusts it), with switches that tamper with the manifest, the signature or the
artifact. A memory session reports `app_testkit::updates::host()`. `dail update check` and
`dail update install [--yes]` are CLI aliases for the chain. An
`#[ignore]` test (`DAIL_IT_UPDATES=1`) GETs the release page over app-tls.

The front ends read `app_front::updates` (Wave 7 F1–F3): the panel (running version, a status line, the buttons —
*Check now*, *Download* on desktop installs, *Install…* through the palette's confirmation step, *Release notes*,
*Skip this version*), the badge (`Update x.y.z`, `Restart to update`) and the first-run question, asked once an account
exists while `settings.updates.checks` is `unset`. The GUI shows the panel and the interval in Settings › General, the
badge in the top bar (it opens that section) and the question as a callout over the reader; the TUI shows them as the
first rows of the general settings and in the status line; the phones as the frame's banner. The policy is saved as
`[settings.updates]` in `config.toml` (`checks`, `interval_hours` 1–720, `skipped_version`).

## 6. Storage (`app-io`)

```text
<data_dir>/dail/profiles/<name>/          # directories::ProjectDirs (XDG on Linux, %APPDATA% on Windows)
  config.toml        # accounts (no secrets), settings, keymap overrides
  state.redb         # tables: mailboxes, envelopes, threads, drafts, outbox, approvals, meta(schema_version)
  blobs/ab/cd…eml    # content-addressed raw messages (sha256)
  index/             # full-text index of downloaded bodies (tantivy, ADR 0005; derived, rebuildable)
  backups/20261020T081500Z/{config.toml,state.redb}   # daily, newest 7 (`store::backup`)
  state.redb.broken-<time>     # a cache that did not open, set aside (`dail doctor` lists them)
  logs/replay-YYYYMMDD.jsonl   # {"t":ms,"seed"?:u128,"origin":"mcp","input":{"command":…}|{"event":…}} — bodies/secrets redacted
```

Replay format (`app-core::replay`): `t` becomes `Ctx::now_ms`, `seed` becomes `Ctx::id_seed` (default `line << 64`), blank
lines and `#` comments are skipped. The audit log (`app_sync::AuditLog`, `Session::open_audited`) is written for every session
on a profile (live, or memory with `--profile-dir`): the engine loop records each command and event it dispatches or ingests
— accepted or rejected, in processing order, with the exact `Ctx` (`ReplayRecord::new`) — and a writer task appends
`ReplayRecord::redacted_line()` to `logs/replay-YYYYMMDD.jsonl` (UTC day of `t`; dir 0700, file 0600 on Unix; a
`# session opened at <ms>` comment starts each session; write failures are logged, never fatal). Redacted: passwords
(`Secret` serializes as `<redacted>`) and the message text commands carry (`body`, `body_text` → `<redacted>`). Headers in
fetch events (subjects, addresses) stay, so the log is private to the profile. A log of a session that started from an
empty profile replays to the same state except for the redacted text (`app-sync/tests/it/audit.rs`); a session that started
from a snapshot replays relative to an empty state. `dail replay <log> [--state]` (app-cli) replays a file offline and
prints one result per record like `batch` (`{"line": n, …}`), then optionally `{"state": …}`; rejected records keep exit
code 0, an unreadable log exits 2. `replay::run` continues past rejected inputs and returns one `ReplayStep { line, result:
ok(Outcome) | err(CoreError) }` per record. Fixtures live in `crates/app-core/tests/replay/*.jsonl`; the harness snapshots
steps + final state with `insta` and checks that replaying twice is identical.

- `ProfileDirs` (app-io) resolves the layout above: `ProfileDirs::named("default")` uses `ProjectDirs::from("", "", "dail")`,
  `ProfileDirs::at(dir)` takes an explicit root; `ensure()` creates it with mode 0700 on Unix.
- `config.toml` (`app_io::Config`): `[settings]` (missing keys take defaults, values range-checked; `images_from` lists the
  senders whose inline images show), `[view]` (the saved part of `ViewState`, `ViewPrefs`: `sidebar_collapsed`, `collapsed` folder keys as `[[view.collapsed]]`, `focused`, `list_filter`; only
  written when not all default; UX-1) and one `[[account]]`
  table per account (`id, display_name, email, preset, imap, smtp, secret` — `SecretRef` only; `status` is runtime state
  and not saved). Saves are atomic and durable (`store::backup::write_atomic`: write `config.toml.tmp`, flush it to
  disk, rename it over, flush the directory on Unix); blobs are written the same way, and redb commits are durable.
- Crash safety (`app_io::store::backup`): `ProfileStore::open` (under the profile lock) opens `config.toml` and
  `state.redb`; a `state.redb` that does not open (a torn or foreign file; not a newer schema) is renamed
  `state.redb.broken-<time>` and the newest backup's copy put in its place, or none (the cache fills again from the
  servers). Only once both files open is the daily backup made: `backups/<UTC time>/` with both files when the newest
  is a day old, the newest 7 kept; blobs are not copied (they can be downloaded again). A broken `config.toml` is
  never replaced (the user edits it): opening fails with the parse error and `dail doctor` names the backups.
- `Store` trait: `load_snapshot`, `apply(PersistDelta)` (deltas a store does not hold are ignored), `put_blob(bytes) → id`,
  `get_blob(id)`. Implementations: `ConfigStore` (settings + accounts in `config.toml`; no blobs), `ProfileStore` (production:
  routes `Mail` deltas to `RedbStore`, the rest to `ConfigStore`, blobs to `BlobDir`; on load it merges both and drops cache
  entries of accounts missing from the config) and `MemoryStore` (app-testkit, blobs in a map).
- `RedbStore` (`state.redb`): tables `mailboxes`, `messages`, `threads`, `outbox`, `drafts` (`u128` ID → JSON record; the
  outbox and the drafts are loaded in `created_ms` order; `drafts` was added within schema version 1, opening creates it) and `meta` (`schema_version`). Each `MailDelta` is one ACID write transaction (removals, then upserts). Unreadable records are skipped
  with a warning on load.
- `BlobDir` (`blobs/`): `blob_id` = lower-case hex SHA-256; stored at `blobs/<2 hex>/<62 hex>.eml`, written via a temporary
  file and rename, deduplicated by content. IDs other than 64 lower-case hex digits are rejected, so an ID can't escape the dir.
- Persistence is an `Effect` (`PersistState{delta}`), so the engine stays pure. `PersistDelta` is `Settings`, `Account`,
  `AccountRemoved`, `View(ViewPrefs)` (emitted by `dispatch`/`ingest` whenever the input changed `view.prefs()`; never in
  undo history; `ConfigStore` writes it, `RedbStore` never sees it) or `Mail(MailDelta)` (`mailboxes`, `messages`, `threads`, `ops`, `drafts` upserted; `removed_mailboxes`,
  `removed_messages`, `removed_threads`, `removed_ops`, `removed_drafts`; removals apply first; `ConfigStore` ignores it).
- MIME (`app_io::mime`): `summarize_headers` / `remote_envelope` for envelopes (message IDs lose whitespace and control
  characters, which mail-parser keeps between `<` and `>`); `parse_message(raw) → ParsedMessage {headers,
  text, text_source: plain|html|none, attachments: [{part, filename, content_type, size, inline, content_id}]}`. The text is the
  text/plain parts, else the text/html parts rendered by `html2text` (100 columns; scripts/styles dropped, remote images never
  fetched, links as numbered footnotes with their real targets); a text part stranded by broken MIME structure is still shown.
  Charsets are decoded by mail-parser with `full_encoding` (encoding_rs: ISO-2022-JP, Shift_JIS, GB18030, Big5, …). Parsing never
  fails; garbage yields empty fields. Corpus snapshots: `crates/app-io/tests/corpus/*.eml`.
- Rendering (`app_io::compose`): the engine resolves a draft into an `OutgoingMessage` (`Draft::outgoing(from, quote_blob,
  date_ms)`: account as `From`, the draft's `Message-ID` — derived from `Ctx::id_seed` when the draft was created — threading
  headers, a `QuoteSource{style, header, blob?}` and only `ready` attachments). The runtime parses the quoted blob's text and
  reads the files, then `render(message, original_text, files, Bcc::Keep|Hide)` builds it with `mail-builder`: UTF-8 plain text
  (`body_text`: own text, then the reply's `> `-quoted original under its attribution line or the forward's header block and
  original), `multipart/mixed` with attachments, `Date` from `date_ms`, `User-Agent: dail`. `Bcc` is kept for Drafts/Sent
  copies and hidden from the copy given to SMTP. A reply whose original body isn't cached carries no quote.
- Documents (`app_io::document::html_document`, `mime::parse_message_with_document`): the HTML part parsed by
  `html5ever` into a small DOM (an `Rc` tree, taken apart without recursion) and walked once into `app_core::document`
  blocks — `paragraph`, `heading{level}`, `list{ordered, start, items}`, `quote`, `preformatted`, `table{rows: [[cell
  {header, spans}]]}`, `rule` — of spans `{text, bold?, italic?, code?, strike?, link?}`. White space collapses as in a
  browser; scripts, styles, forms, frames, media, `hidden` and inline-`display:none` elements are dropped, as is text that
  shows nothing (newsletters' zero-width preview padding). Links keep only `http`/`https`/`mailto` targets, images become
  `[alt]`. Mail clients' markers of quoted text (`gmail_quote`, `<blockquote type=cite>`, `yahoo_quoted`, Outlook's
  `divRplyFwdMsg`/`appendonsend` and every sibling after it) and of signatures (`gmail_signature`, `moz-signature`, a
  top-level paragraph starting with a `--` line, at most 15 lines) become `fold{kind: quote|signature, blocks}`; a fold
  inside a fold is a plain quote. A table is kept only as a data table (2–8 columns, every cell one paragraph, no `role="presentation"`);
  layout tables are flattened into their cells' blocks. Below 48 levels of nesting an element is reduced to its text.
  An `<img src="cid:…">` becomes a span with `image` (the `Content-ID`) and the text `[alt]` (or `[image]`);
  `mime::inline_part(raw, cid)` returns that part's type and bytes. Corpus snapshots:
  `crates/app-io/tests/it/snapshots/*_document.snap`.
- "Open original" (`app_io::original::original_html(raw)`): the HTML part sanitized with `ammonia` (scripts, styles,
  event handlers, forms, frames and all media removed; links only `http`/`https`/`mailto` with
  `rel="noopener noreferrer nofollow"`), or the text in an escaped `<pre>`, below escaped From/To/Cc/Subject, in a page
  with `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'` and `referrer: no-referrer`.
- Compaction (`compact_store` → `CompactStore{keep}`: the blobs of every `fetched` cached message): `Store::compact(keep,
  written_before)`; `ProfileStore` deletes the blobs (and leftover `.eml.tmp` files) not in `keep` that were last written
  more than an hour ago (`COMPACT_GRACE`: a body fetched just now may not have reached the engine yet), then runs redb's
  `compact()` (`RedbStore` holds its `Database` in an `RwLock`: transactions share it, compaction takes it alone).
  `MemoryStore` drops unused blobs at once; `ConfigStore` has nothing to compact. `status.compaction` shows the outcome.
- `meta.schema_version` plus forward-only migrations in `app-io::store::migrations` (`CURRENT = 1`; `MIGRATIONS[n]` upgrades
  `n+1 → n+2`, all pending ones run in the open transaction). A file written by a newer dail is refused (`SchemaTooNew`).

## 7. Front-ends

**Strings and languages** (`app_front::i18n`): the texts the front-ends share are keys of a strings table
(`i18n::Key`, an English template each with `{name}` placeholders, filled by `i18n::tr`). The front-ends pick a
`Lang` at startup from `$DAIL_LANG` (`Shell::set_lang`, `Tui::with_lang_tag`). Besides English there is a
pseudo-locale (`DAIL_LANG=pseudo`): accented, a third longer and bracketed, so hard-coded and clipped strings show.
The status texts (problems, hints, fixes, sync and pending summaries) are in the table; the GUI's and TUI's own labels
move over later. Right-to-left: egui 0.37 lays text out left to right with no bidi algorithm, so Hebrew or Arabic
mail shows in logical order and the layout is not mirrored; bidi control characters from mail cannot reorder the UI
(the TUI replaces them, `text::is_unsafe`; notifications drop them).

### 7.1 GUI (`app-gui`)
```text
┌──────────────┬────────────────────────────────────────────────────────────────────┐
│ ▣ dail     ◧ │ ⌕ Search or run a command  Ctrl K    ● synced 1m ago [2 pending] j/k │
│ [Compose   C]├───────────────────────────┬────────────────────────────────────────┤
│ ALICE@…      │ Inbox [12 unread]         │ Q3 numbers                             │
│ ▸ Inbox   12 │ ● Alice   Q3 numbers 9:41 │ Alice <alice@…> → me        Today 9:41 │
│   Sent   g s │   Bob     Lunch?     9:02 │                                        │
│   Archive g a│                           │ Hi, attached are…                      │
│ + Add account│ “Archived 3” Undo z  2 CONV│ [Reply R] [Reply all A] [Forward F] …  │
│ ⚙ Settings   │                           │                                        │
└──────────────┴───────────────────────────┴────────────────────────────────────────┘
```
- **Palette (Ctrl+K / Ctrl+Shift+P):** (UI-9: an overlay 560 wide under the top bar over the scrim; the input row with a
  search icon, the query in `headline_md` and an `Esc` cap; 36 px rows with an icon by category, mailbox role or
  thread, the title, the detail and key caps, the cursor row inverse; with an empty query the rows sit under captions
  — *Recent*, the command categories, *Mailboxes*, *Threads* — from `app_front::palette::Palette::sections`, while a
  query gives one ranked list; a footer with the keys and the version) fuzzy match (`nucleo-matcher` over title and detail; rows whose title matches
  come first, equal scores prefer the shorter title) over the
  available `CommandSpec`s (Mail, Compose, Navigation, Account, App), mailboxes ("Go to …") and the 20 newest threads,
  plus "Search mail for …" for the typed text. `app_front::palette::flow(name, state)` (shared with the TUI) is an
  exhaustive match over `CommandName` that says how each command gets its arguments: from the view first (the selection or open thread, its
  newest message, the composer's draft, the open mailbox's account), otherwise from second-step pickers (mailboxes —
  *Move to…* offers the thread's account only —, threads, drafts, accounts, pending ops, settings, panes,
  attachments, free text, and a confirmation for `remove_account`). `add_account` opens the account wizard,
  `update_draft` opens the draft in the composer; `open_palette` and `restore_draft` are not offered. Query results
  (`get_settings`, `search`, …) open in a read-only window as `app_front::result`'s document (MISC-22: sections of
  labelled fields and rows; a row that shows a thread or a mailbox opens it), with a *Raw data* toggle for the JSON.
  `app_front::result::doc` builds it from the command, the receipt's data and the state for the GUI, the TUI and
  mobile alike; every string goes through `text::clean`, data a view does not know falls back to labelled fields,
  and `app-front/tests/it/result.rs` snapshots every query over the demo state and fails if a query has no view. Commands chosen lately rank higher (frecency, kept for the
  session in `app_front::palette::Palette`: a count halved every 8 choices; first with an empty query, and among the
  best five matches unless the query starts the best title), and the highlighted command's first doc-comment sentence
  (`Palette::help`, what agents read as the tool description) shows under the list, in the TUI too. `app-front/tests/it/palette.rs` walks every registry command
  through its flow (the palette half of the parity tests).
- **Keys (defaults, Gmail-like):** `j/k` (or ↓/↑) next/prev · `Enter/o` open · `e` archive · `#` trash · `v` move ·
  `s` star · `u` toggle read · `r/a/f` reply/all/forward · `c` compose · `/` search · `g i/s/d/a/t` go to
  inbox/sent/drafts/archive/trash · `z` / Ctrl+Z undo · Ctrl+Shift+Z / `Shift+Z` redo · `Tab`/`Shift+Tab` next/previous
  pane · Ctrl+B / `[` sidebar · F5 / Ctrl+R sync · Ctrl+Enter / Ctrl+S send · Ctrl+K / Ctrl+Shift+P / `:` palette ·
  `Esc` back. The second keys are the ones a plain terminal delivers (the TUI, §7.4; the GUI takes them too): every
  command with default keys has one (`app-tui/tests/it/keys.rs`).
  The keymap (`app_front::keymap`, shared with the TUI) is built from the settings (`keymap::keys_for`): a command's
  override (`set_keybinding`, `settings.keys` in `config.toml`; its keys are taken away from every other command),
  else its preset keys (`set_key_preset`: `gmail`, the registry's `default_keys`; `vim`: `u` undo, `Ctrl+R` redo,
  `d d` trash, `m r`/`m u` read/unread, `F5`/`g r` sync). `KeyState` rebuilds its bindings when the key settings change;
  the palette's and the TUI's hints and help follow them. Each binding is a chord
  (`Ctrl+Shift+Z`, `#`) or a sequence (`g i`, typed within 1.5 s; the front-end passes the time in). A `Chord` is
  toolkit-neutral: modifiers (Ctrl, Shift, Alt) and a named key, letter or digit, or a typed symbol (`#`, `/`) whose
  key depends on the layout; `app_gui::keymap` maps egui's key and text events onto it (a symbol key and its text
  are one chord). `keymap::resolve` fills the arguments from the view — `j/k` select (or, with the
  reader open, open) the neighbouring row (with the sidebar focused they open the neighbouring mailbox, and Enter
  focuses the list), `u` is `mark_read` if a targeted message is unread and `mark_unread`
  otherwise, `Escape` closes the composer (when focused), else the thread, else the search, `Tab` cycles through the
  panes that can have the focus — and otherwise uses the palette's `flow`; a command that still needs an argument
  opens the palette at that step (`v` → *Move to…*, `/` → the search text). Several commands may share a key only
  where the view decides (`Escape`, `u`; `app-gui/tests/it/keys.rs` checks this). While a text field has the focus only
  `Escape` and Ctrl chords other than the editing keys (A, C, V, X, Y, Z) act. The palette shows each command's first
  key, and `g <letter>` on the mailboxes of the current account.
- **Composer:** plain text in the MVP, with To/Cc/Bcc chips and attachments by path / file dialog (`rfd`, XDG portal on Linux, kept in app-gui).
  It shows `view.composer` in place of the reader, or, for a reply to a message of the open conversation, docked under
  it in a resizable bottom panel; the reader then scrolls to its end once (UI-13). New drafts and forwards keep the
  whole pane (UI-8: a header with the title, the save state and *Close*; labelled
  hairline rows for From, To, Cc, Bcc and Subject; the body in `body_lg`; a footer with *Attach files…*, the attachment
  chips, *Discard* and *Send*). Subject and body are typed into local buffers and written back with
  one `update_draft` when typing pauses for a second, the field loses the focus, or before a key command (Ctrl+Enter
  sends what is on screen) — not per keystroke, which would flood the undo history; an agent's change to the draft
  reloads the buffers unless the user has unsaved typing. An address typed into To/Cc/Bcc (`Name <a@b>` or `a@b`)
  becomes a chip on Enter, a comma or leaving the field; invalid text stays in the field. While a recipient field is being typed in, up to 5 contacts that match
  the text (`listing::contacts`, computed once per state revision) are offered below it; a click adds one. The file dialog runs on its
  own thread and each picked path becomes `attach_file`; so does each file dropped on the window while a draft is open
  and editable (`egui` `dropped_files` with a path; *Drop the files to attach them* shows while files hover); attachment chips show `checking…`, the size or the error.
  Send is enabled once there is a recipient and every attachment is checked. A scheduled or running send makes the
  draft read-only with *Cancel send*.
- **Settings page** (UI-12, `app_gui::settings`): while `view.settings` is set it replaces the list and the reader;
  the sidebar and the top bar stay. A title with the version, tabs (Appearance, Keyboard, General, Accounts; a click
  runs `open_settings` with that section), then the section. Appearance: the mode as a segmented control and the
  light and dark themes as cards with a small preview (`set_theme`), density and text size (`set_setting`).
  Keyboard: the preset (`set_key_preset`), a filter, and every bound command by category with its first doc sentence
  and key caps; *Change…* opens the palette at `set_keybinding`. General: undo-send window, polling, notifications
  (a switch), undo history (`set_setting`). Accounts: a card per account with *Test*, *Re-enter password* and
  *Remove…* (asks once more, then `remove_account {confirm: true}`), and *Add account*. Every control sends a
  command; the page holds only a filter and the pending removal.
- **Frame** (UI-5): the sidebar at full height (brand row, *Compose* with its key, an *All inboxes* row with several
  accounts, per account a caption with its tag and the mailboxes with their role icons and the unread count or else
  the `g` key of the current account's role mailboxes, *+ Add account*, and *Settings* pinned at the bottom, which
  runs `open_settings`; collapsed, a 56 px rail of named icon buttons:
  expand, compose, the current account's role mailboxes, settings), then the top bar (the search pill, which runs
  `open_palette`; `/` still opens the palette at its search step), then the list and the reader. The sidebar and the
  top bar draw with the theme's chrome group. There is no status bar.
- **Status** (`app_gui::topbar` over `app_front::status`, a projection of `status`, the accounts and the outbox), in the
  top bar: a status dot (filled when synced, a ring while syncing, a crossed ring when a login failed: shape, not
  colour) and `syncing…` or `synced 2m ago` (the oldest `last_sync_ms` of all accounts), a button that runs
  `sync_now`; a status chip (`2 pending`, `1 pending · 1 failed`, the first problem and `+N`, `searching the
  server…`) whose popover lists `N pending · M failed` with the ops' errors on hover and *retry*
  (`retry_pending_ops`); `searching the server…`; store, keyring, login and sync failures in the
  error colour with a hint and their diagnostic on hover, and the fix `app_front::status` picks from the diagnostic
  (`Cause`: password, quota, TLS ciphers, certificate, network): *re-enter password* opens the wizard at its password
  step for that account (`Wizard::reenter` → `set_password`; another try keeps the account), *retry* runs `sync_now`
  for the account, *test again* `test_account`. The texts are a best guess from the servers' words; the hints say
  e.g. that Gmail needs an App Password, or that a TLS failure is dail's missing ciphers. A failed send shows its hint
  in the composer. The TUI shows the same hint in `?` and names the palette command in the status line; its setup
  re-enters a password the same way. The list footer shows the latest undoable action's `undo_label` with *Undo z*
  until an `undo`/`redo` goes through, else the key hints, and the number of conversations on the right.
- **Toasts:** `send_draft` shows *Sending in N s* with *Cancel send* until the window closes; rejections and runtime
  failures show the command's title and the error.
- **Desktop notifications** (`app_front::alerts`, delivered by `app_gui::desktop`): while the window is not focused,
  each new state is compared with the last frame's: unread mail that arrived in an INBOX fetched before (above its old
  `uid_next`, so neither a first sync nor a history backfill), a draft whose send failed, an agent's new approval
  request — at most one notification of each kind per change, off with the `notifications` setting. Senders and
  subjects are cleaned of control characters and cut to 120 characters. A helper thread hands them to the
  freedesktop notification service over D-Bus (zbus; the body escaped, since servers may read markup; each kind
  replaces its last notification) or to a WinRT toast on Windows (borrowing PowerShell's AppUserModelID until the
  installer registers one; XML-escaped). Clicking one does nothing yet; the TUI has none (it may run over SSH).
- **Attachments** (`app_gui::attachments`): below each body, a caption with the count and total size, then one card per
  attachment (an icon tile by type, the name, size and type) with the icon buttons *Save…*, *Preview* and, for PDFs,
  *Open ↗*, and *Save all…* by the caption (the shown, non-inline ones). The folder comes from `rfd`'s folder dialog on its own thread; the choice
  becomes `save_attachments {message, parts, dir}`. The runtime (`app_sync::saves`; `app-testkit` writes into its fake
  file system) writes each part under `app_io::mime::safe_file_name` (last path component, no control, bidi or
  Windows-forbidden characters, no leading dots, no reserved device names, ≤ 200 bytes) with `create_new`, so an
  existing file is never overwritten: the copy becomes `name (1).ext`. The message's entry in `status.saves` shows
  *Saving into …*, *Saved as …* or the error. PNG/JPEG/GIF/WebP and text attachments (not `text/html`) have
  *Preview*: on the first click `External::LoadPreview{blob, part}` → `Bridge::load_preview` reads the part
  (`SessionClient::attachment_part`, `app_io::mime::attachment_part`) and decodes it on a blocking thread
  (`attachments::preview_data`: `images::decode`, or the first 64 KiB as UTF-8), shown below the row until *Hide
  preview*. A PDF has *Open ↗*: `SessionClient::open_attachment` writes a private copy (`<tmp>/dail-originals/<uuid>-<safe
  name>`, 0600) that `webbrowser` opens in the browser's viewer. Other types are only saved: a generic "open with" would
  run attacker files in arbitrary programs, and an HTML file opened from disk could run its scripts. Previews and
  opening are presentation, like *open original*, not commands.
- **Several accounts:** the sidebar starts with *All inboxes* (`open_unified_inbox`, the inboxes' unread count) and
  tags each account with a colour (`Tokens::tag`, by its position); the list's rows carry their account's colour.
  The TUI titles the unified list *All inboxes*.
- **Checked threads:** Ctrl (Cmd) + click checks or unchecks a row, Shift + click checks every row from the cursor to it,
  `x` toggles the cursor's thread, `* a` checks all that the list shows, `* n` none. Checked rows show a check mark (the `check` icon in the GUI, `✓` in the TUI).
  While threads are checked the list header shows *N checked* with *Archive*, *Trash*, *Mark read*, *Mark unread*,
  *Star* and *Uncheck all*, and every triage key or palette command acts on the checked threads (`palette::targets`).
- **Conversation:** messages oldest first. By default the unread ones and the newest are expanded and the others are
  one line (sender, age, the start of the text; `listing::is_expanded`); a click on a collapsed line sends
  `expand_messages {ids: [id]}`, a click on an expanded message's header `collapse_messages` (in a thread of more than
  one). The palette's *Expand/Collapse all messages* send them with no `ids`. Quoted text and signatures are folded
  behind *··· Show quoted text* / *··· Show signature* (`fold` blocks of documents, `app_front::fold::split` of plain
  text); opening a fold is presentation, like scrolling (`reader::Folds`, per message and kind), not a command.
- **Reader** (UI-7): a 48 px toolbar outside the scroll area (*Reply* primary, *Reply all*, *Forward*; icon buttons
  named with their keys for star, read, archive, trash and *Move…*, which opens the palette at its step; on the right
  "n of N" with the previous and next conversation, as `k` and `j`, and close), the subject in `headline_xl`, a meta line
  of chips (the mailbox when it is not the inbox, the message count, the account with several), then the messages:
  an expanded one is a card with an initials avatar, the sender in `headline_md`, "to …" and the date in mono; a
  collapsed one is a line with a small avatar, the sender (600 when unread), the first line of its text and its age.
  Plain text bodies are `body_lg` (15/24), at most 720 wide. An HTML body is drawn from its `document` (`app_core::document`, built by `app_io::document`, §6;
  drawn by `app_gui::document`; h1 `headline_lg`, h2 `headline_md`, lower headings `body_md` at 600, quotes with a 2 px
  `border_strong` bar, preformatted text in a card): paragraphs, headings, lists, block quotes, preformatted text, rules and data tables,
  with bold, italic, code, strike-through and links. Each span is a label, so a link's text is clicked to open its
  target, whose full URL shows on hover. Images appear as their alt text; remote images are never loaded. A message's
  own inline images (`cid:` spans) are drawn once its sender is in `settings.images_from`: a bar above such a body says
  whether its pictures are hidden, with *Show images from …* / *Hide images from …* (`show_images_from` /
  `hide_images_from`). The shell's `images::Images` cache asks for each image once (`External::LoadImage{blob, cid}`);
  `Bridge::load_image` reads the part with `SessionClient::inline_part` and decodes it on a blocking thread
  (`images::decode`: the `image` crate, PNG/JPEG/GIF/WebP sniffed from the bytes, downscaled to at most 4096 px per side, 96 MiB, upright by EXIF),
  and the texture is drawn in place of the alt text. A plain-text body (or a front-end without documents, like the TUI)
  shows the text. "Open original" writes
  a sanitized HTML file to temp and opens the system browser.
  Below each body, *Links (N)* lists every `http(s)://` and `mailto:` target found in the text (`app_front::body::links`; HTML
  links arrive as numbered footnotes with their real targets), each shown in full and opened in the browser only when
  clicked. Each downloaded message has *Open original ↗*: `Session::open_original(blob)` renders the stored raw
  message with `app_io::original::original_html` and writes it to a private temp file (`<tmp>/dail-originals/<uuid>.html`,
  0700 dir / 0600 file on Unix), which the bridge opens with `webbrowser`; failures come back as bridge notices and
  show as toasts. Opening a link or the original is a local presentation action, not a `Command` (nothing changes in
  state, and an agent reads bodies with `get_message`); the shell reports both as `External` intents that `DailApp`
  carries out after the frame.
- **Account wizard** (first run without accounts, *+ Add account*, or `add_account` in the palette): preset (Gmail —
  App Password —, Hostinger, other) → address, optional name and, for custom servers, `host:port[:tls|starttls]`
  (`Endpoint::parse`, shared with `dail account add`) → password (masked) → `add_account` with the password as a
  write-only `Secret` (the field is cleared at once), then `test_account`; the account's `status` drives the last step.
  Verified: *Sync now* (`sync_now` for the account). Failed: the error, *Try another password* (removes the half-added
  account with `remove_account {confirm: true}` and goes back) or *Keep it anyway*. For custom servers, *Find
  servers* runs `discover_servers` for the address and fills both fields from `status.discovery` (the note says where
  they were found, or that they must be typed); the TUI's setup does the same with Ctrl+F.
- **Approvals tray** (`app_gui::approvals`, cards from `app_front::approvals`): while `state.approvals` is not empty, a box at the top of the main pane
  (above the reader or composer) shows one card per request, oldest first: subject, sending account, every `To:`/`Cc:`/`Bcc:`
  address in full, the first 4 lines (at most 280 characters) of the text and every attachment with its full path and size
  or state. *Approve send* → `approve_request` (disabled while an attachment is being checked), *Edit* → `open_composer`
  for the draft (the user's edits keep the request), *Reject* → `reject_request`. The text is plain labels, never
  interpreted. `approve_request`/`reject_request`/`list_approvals` are also in the palette (picker over the requests).
- Frame discipline: render from `&AppState`, push into `Vec<Command>`, drain to `session.submit()` at the end of the frame.
  The Session pushes `StateChanged` and the GUI calls `ctx.request_repaint()`.
- Shell (`app_gui::run`, `dail gui` or no subcommand): the `Session` lives on a background tokio runtime behind a `Bridge`.
  Each frame, `DailApp` takes one `Arc<AppState>` snapshot and the replies that arrived, runs `Shell::frame(ui, &state,
  replies) → Vec<Request>` and hands the queue to `Bridge::submit`. `app_gui::Bridge` wraps the shared
  `app_sync::Bridge<Purpose>` (§5.4) with `Origin::Gui` and adds *open original*; each reply comes back as
  `Reply { purpose, name, result }` (`Purpose::User`: toasts; `Purpose::Background`: the UI's own queries, e.g. reader
  bodies). State changes and replies call `request_repaint`. Closing the window closes the bridge; the task then shuts
  the session down (unsaved drafts are saved) before `run` returns. `Shell` never sees
  the session, so tests drive it headlessly (`app-gui/tests`, `egui_kittest`: widgets are found by their AccessKit
  label and clicked; a real `Engine` stands in for the session). vsync is off (`DAIL_VSYNC=1` turns it on): egui
  repaints only on demand, and on some Wayland compositors eglSwapBuffers blocks the UI thread otherwise.
- Layout: a resizable sidebar (accounts and their folder trees from `app_front::mailbox::tree`, indented per level,
  unread counts; collapsed
  to a rail by `toggle_sidebar`), the thread list (`listing::visible_threads`: the `view.search` results, else the open
  mailbox, newest first; rows from `listing::ThreadSummary`, laid out only while visible), and the
  reader (toolbar acting on the open thread, one card per message, body text). The list has a 48 px header (title, the
  unread badge, *Clear* during a search, the checked-threads bar), hover actions on a row (UX-10: *Archive*, *Trash*, *Star*/*Unstar*, *Mark read*/*Mark unread* as icon buttons named
  `<action> conversation`, also on the keyboard cursor's row; its context menu has the same plus *Move to…* — the
  palette's target step, `palette::move_to` — and *Focus this mailbox*), rows drag onto the sidebar's folders (UX-11: the row's payload `list::Dragged{account, ids}` — every checked thread
  if the row is checked — and a folder of the same account outlines itself while hovered and takes it with
  `move_threads`; parents that can't be opened take nothing; dragging out to the OS stays unsupported), date group headers (UX-13: `app_front::format::date_group` — *Today*, *Yesterday*, *This week* since Monday, *Earlier
  this month*, then *November 2023*, UTC days — as heading rows that take one row slot each, so `show_rows` stays
  uniform; the densities are `list_density`'s), the toolbar under it (UX-7: tri-state *Select all*
  box — `check_threads{}` / `uncheck_threads{}` —, the *Unread*/*Starred*/*Attachments* filter chips, and for a mailbox
  *Mark all as read* and *Refresh* (`sync_now` for its account; a spinner while it syncs or the mailbox is fetched)) and the footer; its rows come in three densities
  (`list::Density`): cozy, 48 px on two lines (check box, account tag, unread pip, the sender in 600 when unread, star,
  clip, message count and the date in mono; then the subject), compact, 40 px on one line (the sender in a fixed
  column), and preview, 72 px on three lines (the snippet on the third). Cozy and compact rows put the newest message's
  snippet, muted, after the subject (UI-14). Hover and selection fills, the keyboard cursor's 2 px bar and the hairline come from `ui::row_frame`. The
  list's rows and its unread count are computed once per `AppState.revision` (`list::RowCache`), so a frame with 100k
  threads costs ~0.7 ms. `app_core::listing` holds these read-only
  projections; the queries use the same functions, so the GUI never re-derives the rules. Bodies are blobs only the
  session can read: the reader sends a background `get_thread {include_body: true, rich_body: true}` whenever the open
  thread or one of its body states changes. A user `create_draft`/`reply`/`reply_all`/`forward` is followed by `open_composer` for the
  new draft. Look (`docs/ui/monochrome_minimalist_mail/DESIGN.md`, ADR 0008): tonal surfaces, hairline borders,
  inversion for emphasis. `app_gui::theme` is the egui adapter of `app-theme`: a `Theme` becomes `Tokens` (the content
  roles as `Color32` through `Deref`, `chrome()` for the sidebar's group, the account `tags`, the metrics `m`, the
  `type_scale`) and egui `Visuals` once per change; the sidebar sets the chrome group's `Visuals` on its `Ui`. The kit
  `app_gui::ui` (buttons, key caps, badges, pips, status dots, avatars, nav rows, row frames, cards, fields, the search
  pill, segmented controls, switches, tabs, toolbars, overlays, toasts, footer hints, empty states) is what widgets
  draw with; `cargo run -p app-gui --example gallery [-- --theme <id>]` shows all of it per theme, content and chrome
  side by side. The style check (`tests/it/style.rs`) fails on raw colours, fonts, sizes, radii, margins, spaces or
  strokes outside `ui/` and `theme.rs`; its allowlist is empty since UI-10. Overlays use the kit too: toasts on
  `inverse_bg` (errors in `inverse_danger` with an *Error* icon, so colour is not the only signal), the approvals tray
  as a `border_strong` callout, the account wizard and the query window ("READ-ONLY · UNTRUSTED DATA") as `overlay`
  frames over the scrim; menus and tooltips get their look from the adapter's `Visuals`.
  Themes are chosen by name (`set_theme`): `theme` is the mode (0 follows the system, 1 light, 2 dark) and
  `theme_light` / `theme_dark` (defaults `dail-light`, `dail-dark`) name the theme of each mode; an unknown name falls
  back to the mode's default with a notice. The legacy values 3 and 4 are mapped when `config.toml` is read and when
  `set_setting theme` runs: 3 is dark mode with `dail-dark-hc`, 4 light mode with `dail-light-hc`. `list_density` picks
  cozy (0), compact (1) or preview (2) list rows. The `text_scale` setting (percent)
  sets egui's zoom factor; egui's own Ctrl + plus/minus/0 zoom lasts until the window closes (written back, every step
  would become an undo step). Screen readers (AccessKit): thread rows and mailbox rows are buttons named by what their
  marks show (`unread, starred, with attachments, Carol, Lunch?, 2 messages, Nov 14`; `Inbox, 3 unread`), the open
  mailbox pressed, their parts as children; icon buttons carry their tooltip as name; form fields are labelled by their
  captions and Tab follows the reading order. The TUI follows the terminal's colours and font. Dates are shown
  in UTC (`app_core::date`, the calendar math shared with the core's quote headers and the audit log's file names).
  `cargo run -p app-gui --example demo [open] [compose] [agent] [palette] [light|dark]` opens it on the memory
  backend (`agent`: an agent has asked to send a reply).

### 7.2 CLI & batch (`app-cli`)
- Output is JSON lines (`--pretty` for humans): a `CommandReceipt` per command, or
  `{"ok":false,"name":…,"error":{"error":"<variant>",…}}` for a rejection. Batch results add `"line": n`.
- Dry runs: `dail exec <cmd> --args … --dry-run`, or `"dry_run": true` on a batch line, print the `Preview` instead
  (§4) and change nothing.
- Exit codes: `0` success, `1` a command was rejected, `2` usage/runtime error (message on stderr). `batch` stops at
  the first rejected command unless `--keep-going` is set (exit code is then still `1` if any failed).
- Batch file format: one `Command` JSON per line (`{"cmd":"mark_read","args":{"ids":["…"]}}`). `#` starts a comment.
  A missing `args` means `{}`. `-` reads stdin, streaming one result per line. After each command `batch` waits for its
  effects (`Session::flush`), so a line sees the results of earlier ones (e.g. `sync_now`, then `list_mailboxes`).
- `--backend memory [--fixture <name|path.json>]` runs fully offline against `app-testkit` (ADR 0001). `--backend live`
  is the default: `LiveBackend` over the profile chosen by `--profile <name>` (default `default`) or `--profile-dir <dir>`,
  with `config.toml` + `state.redb` + `blobs/` (`ProfileStore`), the OS keyring and real IMAP/SMTP.
  `--backend`, `--fixture`, `--profile`, `--profile-dir` and `--pretty` are global flags. Every subcommand attaches to
  a running owner of the profile first (§5.4); `--backend memory --profile-dir <dir>` makes a memory session attachable.
- `dail account add --preset <hostinger|gmail|custom> --email <addr> [--display-name …] [--imap host:port[:tls|starttls]]
  [--smtp …]` dispatches `add_account`, then `test_account`, waits for the result and prints a third line
  `{"ok":…,"name":"account_status","id":…,"status":{…}}`; exit code `1` if verification failed. Without a suffix, ports
  143/587 use STARTTLS, others TLS. The password comes from a no-echo prompt or, with `--password-stdin`, from the first
  line of stdin — never from argv. If stdin is not a terminal and `--password-stdin` is missing, it exits `2`.
  `--no-password` adds the account without a secret; `--no-test` skips the test.
- `dail doctor` checks the profile and prints one line per check, `{"check","status":"ok|warn|fail","detail"}`
  (exit code 1 on a failure): version, profile, the TLS client, then — holding the profile's lock — `config.toml`
  parses, `state.redb` opens and passes redb's integrity check (with record counts), downloaded messages and
  unfinished writes, the index, backups, set-aside broken files, and per account the login status and whether its
  password is in the keyring (looked up, never printed). With dail running on the profile it reports the running
  process's state instead (accounts, store/keyring/sync errors) and says to close it for the file checks. Not a
  registry command: like `replay` and `schema`, it inspects the installation rather than the mail.
- `dail account password --email <addr> [--password-stdin]` dispatches `set_password` with a password read the same
  way, waits for the login test and prints `account_status` like `add`; exit code `1` unless verified.
- `dail exec --help` lists the registry; `dail schema --command <name>` prints one spec (with `kind` and `agents`) and its
  args schema.
- `dail replay <log> [--state]`: see §6 (audit log).

### 7.3 MCP (`app-mcp`)
- `rmcp` server over stdio (`dail mcp [--backend memory]`). Tools are generated from the registry: name = `CommandSpec.name`,
  title = `CommandSpec.title`, description = the variant's doc comment, input schema = `CommandName::args_schema()`,
  annotations from `CommandKind`:

  | Kind | readOnly | destructive | idempotent | openWorld |
  |---|---|---|---|---|
  | Query | true | false | true | false |
  | View | false | false | true | false |
  | Mutation | false | false | false | true |
  | Outbound | false | false | false | true |
  | Destructive | false | true | false | true |

- Calls are submitted with `Origin::Mcp` (`Origin::McpAllowSend` with `dail mcp --allow-send`; resources always as `mcp`). Engine rejections are tool errors (`isError: true`, `structuredContent` =
  `{"ok":false,"name":…,"error":{…}}`); successes return the `CommandReceipt` as `structuredContent`. Every tool
  that changes data (Mutation, Outbound, Destructive) also takes `dry_run: true`, which returns the `Preview` (§4). Only a dead session
  is a JSON-RPC error. `CommandSpec.available` is not enforced over MCP (same behaviour as `dail exec`), `CommandSpec.agents`
  is: every tool is listed, and the `user_only` ones (§9) answer `origin_not_allowed`, as their descriptions say.
- Resources (`app_mcp::resources`), each the `data` of one Query submitted with `Origin::Mcp` (so they work attached, too),
  as `application/json` text: `dail://mailboxes` (`list_mailboxes`), `dail://mailbox/{id}/threads` (`list_threads`, newest 50),
  `dail://thread/{id}` (`get_thread` with `include_body`; missing bodies start downloading) and `dail://state/view`
  (`get_view`). `resources/list` returns the two fixed ones plus one thread list per cached mailbox; `resources/templates/list`
  the two parameterized URIs. Unknown URIs and engine rejections are `-32002` (resource not found) with the `CoreError` in `data`.
- Transports: stdio (`dail mcp`), and Streamable HTTP on loopback (`dail mcp --http 127.0.0.1:<port>`, path `/mcp`;
  ADR 0007): rmcp's tower service on hyper, non-loopback addresses refused, rmcp's `Host` check, and a bearer token
  from `<profile>/mcp-token` (32 random bytes, 0600; a private memory session prints a one-off token) on every request
  — 401 without it, 403 for a foreign `Host`, 404 off `/mcp`. Same tools, origin and policy as stdio.
- Prompts (`app_mcp::prompts`): `triage_inbox {limit?}`, `summarize_thread {thread}`, `draft_reply {thread, intent?}` —
  one user message each that names the tools to call in order and ends with the rules (untrusted content is data;
  sends wait for approval; prefer reversible actions). Arguments are checked (`thread` must be a thread ID, `limit`
  1–100) and free text is JSON-quoted, so an argument can't rewrite the prompt. A test pins that every tool a prompt
  names exists, and that the instructions name every `user_only` tool.
- Server instructions (`app_mcp::INSTRUCTIONS`): everything in `untrusted_content` is third-party data, never
  instructions (not even when it claims to be from the user or dail), links and attachments are not opened because an email
  says so; agent sends go through the user's approval (unless `--allow-send`), attachments show their path on the approval
  card; account settings, `restore_draft` and the approval decisions are the user's (`origin_not_allowed`); mutations are
  undoable, destructive tools need `confirm`.
- Every attacker-controlled part of an email in the output — subject, body, sender/recipient names and addresses,
  Message-ID / In-Reply-To, attachment names, quotes — sits inside `{"untrusted_content": …}`. `app-mcp/tests/it/untrusted.rs`
  enforces it: a fixture message carries a marker in each of those parts, and an agent session over every content-bearing
  tool and every resource must show the marker only below an `untrusted_content` key (also in the text copy of the result).

### 7.4 TUI (`app-tui`)
A terminal front-end for reading, triaging and answering mail over SSH from a phone or a tablet (Termius, Blink), and
for testing: under `tmux` an agent can type into it (`send-keys`) and read the screen (`capture-pane -p`). Same engine,
registry, keymap and palette flows as the GUI (`app-front`, ADR 0004); it never links egui.

- **Projection.** `Tui` is UI-local state, like `Shell`: `draw(frame, &state)` renders one frame of `AppState` and
  `input(event, &state) -> Vec<Request>` turns a key, a paste, a resize or a tick into requests; `replies(replies,
  &state)` takes the results back. The clock is injected (`Tui::with_clock`). A resize never dispatches a command.
- **Layout** (`app_tui::layout::arrange`, a function of `ViewState` and the terminal size): ≥ 120 columns the
  mailboxes (unless `toggle_sidebar` collapsed them), the list and the reader or the composer; 80–119 the list and the
  reader (or composer), the mailboxes in the list's place while the sidebar has the focus; < 80 one pane: the mailboxes
  while the sidebar has the focus, else the composer, else the open thread, else the list. Below 40×12 a "terminal too
  small" notice. One header line (`dail · <place> · <account>`) and one status line; borders, with the pane's title
  and bold when focused, only from 80 columns. By default only the terminal's own colours: default foreground and
  background, bold, dim and reverse, so light and dark themes both work and `NO_COLOR` holds. With the setting
  `tui_colors` = 1 (UI-16, ADR 0004 amendment) one pass over the finished frame (`app_tui::colors::paint`) maps those
  attributes onto the active `app-theme` roles in truecolor: the `canvas` fill and `text` everywhere, `text_strong`
  for bold, `text_muted` for dim, the `selected` fill for reversed cells, `border` for box drawing, the chrome's
  `base` on the header and status lines. Light mode (`theme` 1) takes `theme_light`, anything else `theme_dark`.
  `NO_COLOR` (set and not empty) still wins.
- **Mailboxes and list.** The mailboxes pane lists each account (bold) and its folder tree (`app_front::mailbox::tree`:
  role names, two spaces per level, `▾` on an expanded parent, `▸` on a collapsed one or account (with `+n` unread hidden), parents that can't be opened dimmed, unread counts), the open one
  marked `›` and reversed. The thread list is `listing::visible_threads` with rows
  from `ThreadSummary`: a cursor gutter `›`, unread `●`, starred `★`, attachment `@`, sender, subject and date
  (`app_front::format::short_date`); unread rows bold. The cursor is `view.selected` (else the open thread), as in the
  GUI, shown reversed and with `›` (so `tmux capture-pane` sees it too); the list scrolls as little as possible to
  keep it visible.
- **Reader** (`app_tui::reader`): the open thread, oldest message first; a collapsed message (`listing::is_expanded`,
  §7.1) is one line `▸ From · age · text`, the others come with `From`, `To`, `Cc`, `Date` (and the
  folder outside the inbox), the text, *Attachments (N)* and *Links (N)* with their full targets — wrapped, never cut
  (`text::wrap`, by display width). Bodies come from the shared body tracking: before each frame `Tui::refresh` returns
  the background `get_thread {include_body: true}` (`Purpose::Bodies`) when the open thread or a body state changed.
  Space/PageDown and `b`/PageUp page (one screen less a line), ↑/↓ scroll when the reader is the only pane or has the
  focus; `j`/`k` still move between threads. A new thread starts at the top. No browser and no "open original": over
  SSH they would open on the host.
- **Keys** go through the shared keymap (`app_front::keymap`): `app_tui::keys::chord_of` maps crossterm's key events
  onto chords (a typed symbol is a `Text` chord, Ctrl+Shift+letter arrives as Ctrl+letter, BackTab is Shift+Tab), so
  `j k Enter o e # s u v z / Tab Escape c r a f`, the `g` sequences and the arguments from the view work as in the
  GUI; a command that still needs an argument opens the palette at that step. With the mailboxes focused, `j`/`k` open
  the next or previous mailbox and Enter goes back to the list (shared, so the GUI does the same). A new draft opens in
  the composer. TUI-local keys only scroll (Space, `b`, PageUp/PageDown, ↑/↓ in a lone or focused reader), stand in for
  Escape outside text fields (`q` and Backspace; from the mailboxes they go back to the list; on the top level `q`
  quits), show help (`?`) or quit (Ctrl+C); besides those, `m` turns mouse capture off and on, Ctrl+O in the
  composer opens `$EDITOR`, and Enter on the empty first-run screen opens the account setup. A paste outside a text
  field is dropped, never run as keys.
- **Settings** (`app_tui::settings`, `g ,`): while `view.settings` is set, the sections General, Keys and Accounts
  as lists of keys and values over the panes (no appearance: the TUI follows the terminal; `appearance` shows
  General). Tab moves between sections (`open_settings`), `j`/`k` between rows, Enter opens the palette at the step
  that changes the row (or tests the account), Esc runs `close_settings`.
- **Palette** (`app_tui::palette`, Ctrl+K or `:`): the shared `app_front::palette::Palette` drawn over the panes — the
  whole body below 80 columns, a bordered box from 80 — with the prompt of a second step, the query and the rows
  (title, detail from 60 columns, and the command's key that a plain terminal delivers, `keys::hint`). ↑/↓ (Ctrl+P/N,
  PageUp/PageDown) move, Enter chooses, Esc steps back or closes, Backspace edits and on an empty query stands in for
  Esc, Ctrl+U clears; a paste goes into the query. `add_account` opens the account setup. Query results
  (`get_settings`, `search`, …) open in a scrollable read-only view (`ResultView`) marked as untrusted data: the
  `app_front::result` document as text (Tab / Shift+Tab move between the rows that open something, Enter opens one),
  `r` for the raw JSON; closed with `q`, Esc or Backspace. `app-tui/tests/it/palette.rs` walks every registry command through the TUI palette by
  keys (the TUI half of the palette parity).
- **Status line** (`view::status`, a projection like the GUI's bar, fitted to the width, most important first): the
  countdown *Sending in N s · z cancels* of a send this TUI scheduled, the latest notice (a rejected command's title
  and error, or a runtime failure, for 6 s — the GUI's toasts), the first problem (store, keyring, login, sync;
  `app_front::status::problems`, `+N more`), `N pending · M failed (retry: palette)`, the latest undoable action
  (`app_front::notice::LastAction`) with `z undo`, and the sync state; `? help` on the right when there is room.
  `?` opens a read-only view with the full text of the current problems, every registry key a plain terminal
  delivers with its command's title, and the TUI-local keys.
- **Send** (Ctrl+S, or *Send* in the palette) writes typed text back first, then `send_draft`: the status line counts
  *Sending in N s · z cancels* down while the draft is read-only (`z` undoes the send, `cancel_send`). *Attach file*
  in the palette takes a typed path (`~/` is the home directory; there is no file dialog: over SSH it would open on the
  host); the composer lists each attachment with its full path and `checking…`, its size or its error. Quitting (`q`,
  Ctrl+C) a TUI that owns the session while a send is in its undo window asks first — the status line becomes the
  question, `q` or Ctrl+C again quits, any other key stays — because closing cancels the send (§4.4); an attached TUI
  quits at once and the owner keeps the send. A hang-up can't ask: the send is cancelled with a `send_error` that the
  draft shows on the next start.
- **Agent send requests** (`app_tui::approvals`): while `state.approvals` is not empty, a reversed banner line below
  the header (*An agent wants to send …* — Ctrl+K, *Show send requests*). `list_approvals` from the TUI opens the
  requests view instead of its JSON: each request in full from `app_front::approvals::cards` (subject, sending
  account, every `To`/`Cc`/`Bcc` address, the first lines of the text, every attachment with its full path and its
  size or state), `j`/`k` between them; `a` approves after a confirmation (`y`; anything else keeps it, and nothing
  is offered while an attachment is being checked), `e` opens the draft in the composer (the user's edits keep the
  request), `r` rejects. Approving schedules the send with the undo window like the user's own (`Origin::Tui`).
- **Untrusted text** (`app_front::text`, re-exported by `app_tui::text`): every widget draws email content and an agent's draft text through one
  sanitizer, `clean` (one line) or `clean_lines` (a body): C0 and C1 controls (ESC among them), DEL and the bidi
  embeddings, overrides, isolates and marks become U+FFFD, tabs are expanded, line breaks in a line become spaces.
  No OSC 8 links and no window title are ever written. `app-tui/tests/it/untrusted.rs` puts `ESC ]52;…`, `ESC [2J`,
  `\r`, a C1 CSI and U+202E into a subject, names, addresses and a body (and an agent's draft) and checks the buffers
  of the list, the reader, the palette, a query's result and the approvals at the three test sizes.
- **Composer** (`app_tui::composer`, for `c`/`r`/`a`/`f` and `open_composer`): the title and the draft's state
  (`app_front::compose::draft_status`), errors, From, To/Cc/Bcc (Cc and Bcc only when filled or under the cursor),
  Subject, the body in a `ratatui-textarea`, and below it the quote's header (read-only) and the attachments. Tab and
  Shift+Tab (↑/↓ outside the body) move between the fields. An address typed into To/Cc/Bcc (`parse_addr`) is added
  on Enter, a comma or leaving the field; invalid text stays; Backspace on an empty field removes the last address.
  While text is typed into a recipient field, a dim line under it shows the best matching contact
  (`listing::contacts`, kept per state revision), and Tab adds it (without a match Tab moves on).
  Subject and body go back as one `update_draft` when typing pauses for a second (`Tui::refresh`), the cursor leaves
  the field, or before a key command; an agent's edit reloads the fields unless there is unsaved typing. While the
  composer has the focus the registry's Escape and Ctrl chords act first (Ctrl+S sends what is on screen, Ctrl+K
  opens the palette, Esc closes the composer; Ctrl+A, C, V, X, Y and Z stay the editor's), then the editor gets the
  key; a paste goes into the field. Untrusted text loaded into the editor (an agent's draft) has its unsafe characters
  replaced. A scheduled or running send makes the draft read-only.
  Ctrl+O (in the footer) hands the body to `$VISUAL`, else `$EDITOR` (a program and its arguments): typed text is
  written back first, the TUI asks the runner (`External::EditBody`, `Host::edit`), which stops the input thread
  from reading (a pause flag, then the gate it holds while polling), leaves the alternate screen and raw mode, writes
  the body to a private temporary file (`app_tui::editor::PrivateFile`: a new 0700 directory with one 0600 file, both
  removed afterwards), runs the editor, and takes the terminal back. The saved text comes back as one `update_draft`
  (`Tui::edited`); a failed editor or no editor at all is a notice.
- **Session.** `app_tui::run(runtime, Target, tui)` connects `Tui` to an `app_sync::Bridge<Purpose>` with `Origin::Tui`:
  `Target::Own(session)` shuts the session down when the TUI ends, `Target::Attach(conn)` leaves the owner running.
  `Purpose::User` results go to the status line; `Purpose::Bodies(thread)` feeds the reader.
- **`dail tui`** (app-cli; the global flags apply) attaches to the profile's owner (the GUI or `dail mcp`, origin
  `tui`), or owns the profile and listens on its socket, so `dail exec`, `dail mcp` and `dail account add` attach to
  it. An attached TUI whose owner goes away says so on stderr and exits with code 2. Without accounts it offers the
  account setup (Enter) and shows the `dail account add` line for this profile (with
  `--profile`/`--profile-dir`/`--backend memory` as given) to run in another shell instead; the account then appears
  live. Plain `dail` starts the TUI instead of the GUI on Linux when neither
  `DISPLAY` nor `WAYLAND_DISPLAY` is set and stdin and stdout are terminals. In TUI mode `tracing` writes to
  `<profile>/logs/tui.log` (0600; nothing for a private memory session), because stderr is the screen; without a
  terminal `dail tui` exits with code 2.
- **Terminal lifecycle** (`app_tui::run`): raw mode, the alternate screen and bracketed paste, restored when the TUI
  ends, on an error and on a panic (a hook that restores first). One queue (`Event`) carries keys, pastes and resizes
  from an input thread, the bridge's wake-ups (state changes, replies), signals, and — while a countdown or an age is
  on screen (`Tui::wants_tick`) — a 1 s tick; the loop (`event_loop`, also driven by tests) takes every queued event
  and draws once per batch. A dead terminal (input fails, or drawing does), SIGHUP or SIGTERM (the console's close
  event on Windows) end it like quitting (`Exit::Hangup`): the bridge closes and an owned session shuts down, saving
  unsaved drafts. The input thread is not joined, because crossterm's read may never return on a dead terminal.
- **Account setup** (`app_tui::setup`, stretch; Enter without accounts, or *Add account* in the palette): preset →
  address, optional name and, for custom servers, `host:port[:tls|starttls]` (`Endpoint::parse`) → the password,
  drawn as `•` and moved straight into `add_account`'s write-only `Secret` (the field is empty afterwards) → the
  setup's own `test_account` (`Purpose::Setup`), whose result arrives in the account's `status` → Enter runs
  `sync_now` for the account. After a failed login, `p` removes the half-added account (`remove_account {confirm:
  true}`) and asks for another password; `k` keeps it. Over a plain SSH login the keyring may be out of reach (see
  the Wave 1.5 known limitations in `ROADMAP.md`): the keyring error shows in the status line.
- **Mouse and touch** (stretch): mouse capture is on from the start, so a phone's taps and swipes arrive as mouse
  events. A tap on a thread row selects it, a second tap opens it; a tap on a mailbox opens it; the wheel (a swipe)
  scrolls the list (without moving the selection; the list follows the cursor again once it moves) or the reader, and
  a read-only view. `m` turns capture off and on (`External::MouseCapture`): off, the terminal's own selection can copy
  a link. The last frame's hit areas map positions to rows.
- **Attached end to end** (`app-tui/tests/it/attached.rs`): a `Tui` over an `app_sync::Bridge` attached through an
  `IpcClient` to a running session sees an agent's archive (another IPC client, origin `mcp`) through the change
  notifications — it waits on the bridge's wake-ups, never polls — shows the agent's send request in the banner and
  the requests view, and its approval goes through with origin `tui`: the owner's session schedules the send.
- **Tests.** `app-tui/tests` draw `Tui` on ratatui's `TestBackend` over a real `Engine` (requests dispatched with
  `Origin::Tui`, replies fed back) and snapshot the screen with `insta`. `cargo run -p app-tui --example tui-demo [open]
  [compose] [agent] [palette]` opens it on the memory backend.

### 7.5 Android (`app-mobile`, `app-android`, ADR 0009)

The touch presenter. `Mobile` is UI-local state like `Tui`: it reads `&AppState`, never changes it, and holds no
toolkit, tokio or JNI. The Android host (`app-android`, below) submits its requests through an `app_sync::Bridge`
with `Origin::Gui` and hands its frames to Compose as JSON.

- **API.** `frame(&state) -> Frame` (the next screen), `event(UiEvent, &state) -> Vec<Request>` (what to submit),
  `refresh(&state) -> Vec<Request>` (bodies for the open thread, before each frame), `replies(Vec<Reply>, &state) ->
  Vec<Request>` (results; a new draft opens in the composer) and `externals() -> Vec<External>` (what only Android
  does: `haptic`, `pick_files {draft}`, `import_files {draft, uris}`, `sending {draft}`, `open_url {url}`, `copy {text}`, `save_files {blob, files}`, `schedule {sync, interval_mins}`). `Reply` is `app_sync::Reply` with the runtime error as its message, so the crate needs no
  app-sync. The clock is injected (`Mobile::with_clock`).
- **Frames** (`app_mobile::frame`, `Serialize + JsonSchema`): `revision`, `layout` (window class and panes),
  `top_bar` (title, sync state, the drawer or back button, search), `drawer` (*All inboxes* with two accounts or
  more, each account with its colour tag — an index into the theme's `tags` — and its mailboxes with role icons and
  unread counts, and a footer: *Settings* = `open_settings`, *Add account* = `open_settings accounts`; `modal` on
  compact and medium windows, a permanent first pane on expanded ones, a rail there after `toggle_sidebar`), `list`,
  `reader` (below), `link`, `composer` (below), `settings` (below, in place of the list and the detail), `setup` (below), `fab` (compose), `banner` (the
  first `app_front::status` problem and its fix; with no problem, news with `info: true` and a second `dismiss` button:
  the first-run question about update checks, or a new version with *Open* = the release page and *Skip*), `snackbar` (a send counting down with Undo = `cancel_send`, a
  rejection, or the latest undoable action with Undo for 6 s), `theme`, `about`, `palette`, `result` (below), `back`
  and `wants_tick`.
- **Settings** (`mobile/settings.rs`). `settings` while `view.settings` is open: the four `open_settings` sections as
  `tabs` (*Appearance*, *Keyboard*, *General*, *Accounts*), then the open section's `groups` of rows. A row has a
  label, its value and `choices` (segmented buttons: a tap is `set_setting`, `set_theme` or `set_key_preset`, so no
  number is typed) or `actions`. Appearance: the mode, the light and dark themes of `app_theme::builtin`, the list
  density. Keyboard: the key preset, *Change a key…* (the palette at `set_keybinding`), and every binding that Android
  delivers. General: the undo window, the poll interval and the first fetch, and the phone's *Background &
  notifications* group (`background_sync`, the interval unless 0, `notifications`, and *Battery optimization* and
  *Notification settings*: `External::SystemSettings`). It is a group under General and not a fifth section, so the
  desktop's tabs stay as they are. Accounts: each with its status and *Test* (`test_account`), *Password* (the setup's
  password form), *Remove* (the palette's confirmation step, `app_front::palette::remove_account`), then *Add an
  account*. `text_scale`, `tui_colors` and `history_cap` are the desktop's and not shown.
- **Account setup** (`mobile/setup.rs`), the GUI's wizard and the TUI's: provider → address (the servers for a custom
  one, *Find servers* = `discover_servers`, whose answer fills them through `status.discovery`) → password →
  `add_account` (purpose `Setup`) → `test_account` → *Sync now* (`sync_now` for the account). `setup` is a sheet over
  the panes; Back goes a step back, then closes it. Its fields are `Field::{Email, Name, Imap, Smtp, Password}` with
  an `InputKind` that Kotlin maps to the keyboard and to autofill's content types (email address and username,
  person name, password). The password arrives in `text` events and moves into the `Secret` of the command at once;
  no frame carries it (its `TextValue` is always empty, its version changes to clear the host's field) and the
  setup's `Debug` leaves it out. A failed test offers another password and removes the half-added account first. It
  opens from the palette and keys (`add_account`, `set_password`), *Add an account*, the top bar's *Add account*
  without accounts, the banner's *re-enter password* fix (`Fix::Password`), and on its own once on a profile without
  accounts.
- **Theme** (`app_mobile::theme`, ADR 0008). `theme` comes in the first frame and again whenever it changes: the
  theme the settings choose for the system's dark mode (`configure`'s `dark`), as the GUI chooses it (`theme` 0
  follows the system, 1 light, 2 dark, the legacy 3 and 4 the high-contrast pair; an unknown name falls back). It
  carries the content and chrome colour roles as `#RRGGBBAA`, the tags, the type scale, the metrics and the icon
  font's glyph per icon name (Kotlin draws a button with an icon from it, and its label only where the font has no
  glyph: the list's *Mailboxes* button is the `menu` glyph, named *Mailboxes* for TalkBack, MISC-2). The drawer's *About* (MISC-3: the entry after *Settings*, icon
  `info`; without accounts, there being no drawer, the top bar's) opens `about`, whose top bar has a *Close* button (`close`,
  as Back) and over which a modal drawer gives way (the version, the bundled fonts' and icons' licences, from
  `app_theme::fonts` and `icon`, and the lists in `crates/app-mobile/licences/`: the Rust crates `libdail.so` links
  and the Android libraries of the release APK, each `name version SPDX-licence`); Back closes it, and no command runs. Every string from mail goes through `app_front::text::clean`.
- **Window classes** (Material's, by width): compact < 600 dp, medium 600–839, expanded ≥ 840, mapped onto
  `app_front::panes::Width` (narrow, medium, wide): one pane as the TUI below 80 columns, list and detail, drawer list
  and detail. The class never dispatches a command. A landscape window that is not compact always shows the sidebar
  (MISC-6). There and on an expanded window, the drawer's `style` button runs `toggle_sidebar`: the full tree, or
  the icon rail while `view.sidebar_collapsed` (persisted in `[view]`, MISC-11). A pinch on the message (`pinch{zoom_in}`, MISC-6) steps the
  presenter's `pane_step` (the reader's `maximize` button, MISC-14, jumps to the last step and back, and leaves a bare
  top bar): zooming in hides the sidebar, then the list; zooming out brings them back, one step per
  gesture; the text size never changes and the step stays across threads. Kotlin's drawer sheet and rail take no window
  insets of their own (the Scaffold's padding has them), which kept the landscape drawer too wide and misaligned.
- **Actions.** Everything tappable carries an action ID from the frame's table (`Tap`, `LongPress`, `Swipe`). IDs only
  grow, and the table holds the last frame's, so an ID from an older frame is ignored, never taken for another action.
  So a callback that outlives a frame (a remembered swipe state's `confirmValueChange`) must read the current frame's
  row through `rememberUpdatedState`, never the one it was created with (MISC-1). Kotlin never builds a command. A row's tap opens its thread (toggles its check while threads are checked), a long
  press checks it (with `haptic`), a swipe to the start archives and to the end trashes (with `haptic`). While
  threads are checked, `bulk` replaces the top bar: `N selected`, archive, trash, mark read, mark unread and star over
  `palette::targets`, and a close button (`uncheck_threads`). Kotlin draws rows in a `SwipeToDismissBox` that springs
  back (the next frame drops the row) and a `combinedClickable` for the long press.
- **The composer** (`app_mobile::composer`, the TUI's pattern). The typed subject and body stay in the presenter's
  buffers and go back as one `update_draft` when typing pauses for a second (`tick`), when the field loses the
  focus (`blur`), or before any other action (a tap, Back, a key) — never per keystroke. A change made elsewhere (an
  agent's) reloads a field unless it has unsaved typing. Every text field is a `TextValue {text, version}`: Kotlin
  keeps the IME's state and takes the frame's text only when the version changes, so a frame never overwrites a
  composition. To, Cc and Bcc show their addresses as chips (a tap removes one) and take typed addresses
  (`compose::add_recipients`) at a comma, a semicolon, a line break or a blur; invalid text stays typed. Up to three
  contacts matching the typed text (`listing::contacts`, cached per revision) are suggestions; a tap adds one. With
  two addresses or more, `from_choices` is a menu of `set_draft_from`. The quote of a reply or forward is read-only
  and folded (a tap shows its header); the draft's text is scrubbed like the TUI's (unsafe characters become
  U+FFFD, line breaks stay). *Attach files* asks the host for the system file picker (`External::PickFiles{draft}`);
  the draft's files are chips with `checking…`, their size or why they cannot be sent (a tap is
  `remove_attachment`). *Send* (`send_draft`) shows while `compose::sendable` (a recipient, every file checked); the
  snackbar counts the undo window down (*Sending in N s · Undo* = `cancel_send`). Once per draft that becomes
  scheduled or leaving, by whatever command, the presenter asks for `External::Sending`: Kotlin starts `SendService`, a
  foreground service (`shortService` from Android 14, `dataSync` before) that runs the host's `sends` task, so leaving
  the app never strands a send. Every new `send_error` is told once in the snackbar (*Not sent: “subject”: error*),
  including the sends a restart cancelled (§4.4). *Discard* is `discard_draft` (undoable). While a send is scheduled or leaving, the
  composer is read-only. A hardware key in the composer types (`KeyState::press` with `typing`).
- **Leaving the app** (A5). With the first frame and whenever `background_sync` or `background_interval_mins`
  changes, the presenter asks for `External::Schedule{sync, interval_mins}`; `Host.kt` applies it with or without an
  activity. `DailJobService` (JobScheduler; no WorkManager) runs two jobs through the host's `background` call, which
  opens the session if no activity has: `flush` (one-off, any network, scheduled by `MainActivity.onStop`: queued
  triage, draft and attachment saves, at most 3 min) and `check` (periodic every `interval_mins`, network required,
  persisted across reboots with `RECEIVE_BOOT_COMPLETED`; `sync_now` as `runtime`, then waits until no account is
  syncing), scheduled unless `sync` is 0 (push keeps it as the fallback). While the activity is started, a default
  network callback (`ACCESS_NETWORK_STATE`) sends `UiEvent::Online` when a network comes back, which syncs if the app
  is visible and has an account.
- **Notifications** (A5, `app_mobile::notify`). `app_front::alerts` now carry their threads (new mail) or draft
  (a failed send, an approval). The host's alert watcher (`watch_alerts`, a runtime task) diffs each new state while
  no activity is visible (the last `visible` event) and hands every `app_mobile::Notification` as JSON to
  `Listener.alert` from that attached runtime thread: `tag`, `channel` (`new_mail`, `not_sent`; `background` is the
  push service's), `title`, `body`, a lock-screen `public_title` without sender or subject, `open` (a `NoteAct`:
  `open_thread`, `open_draft`) and `actions` (*Archive*, *Mark read* over the new threads). Approval requests are left
  out (the phone has none). Kotlin's `Notifier` posts them (`VISIBILITY_PRIVATE` with the public version); a tap opens
  `MainActivity`, which sends `UiEvent::Notification{act}`; a button's `NoteReceiver` calls `background` with
  `{"notification": act}`, and the host submits `NoteAct::commands` as the user and flushes. Threads or drafts that are
  gone submit nothing. `POST_NOTIFICATIONS` is asked once, on the first frame with an account. The `notifications`
  setting turns them off as on the desktop. Sends a restart cancelled are told in the snackbar only: the restart
  happens before the watcher's first state.
- **Push** (A5, `background_sync` 2). The engine's IMAP IDLE watches (§5.2 step 11) already run while the session
  lives, so `PushService` only keeps the process: a foreground service (`specialUse` from Android 14 with its
  `PROPERTY_SPECIAL_USE_FGS_SUBTYPE`, `dataSync` before), started and stopped by the `schedule` external (Kotlin keeps
  the last answer in preferences) and after a reboot by `PushService.Boot` (`BOOT_COMPLETED`). Its silent notification
  on the *Background* channel is Rust's `app_mobile::push_status`: *Watching N inboxes* (the accounts whose watch is
  `watching`; *Connecting to your inboxes* while none is), handed out whenever the text changes, visible or not, and
  again by the `push` task the service runs once it is in the foreground. Its buttons: *Stop* (`NoteAct::StopPush` =
  `set_setting background_sync 1`, as the user, which stops the service through the next `schedule`) and *Battery*
  (the system's battery-optimization screen, a host action). The periodic check stays scheduled as the fallback.
  Settings › General › *Background & notifications* turns push on and off.
- **`mailto:` and the share sheet.** `MainActivity` takes `VIEW`/`SENDTO` with `mailto:` and `SEND`/`SEND_MULTIPLE`
  of any type, and Kotlin only reads the intent into `UiEvent::Share{mailto, to, subject, text, files}`. The presenter
  parses the link (`app_front::mailto`, RFC 6068: path and `to`, `cc`, `bcc`, `subject`, `body`), adds `EXTRA_EMAIL`,
  the subject and the text, and submits `create_draft` for the current account (`Purpose::Shared`), which opens in the
  composer — never a send. Of the shared URIs only `content://` ones of other apps pass (`file://` would name any file
  dail can read; `net.darii.dail` and its suffixed authorities serve dail's own files); the rest are counted in the
  snackbar. Once the draft exists, `External::ImportFiles` hands the accepted URIs back, and each is copied and
  attached like a picked file. Without an account the snackbar says to add one.
- **The reader** (`app_mobile::mobile::reader`): the thread's messages, collapsed (sender, date, snippet) or expanded,
  a tap toggling (`expand_messages`/`collapse_messages`). An expanded body is `DocBlock`s: an HTML body's
  `app_core::document` (paragraphs, headings, lists, quotes, preformatted lines, tables, rules, folds), or plain
  text's paragraphs with its signature and quote folded (`app_front::fold`). Every span is cleaned (line breaks
  kept), marked where search words are (`hit`), and a link span carries only an action ID. Folds stay closed until
  tapped (presenter state per message and kind, no command). *Links (N)* lists every target in full. A link's tap
  opens `link`, a dialog with the full target, *Open* and *Copy*: *Open* asks the host for `open_url` (`http`/`https`
  only, `ACTION_VIEW` with `CATEGORY_BROWSABLE`), and a `mailto:` link becomes a new draft (as a share does), never
  another app; *Copy* is `copy`. A × beside the subject (`close`, `close_thread`, MISC-7) closes the message. The toolbar: archive, trash, star or unstar, mark read or unread, *Move…* (the
  palette at `move_threads`' step) and reply.
  - **`previous`/`next`** (MISC-18) are the neighbouring conversations of the list. Each is a `Neighbour`: the
    `open_thread` button and a peek (sender, subject, date) that the list rows' own helper builds, so it is cleaned
    as they are.
  - **Turning.** There is no pager bar. A pull past the end of the body shows the next conversation's peek, and a
    pull past its top the previous one's. Released past about 80 dp/pt, the body slides out, the peek slides in,
    and the conversation's frame replaces it. Only a drag pulls, never a fling's momentum.
  - **Animations off / Reduce Motion:** a cross-fade instead of the slide.
  - **Screen readers:** TalkBack and VoiceOver offer both conversations as custom actions.
  - `position` (`2 of 14`) stays in the frame but is not drawn. Back closes the link dialog or the preview, then the thread (the `Escape` binding).
- **Attachments and pictures.** An expanded message lists its attachments (not the inline parts): a tap opens
  `preview` over the panes, a picture (`kind: image`, the host's `blob` `image` request decoded by `app_io::images`),
  text (`kind: text`, not HTML: `blob`'s `text` request, which answers `app_mobile::preview_text` — UTF-8, at most
  256 KiB, every line cleaned), PDF pages (`kind: pdf`: `blob`'s `pdf_pages`, then `pdf_page` for base images and
  zoom tiles, ADR 0012) or a note that dail does not show the type. The kind comes from the MIME type (PNG, JPEG, GIF,
  WebP; `application/pdf`; text but HTML), and from the extension for `application/octet-stream`. `failed` is the
  text shown when the host cannot load it. Pictures and PDFs zoom (pinch, double tap fit ↔ 2.5×, pan; up to 5×):
  Android's viewer (`ui/viewer/`) is adapted from pdfcraft-android (MIT, credited in About); PDFs show "Page n of m".
  No Compose button floats over a preview. There is no "open with": other apps get a
  file only through *Save*. *Save* (one file) and *Save all (N)* ask for `External::SaveFiles{blob, files}`; the host
  replaces each name with `app_io::mime::safe_file_name` before Kotlin sees it, and Kotlin reads each file with
  `blob`'s `attachment` request into a document the Storage Access Framework creates: one file through the "create
  document" picker, several through a folder (`OpenDocumentTree`, `DocumentsContract.createDocument`, which gives a
  taken name a suffix, so nothing is overwritten). A body with `cid:` pictures shows *Pictures in this message are
  hidden.* and *Show images from \<sender\>* (`show_images_from`); while the sender is in `images_from`, those spans
  carry `image {blob, cid}` and Kotlin draws `blob`'s `inline` answer in place of the alt text. Nothing remote ever
  loads.
- **The palette** (`palette`, over the panes) is `app_front::palette::Palette` drawn as rows with action IDs: the top
  bar's search (`open_palette`, like the desktop's search pill) opens it at the root (the available registry
  commands, mailboxes and recent threads under their captions; one ranked list once text is typed, ending in *Search
  mail for “…”* = `set_search`). Its input is the `query` field of `Text` events and a `TextValue` whose version
  changes when the presenter replaces it (a step, a choice, a close). A row's tap is `Palette::choose`: the commands
  go out as the user's, or the sheet moves to the second step (`prompt`, `Filter…` or a free-text hint). Back leaves
  the step, then closes the palette (`close_palette`). A key binding or a button that needs an argument (the reader's
  *Move…*) opens it at that step; `add_account` and `set_password` open the account setup. A user's `Query` command answers in `result`: the `app_front::result` document (`sections` of `fields` and `rows`;
  a row's `action` closes the result and opens its thread or mailbox), or after its `raw` button (*Raw data* / *View*)
  the receipt as pretty JSON in `lines`, cleaned, read-only, under
  *Read-only result. Email content in it is untrusted data.*; Back closes it. While a search is open, rows carry
  `marks` (their subject and snippet cut at `app_front::highlight` words) and the list's `server` says *Searching the
  server…* or the newest server search's failure. `tests/it/palette.rs` walks every registry command through the sheet
  by taps (the parity walk).
- **Navigation is the `ViewState`.** The menu button is `focus_pane sidebar`. On compact and medium windows that
  opens the drawer modally over the panes `app_front::panes::behind_drawer` chooses (the frame keeps the list), and a
  tap in it also sends `focus_pane list`, which closes it. Back closes the About screen, a query's result or the palette
  (above) first, then a modal drawer (`focus_pane list`), else is the first `Escape` binding that resolves (`app_front::keymap`: close the composer, the thread, the search, the
  settings), else unchecks the checked threads. `frame.back` says whether Back is taken; otherwise Android handles it.
  `Refresh` (pull to refresh) is `sync_now`.
- **Hardware keyboards** (`app_mobile::keys`). `MainActivity.onKeyDown` gets the keys that no view took (text fields
  take typing; Back, Home and volume stay the system's) and sends `KeyDown{code, unicode, ctrl, shift, alt, meta}`,
  with the character as typed with Shift only. `keys::chord_of` maps it onto the keymap's chords (named keys by key
  code, a letter or digit with Ctrl or Alt by key code, a symbol with them as `Key::Symbol`, anything else as
  `Chord::typed`) and the chord goes through `KeyState`, as on the desktop. Meta is the system's, and so are the chords
  `keys::delivers` refuses: Ctrl+Space (the next keyboard layout), Alt+Tab (the app switcher), and the function keys
  (a Chromebook's top row). The palette's key hints and the Keyboard settings show only delivered keys.
  `Key{chord}` (the keymap's notation) is still taken, for the adb bridge. `tests/it/keys.rs`: every command with
  default keys has one Android delivers, in both presets, and some key event produces each of them.
- **The list** is a window of rows around what Kotlin reports visible (`Scrolled{first, count}`, ±20 rows), with
  `total` for `LazyColumn`; the thread IDs are cached per `AppState.revision`. Rows come in the `list_density`
  (cozy, compact, preview) and carry the account's tag with two accounts or more; `refreshing` is true while a sync
  runs (`!status::can_sync`), and the list has an empty text per case. `tests/it/list.rs` has an ignored measurement,
  `scale_a_frame_over_100k_threads` (`--release --ignored`): with the listing cached a frame takes well under 8 ms.
- **The message page** (MISC-18; it replaced MISC-17's sheet).
  - **The frame.** On a compact window an open thread keeps `layout.panes = [list]` and carries the message as
    `layout.page = reader`. The top bar is the list's, there is no floating button, and Back closes the page.
  - **Drawing.** Hosts draw a page over the panes and the top bar, below the status bar (`ReaderPage.kt`, and
    `ReaderPage` in `ReaderPane.swift`). It slides in from the right in 250 ms.
  - **The swipe back.** A horizontal drag anywhere on the page follows the finger. A block's own horizontal scroll
    goes first, and the dominant axis decides against the body's scroll. Past a third of the width, or with a
    fling, the page sends the reader's close (`close_thread`); otherwise it springs back.
  - **Leaving.** A frame that drops the page over the list slides it out. One that puts something else there (the
    composer) removes it at once.
  - **The list beneath** keeps its scroll position and is hidden from TalkBack and VoiceOver. VoiceOver's escape
    gesture closes the page.
  - **What shows over it:** the snackbar, a modal drawer, the link dialog, and the screens that take the panes'
    place (about, setup, palette, result, preview). The page waits under them, in place.
  - **Animations off / Reduce Motion:** the page appears without the slide.
  - Medium and expanded windows keep the message as a pane.
- **Keylines** (MISC-20). Both apps lay the panes on the top bar's two keylines, constants in one place per app
  (`android/…/ui/Keylines.kt`, `ios/Dail/Views/Metrics.swift`): the icon column (the menu icon, 16 to 40 dp/pt from
  the edge) and the text keyline (the title's start, 56 dp/pt; iOS gives the navigation button a fixed 24 pt slot at
  16 pt so its title starts there too). In the list the account's colour bar sits at the very edge, the unread dot or
  check box is centred in the icon column, and sender, subject, snippet, the server line, the empty state and the
  placeholders start on the text keyline. In multi-pane windows the keylines count from the pane's edge.
- **Tests.** `app-mobile/tests/it` run `Mobile` over a real `Engine` with the `basic` fixture (requests dispatched with
  `Origin::Gui`, replies fed back) and snapshot frames as `insta` JSON at 360×800, 700×1000 and 1280×800 dp.
  `cargo run -p app-mobile --example frames [width height]` prints the demo's frames.
  `tests/it/untrusted.rs` poisons a thread's subject, names, addresses, snippets, a folder's name, both bodies (HTML
  with a link, a picture's Content-ID and a table; plain text with a signature), link targets and attachment names
  and types with `ESC`, C1 controls, `\r` and U+202E, and walks every string of the list, drawer, reader, link
  dialog, preview, palette, query result and reply frames at the three window sizes: none is left (a body keeps
  its line breaks), only U+FFFD.
- **Contract.** `tests/it/contract.rs` writes those frames, one of every `UiEvent` and `External`, and the JSON Schema
  of `Frame` and `UiEvent` into `android/app/src/test/resources/contract/{frames,events,externals,schema}/`, which the
  Kotlin types decode strictly. A stale, missing or extra fixture fails the test; `DAIL_UPDATE_CONTRACT=1` rewrites them.

**The host** (`app-host`, ADR 0010; `app-android` is `[lib] name = "dail"`, cdylib + rlib, over it). `app_host::Host`
is the JNI-free core that Kotlin's calls map onto; it builds and is tested on the desktop. The platform comes in as an
`app_host::Platform` (the credential store and the logs, two plain functions, and the install kind for update checks):
`app_android::PLATFORM` on Android, `app_host::DESKTOP` (the OS keyring, no logs, no install kind) in tests. With an
install kind, `open` ingests `UpdateHostDetected` (this crate's version, target `android`/`ios`), as the CLI runtime does.

- **`open(&Config, Platform, wake, alert)`.** `Config` = `{profile_dir, cache_dir?, outgoing_dir? (default `<profile_dir>/outgoing`), backend: "live" | {"memory": fixture}, lang?,
  version?}`. Live: `ProfileDirs::at(profile_dir)`, the profile's `lock` (a second host is refused), then the session as
  `app-cli` opens it (`ProfileStore`, the platform's secret store, `NetVerifier`, `NetImap`, the full-text index, the audit log),
  without an IPC socket. Memory (the `demo` feature only, app-testkit as ADR 0001): the fixture's first account added
  and synced, the inbox open, like the GUI's demo. A tokio runtime of two workers; `Bridge::own` with `Origin::Gui`.
- **`event(UiEvent)` / `frame()`** return `Output{frame, externals}`: the presenter's requests are submitted through
  the bridge (only queued, never awaited), the replies fed back, the bodies for the open thread asked for. `wake` (from
  runtime threads) says a new frame is ready.
- **`blob(BlobRequest)`** (IO threads, blocks): `{attachment: {blob, part}}` as stored, `{text: {blob, part}}` as
  `app_mobile::preview_text`, or `{image: {blob, part}}` /
  `{inline: {blob, cid}}` decoded by `app_io::images` into width and height (two little-endian `u32`) then RGBA;
  `{pdf_pages: {blob, part}}` as JSON `{"pages": [[w, h], …], "max_side", "max_width", "tile_max"}` (points as shown,
  limits in pixels) and `{pdf_page: {blob, part, page, width, tile?: {x, y, w, h}}}` rendered by `app_io::pdf` into
  the same image format (ADR 0012). The host keeps the open preview's PDF bytes (one entry, keyed by `(blob, part)`,
  dropped when no preview shows) and runs one PDF job at a time on a blocking thread, within 10 s. `None` if it cannot
  be read, decoded or rendered in time.
- **`import(draft, name, source)`** (IO threads, blocks while it copies): a picked or shared file. Kotlin opens the
  content URI and passes `/proc/self/fd/<n>`, so Rust reads it without a new JNI call; `app_host::outgoing` copies it into
  `<outgoing_dir>/<32 hex digits>/` under `safe_file_name(name)`, at most 25 MiB (`MAX_ATTACHMENT_BYTES`; a larger file
  is not kept and the snackbar says so), and submits `attach_file` with the copy's path as the user. A content URI's
  permission ends with the activity; the copy outlives it. `open` removes the folders no draft names
  (`outgoing::sweep`). `calls::background` takes it as `{"import": {draft, name, source}}` and answers
  `{"attached": path}`. In the demo the memory backend checks files in its fake file system, so a picked file shows as
  failed there.
- **`debug(DebugTask)`** (the debug adb bridge; a release build of the library refuses it): `{"exec": {origin,
  command}}` submits a registry command (`args` may be left out) as `cli` or `mcp` only, never as `gui` or `runtime`,
  and answers `{"receipt": …}` or `{"rejected": …}`; `"state"` answers the `AppState`. `calls::background` takes a
  `Task`, an import or a `DebugTask`.
- **`background(Task, limit)`** (a job's thread, blocks at most `limit`, 3 minutes for Kotlin): `check` submits
  `sync_now` as `runtime` and waits until no account's `SyncStatus.pending`; `flush` waits for the session's queued
  work; `sends` waits until no draft is scheduled or leaving (`Draft::is_outgoing`). `close()` shuts the session down after the queued requests.
- **JNI.** `app_host::calls` holds the six calls in safe Rust, JSON in and out: `open(platform, config, wake, alert) -> {handle}` (Kotlin also passes the application context),
  `event`, `frame`, `blob` (bytes or `null`), `background(task) -> {done}`, `close`. A handle is a small integer into a
  table, never a pointer (arm64 tags heap pointers). Every call answers `{"error": "…"}` on failure, and a panic is
  caught there and becomes such an error. `src/jni.rs` only converts arguments: `JNI_OnLoad` keeps the `JavaVM`, the
  exports are `Java_net_darii_dail_Native_*` (Kotlin's `object Native` with `@JvmStatic external fun`), and
  `Listener.wake()` is called from runtime threads, each attached once (`jni` 0.22's `attach_current_thread`).
  `Listener.alert(json)` joins with the notifications (A5).
- **Secrets** (`app-android`'s `dail::secrets`). On Android, keyring 4's `android-native-keyring-store` (an AES key in the Keystore,
  the ciphertext in `SharedPreferences`) becomes keyring-core's default store, and `AndroidSecrets` (keyring-core's
  `Entry`) is the session's `SecretStore`: keyring's `v1` `Entry`, which `KeyringSecrets` uses, refuses Android.
  `Native.open` takes the application context and hands it to ndk-context with the `JavaVM`, once. Elsewhere the host
  uses `KeyringSecrets`. The round trip on a device is an ignored test (`DAIL_IT_ANDROID_KEYSTORE=1`, inside the app's
  process).
- **Logs** (`app-android`'s `dail::logging`). `tracing` to `<profile>/logs/android.log` (0600) and, on Android, to logcat through logd's socket
  (`tracing-logcat`, no liblog); `warn` and above in release builds, `info` in debug builds.
- **The lint exception.** The crate's `[lints]` is the workspace's with `unsafe_code = "deny"`; only `src/jni.rs`
  allows unsafe code. `tests/it/lints.rs` pins both.
- **Tests** (`app-host/tests/it`; `app-android/tests/it` keeps the lints and the Keystore round trip): a live profile with no accounts (the lock, the background tasks), a synced
  memory session driven by events (a tap opens a thread and wakes the app, a swipe archives with `haptic`, bad JSON is
  an error), and the calls over JSON (handles, errors, the log file).

**The Kotlin app** (`android/`, Gradle; application ID `net.darii.dail`). Versions live in
`gradle/libs.versions.toml`: AGP 9.4 (its built-in Kotlin), Kotlin 2.4, and the Compose BOM, AndroidX core and
lifecycle that Dependabot keeps current. compileSdk 37.2 (Compose 1.12 needs it), targetSdk 36, minSdk 29, NDK r28
(`ndkVersion`), ABI splits arm64-v8a and x86_64 (`-Pdail.abis=x86_64` narrows a local build).

- **The library.** A `cargoNdk<Variant>` task per variant runs `cargo ndk -t <abi>… --platform 29 build -p app-android
  [--release] [--features demo]` with `-Wl,-z,max-page-size=16384`, and keeps only `libdail.so` (cargo-ndk also copies
  dependencies' cdylibs) in `build/rustJniLibs/<variant>`, a generated jniLibs directory. So `./gradlew assembleDemoDebug`
  and Android Studio build everything.
- **Flavours.** `live` (the profile, the keyring, the servers) and `demo` (`net.darii.dail.demo`: the memory backend
  with the `media` fixture (`basic` plus a photo and PDFs, MISC-19); `BuildConfig.BACKEND` is `open`'s `backend`.
- **Classes.** `Native` (the six calls) and `Listener`; `DailApplication` holds the `Host`, opened on first use; `Host`
  runs `open`, `event` and `frame` on one thread, "dail-ui", and publishes the decoded frame as a `StateFlow`;
  `MainActivity` draws it edge to edge, sends `configure` on every configuration change, `visible` on start and stop,
  `tick` every second while `wants_tick`, and Back through `BackHandler(frame.back)` (predictive back is on in the
  manifest). `ui/Frame.kt` holds the frame types (kotlinx.serialization), `ui/UiEvent.kt` the events, `ui/DrawerScreen.kt`
  the drawer (Material's `ModalNavigationDrawer` while `modal`, whose swipe or scrim closes it by sending Back; a
  `PermanentDrawerSheet` pane, or a `NavigationRail` while `rail`), `ui/ListScreen.kt` the thread list (a `LazyColumn` of
  `total` items, placeholders outside the window, `scrolled` reports, `PullToRefreshBox`, the three densities),
  `ui/DailScreen.kt` the banner card and the snackbar, `ui/SettingsScreen.kt` the settings (tabs, `FilterChip`
  choices), `ui/SetupScreen.kt` the account setup (its fields set `contentType` for autofill).
- **Theme** (`ui/DailTheme.kt`). Material 3 from the frame's theme, the last one `Host` received: the content roles
  fill the colour scheme's slots (`inverse_bg` → primary, `canvas` → background, `base` → surface, `raised` →
  surface variant, `border_strong` → outline, `danger` → error, …), the chrome roles the top bar, the type scale the
  typography (Geist and JetBrains Mono at weights 400/500/600 on their `wght` axis). The status bar follows the
  chrome's luminance, the navigation bar the content's. A `themeFonts` task copies Geist, JetBrains Mono and the
  Material Symbols subset from `crates/app-theme/assets/fonts` into generated font resources; an icon whose name
  has no glyph is drawn as its label.
- **Tests** (`app/src/test`, JVM): `ContractTest` decodes every contract frame and external strictly and checks that
  Kotlin's events encode exactly as the event fixtures.
- **The debug adb bridge** (`app/src/debug`: `DebugBridge`, a content provider at `<applicationId>.debug`, exported in
  debug builds only). It answers only the shell UID (2000), and throws `SecurityException` to anyone else. Methods
  (`adb shell content call --uri content://net.darii.dail.demo.debug --method <m> [--arg '<json>']`, the answer in the
  bundle's `result`): `exec` (the command as `cli`), `exec_agent` (as `mcp`, under the agent policy), `state`, and
  `frame` (the frame last drawn, kept by `Host`, so the bridge never takes the UI's replies or action IDs). It opens the
  session if no activity has. With `uiautomator dump`, `screencap` and `input` an agent drives and reads the app.
  `app/src/androidTest` (`connectedDemoDebugAndroidTest`) calls it through the shell (`UiAutomation`), as adb does.
- **The Gradle side of `cargo deny`.** `gradle/verification-metadata.xml` holds a SHA-256 per artifact (regenerate
  with `./gradlew --write-verification-metadata sha256 <tasks>` after a version change); Licensee allows only
  Apache-2.0, MIT and BSD; Spotless runs ktlint (`.editorconfig`); Android Lint treats warnings as errors, with
  `app/lint.xml` turning off, with reasons, the checks that deliberate version updates make moot.
- **The APK check** (`cargo xtask android apk-check`, over every APK under `app/build/outputs/apk`): `lib/` holds
  only `libdail.so` and the prebuilts of `PREBUILT_ALLOWLIST` (so far Compose's `libandroidx.graphics.path.so`), each
  with its reason; every `.so` is 16 KB-aligned (`zipalign -c -P 16`); the permissions equal `APK_PERMISSIONS`; a
  release APK is not debuggable and carries no debug bridge. Instrumented-test APKs are skipped.
- **Licence lists** (`cargo xtask android licences`): `rust-crates.txt` from `cargo metadata` for both Android
  targets (`app-android`'s normal dependencies without features, less the workspace's crates and proc macros) and
  `android-libraries.txt` from Licensee's `androidLiveRelease` report. `android check` runs `--check`, which fails
  when either is stale.
- **Release** (`scripts/release-android.sh`; the `android` job of `release.yml`, on demand): `assembleLiveRelease`
  signed with the key from `DAIL_SIGNING_STORE`, `DAIL_SIGNING_STORE_PASSWORD`, `DAIL_SIGNING_KEY_ALIAS` and
  `DAIL_SIGNING_KEY_PASSWORD` (CI decodes the keystore from the `DAIL_ANDROID_KEYSTORE_BASE64` secret into the
  runner's temp directory; without the variables the release APKs are unsigned and the script refuses), then
  `apk-check`, `apksigner verify`, and `dist/dail-<version>-<abi>.apk` with a `.sha256` each. The version name is the
  workspace version and the version code `major·10⁴ + minor·10² + patch`, the same for both ABIs. R8 minifies and
  shrinks; `app/proguard-rules.pro` keeps `Native`, `Listener` and the native methods (JNI binds them by name) and
  the app's kotlinx.serialization serializers. Size at 0.1.0: 17 MiB arm64-v8a, 19 MiB x86_64, nearly all of it
  `libdail.so`.

### 7.6 iOS/iPadOS (`app-ios`, ADR 0010)

In progress (ROADMAP Wave 5). The SwiftUI app draws `app-mobile`'s frames, as the Kotlin app does (§7.5), over the
same `app-host`. There is no Mac: Swift, linking and the simulator run on `macos-latest` CI runners only.

**The host** (`app-ios`, `[lib] name = "dail"`, staticlib + rlib). `app_ios::PLATFORM` gives `app-host` the
credential store and the logs. It builds and is tested on every host; the iOS-only parts sit behind
`cfg(target_os = "ios")`.

- **The C calls** (`src/ffi.rs`, declared in the hand-written `include/dail.h`). `dail_open(config, listener)`,
  `dail_event`, `dail_frame`, `dail_blob` (a `DailBytes {ptr, len}`, null for none), `dail_background` and
  `dail_close` are `app_host::calls` with C strings: NUL-terminated UTF-8 in, owned by Swift for the call; strings out
  owned by Rust until `dail_string_free`, bytes until `dail_bytes_free`. A null or non-UTF-8 argument answers
  `{"error": "…"}`. The `DailListener` is `{context, wake, alert}`, C function pointers called from runtime threads
  (Swift hops to the main actor). `tests/it/header.rs` matches the header's functions and struct fields against
  `ffi.rs`, so no `cbindgen` is needed.
- **Secrets** (`dail::secrets`). On iOS, keyring 4's `apple-native-keyring-store` protected-data store (the
  data-protection Keychain, not synced to iCloud) becomes keyring-core's default store, and `IosSecrets`
  (keyring-core's `Entry`) is the session's `SecretStore`: keyring's `v1` `Entry` refuses iOS. app-sync turns the
  store's `protected` feature on for iOS, without which keyring does not build there. Elsewhere `KeyringSecrets`.
- **Logs** (`dail::logging`). `tracing` to `<profile>/logs/ios.log` (0600); `os_log` would need C. `warn` and above
  in release builds, `info` in debug builds.
- **The lint exception.** As app-android's, with `src/ffi.rs` the only module allowing unsafe code;
  `tests/it/lints.rs` pins both.
- **Build** (`cargo xtask ios`, `xtask/src/ios.rs`). `check` runs clippy `-D warnings` for `aarch64-apple-ios` and
  `aarch64-apple-ios-sim` over the crates the app links, with and without `demo`; it needs only the rustup targets, so
  it runs on Linux. `xcframework [--demo]` (macOS with Xcode) builds `libdail.a` in release for both targets
  (`target/ios-live` or `target/ios-demo`) and packs it with `include/` (`dail.h` and the `DailCore` module map) into
  `ios/Frameworks/DailCore[-Demo].xcframework`, which is gitignored. Elsewhere it reports "cannot run".
  `build` (macOS with Xcode and XcodeGen) makes both, generates `ios/Dail.xcodeproj` from `ios/project.yml` (never
  committed) and builds the `Dail` (live) and `DailDemo` schemes for the simulator. `test` runs `DailTests` in the
  `DailDemo` scheme on the newest available iPad simulator (result bundle `target/ios-tests.xcresult`, which CI uploads
  on failure). `app` archives both schemes for devices in Release, unsigned, as
  `dist/dail-<v>[-demo]-unsigned.ipa` (a zip of `Payload/<App>.app`) with `.sha256`; `release.yml`'s `ios` job uploads
  them. `all` is the four, the non-default `ios` gate of `cargo xtask verify`. `ci.yml`'s `ios` job runs the gate on `macos-latest` when a change touches `crates/`, `ios/`, `xtask/`, `Cargo.*` or the workflow (a
  `git diff` in the `ios-changes` job), and always when started by hand.
- **The app** (`ios/`). Two app targets over the same Swift sources, like Android's flavours: `Dail`
  (`net.darii.dail`, links `DailCore.xcframework`) and `DailDemo` (`net.darii.dail.demo`, `DailCore-Demo`, the `DEMO`
  condition: `{"memory": "media"}`). iOS 17, iPhone and iPad, unsigned. `Native.swift` is the only file that calls
  `dail.h`: it copies and frees the strings and bounces the listener (an unretained `Listener` as the context) to
  the main actor. The profile is `Application Support/profiles/default`.
- **The contract in Swift** (`ios/Dail/Frame.swift`, `UiEvent.swift`): Codable mirrors of `app_mobile`'s frame,
  externals, notifications and events, by hand, as Kotlin's. `Button`, `Layout` and `Preview` are `ActionButton`,
  `FrameLayout` and `AttachmentPreview`, so as not to shadow SwiftUI. `DailTests` reads the same fixtures as Kotlin's
  `ContractTest` (`android/app/src/test/resources/contract`, a folder resource of the test bundle). Codable ignores
  unknown keys, so "strictly" means: each fixture decodes, and re-encoding the value gives back every key and value it
  had (`null` counts as absent). The events Swift builds equal Rust's fixtures. `HostTests` opens the demo host in the
  simulator, decodes its frame strictly and sends a `configure`.
- **The views** (`ios/Dail/Views`, as Kotlin's `ui/`). `Host.swift` runs every call on one serial queue ("dail-ui"),
  as Android's thread, and publishes `Screen` (opening / ready / failed) on the main actor; a wake takes the frame.
  `RootView` sends `configure` (points as dp; the Dynamic Type size as the font scale) on every size or look change,
  and the app `visible` on scene changes. `FrameView` draws the top or bulk bar, the banner, the layout's panes in a
  row (drawer, list, reader), the snackbar (undo) and the floating button; the drawer of compact windows is a sheet
  whose dismissal is Back. `ListPane` keeps Android's window (`total` rows, placeholders outside `offset…`), reports
  `scrolled` from the rows on screen, and maps tap, long press, both swipes and pull to refresh to their events.
  `ReaderPane` draws `DocBlock`s as text: untrusted text is always verbatim (never Markdown), a link span is a
  `dail-action:<id>` link the view turns into a tap (so the link sheet shows the target first), and inline images
  are placeholders. `PreviewView` draws an attachment preview (MISC-19): Close, the name and Save, then a picture or
  PDF pages in `DocumentView` (a `UIScrollView` zooming 1–5×, double tap fit ↔ 2.5×, unanimated under Reduce Motion;
  pages laid out by `DocumentLayout`, a port of the adapted `DocLayout`; each page a fit-width image from `blob` with
  a `CATiledLayer` above it whose `draw` asks for `pdf_page` tiles off the main thread) with "Page n of m", or the text
  monospaced. `Host.blob` runs `dail_blob` on a serial "dail-blob" queue, never on main or dail-ui; views get it as
  the `\.blobs` environment value. Icons map the frame's Material names to SF Symbols, with the label for VoiceOver. Externals:
  `copy`, `open_url`, `haptic` and `system_settings`; the file ones wait for the compose wave.
- **Palette, setup and settings** (`PaletteView`, `SetupView`, `SettingsPane`). Text fields are the presenter's
  (`FrameTextField`): typing goes back as `text`, and the frame's value replaces the field only when its `version`
  moves; leaving a field sends `blur`. Setup fields take their keyboard, autofill and capitalisation from `InputKind`,
  and the password is a `SecureField` that never comes back in a frame. Tests over the demo host: the parity walk
  (`PaletteTests`) finds every command row of the empty palette by its title in a fresh host and taps it, and the
  palette must close, open the setup or move to a step; what each step builds is `app_front`'s, walked in Rust.
  `KeychainTests` runs the setup of a live session (Gmail preset, a throwaway password) and finds the password in the
  Keychain (`dail` / `account/<id>`), then deletes it.
- **Signing.** Device archives are unsigned (the user re-signs them). Simulator builds are signed ad hoc
  (`CODE_SIGN_IDENTITY[sdk=iphonesimulator*] = -`): without the entitlements that signing embeds, the
  data-protection Keychain refuses every item.
- **Installing on a device** (free Apple ID, ROADMAP I4). What worked for the live IPA on the user's iPad: download
  `dail-<v>-unsigned.ipa` from the release (check its `.sha256`), re-sign and install it with iloader under a free
  Apple ID, turn on Developer Mode (Settings → Privacy & Security, then restart), and trust the developer profile
  (Settings → General → VPN & Device Management). A free-account signature expires after 7 days; re-sign to renew.
  The demo IPA (`net.darii.dail.demo`) should take the same steps under its own bundle ID (not yet tried).
- **Scale** (plan §9). `app-testkit`'s generated `large` fixture is `basic` plus 20 000 inbox messages in threads of
  4. `ScaleTests` opens it in the simulator, keeps the whole inbox offline through the palette, and logs the open,
  backfill and scroll times and the app's footprint (`phys_footprint`). `cargo xtask ios measure` builds `app-io`'s
  `scale` tests (redb and tantivy at 50k messages, `--ignored`) for `aarch64-apple-ios-sim` and runs them in the
  booted iPad simulator with `xcrun simctl spawn`; CI's `ios` job runs it after the gate. The results are in plan §9.
- **Tests** (`app-ios/tests/it`): the C calls over a live profile with no accounts (handles, the lock, the listener,
  the log file, freeing), bad input (null, non-UTF-8, unknown handles), and the header.

## 8. Initial Command catalogue (MVP)

| name | kind | args (sketch) | milestone |
|---|---|---|---|
| `get_settings` | Query | — | M0 |
| `set_keybinding` / `set_key_preset` | Mutation | `command, keys?` (none: back to the preset's; `[]`: unbound) / `preset: gmail | vim`; in `settings` (`config.toml`), undoable | Wave 2 |
| `set_setting` | Mutation | `key: SettingKey, value` (`undo_send_ms`, `poll_secs`, `initial_window`, `history_cap`, `theme`: 0 system / 1 light / 2 dark (3 and 4 still accepted: they set dark mode with `dail-dark-hc` or light mode with `dail-light-hc`), `text_scale`: 50–300 %, `notifications`: 0 off / 1 on, `list_density`: 0 cozy / 1 compact / 2 preview, `tui_colors`: 0 terminal / 1 theme truecolor, `background_sync` (Android only): 0 while open / 1 periodic (default) / 2 push, `background_interval_mins` (Android only): 15–720, default 30, `stale_secs`: 0–86400, default 60 — `open_mailbox` fetches a mailbox whose `last_fetch_ms` is older (UX-6), `full_sync_secs`: 0 off or 60–86400, default 900 — the poll's full rounds, `after_triage`: 0 next (default) / 1 previous / 2 list — after the open thread is archived, trashed or moved, the GUI and the TUI submit `app_front::advance::after`'s `open_thread` of the neighbour (rows triaged with it skipped; the other side at the ends) or `close_thread` right after it (UX-12), `mark_read_after_ms`: 0 never or 250–60000, default 2000 — the GUI and the TUI run the timer (`app_front::autoread::AutoRead`, UX-5): once the open thread has been open that long (counted only while the GUI window has the focus) its unread expanded messages get one ordinary, undoable `mark_read` from the user; leaving the thread cancels it, the user's `mark_unread` disarms it until another thread opens; agents' `open_thread` never marks anything read; the TUI checks once a second; the phone does not run it yet) | M0 |
| `set_theme` | Mutation | `mode?: system | light | dark, light?, dark?` (theme names, slugs; at least one field); sets `theme`, `theme_light`, `theme_dark` in one undo step; agents allowed (only the look changes); an unknown name falls back to the mode's default in the GUI with a notice | UI-11 |
| `undo` / `redo` | Mutation (not recorded) | — | M0 |
| `open_palette` / `close_palette` | View | — | M0 |
| `add_account` | Mutation | `preset, email, display_name?, password?, imap?, smtp?` (password → keyring via `StoreSecret`; user only, §9) | M1 |
| `test_account` | Query | `id` (emits `VerifyAccount`; result lands in `Account.status`) | M1 |
| `update_account` | Mutation | `id, display_name?, imap?, smtp?` (a server change resets status to `unverified`; user only, §9) | M1 |
| `set_password` | Mutation (no history; user only, §9) | `id, password` (write-only `Secret`) → status `unverified`, emits `PersistState`, `StoreSecret`, `VerifyAccount`; the result lands in `Account.status` | Wave 2 |
| `remove_account` | Destructive | `id, confirm` (also emits `DeleteSecret`) | M1 |
| `discover_servers` | Query | `email` → emits `DiscoverServers{email, domain}`: HTTPS autoconfig at `autoconfig.<domain>`, `<domain>/.well-known/autoconfig`, then Mozilla's ISPDB (ADR 0006); the servers (or why not) land in `status.discovery`, and asking again for the same address returns the servers found (`state: found`, no new lookup); nothing is configured | Wave 2 |
| `sync_now` | Mutation (no history) | `account?` (lists mailboxes → fetches each; skips accounts still syncing; starts polling) | M2 |
| `list_mailboxes` | Query | `account?` → `{mailboxes: [{id, account, path, parent (path above, or null), depth, role, unread, total, cached, window, backfilling}]}` (inbox first) | M2 |
| `list_threads` | Query | `mailbox, offset = 0, limit = 50 (1–500)` → `{total, threads: [{id, messages, in_mailbox, unread, flagged, has_attachments, last_date_ms, untrusted_content: {subject, from, snippet}}]}` (the newest message's snippet, or null), newest first | M2 |
| `get_thread` / `get_message` | Query | `id, include_body = false` (+ `rich_body = false` on `get_thread`: also `body.document` for HTML bodies) → thread `{id, account, last_date_ms, untrusted_content: {subject}, messages: [msg]}` / `{message: msg}`; msg = `{id, account, mailbox, thread, date_ms, flags, size, has_attachments, body: {state}, downloading, untrusted_content: {subject, from, to, cc, message_id, snippet, body?: {text, text_source, attachments}}}`; `include_body` starts downloads of missing bodies | M3 |
| `save_attachments` | Mutation (no history; user only, §9) | `message, parts? (MIME part numbers; none: all), dir` (absolute; the body must be downloaded) → emits `SaveAttachments`; progress in `status.saves` (one per message, at most 16) | Wave 2 |
| `create_folder` | Mutation (no history) | `account, path` (trimmed; no control characters, `*`, `%` or empty segments; not an existing path, compared without case) → `{mailbox, path, undoable: false}`; the mailbox is cached at once (the account's delimiter, never fetched) and `ManageFolder{account, imap, action: create{path}}` runs IMAP `CREATE`; agents allowed | UX-14 |
| `rename_folder` | Mutation | `id, path` (the new full path; not the inbox, not into itself, not an existing path) → `{mailbox, from, to}`; the folder and its subfolders get their new paths in the cache, and pins, hidden folders and collapsed parents follow; `ManageFolder{rename{from, to}}` (IMAP `RENAME`); undo renames it back; agents allowed | UX-14 |
| `delete_folder` | Destructive | `id, confirm` (not the inbox, not a folder with subfolders) → `{deleted, path, undoable: false}`; drops the mailbox and its cached mail, `ManageFolder{delete{path}}` (IMAP `DELETE`); agents allowed with `confirm` | UX-14 |
| `pin_mailbox` / `unpin_mailbox` / `hide_mailbox` / `show_mailbox` | Mutation | `id` → `{mailbox, pinned, hidden, changed}`; settings `pinned_mailboxes` (in pinned order) and `hidden_mailboxes`, kept as `{account, path}` (they survive a rebuilt cache), saved in `config.toml`; repeating changes and records nothing; undo is the opposite command (an unpinned folder comes back last). Pinned folders form a *Favourites* section above the accounts (`app_front::mailbox::favourites`); hidden ones and their subfolders leave the tree (`visible_rows`) but stay in search, *Move to…* and behind the GUI's per-account *Show hidden (n)* toggle; agents allowed | UX-8 |
| `show_images_from` / `hide_images_from` | Mutation | `sender` (an address, kept in lower case in `settings.images_from`, saved in `config.toml`) → `{sender, showing, changed}`; repeating it changes and records nothing; the reader draws those senders' inline `cid:` images, never remote ones | Wave 2 |
| `list_contacts` | Query | `query = "", limit = 50 (1–500)` → `{contacts: [{count, last_ms, written_to, untrusted_content: {email, name}}]}`: senders and recipients of the cached mail but the accounts' own addresses, the ones you wrote to first, then by count and recency (`listing::contacts`) | Wave 2 |
| `set_identity` / `remove_identity` | Mutation (user only, §9) | `account, email, name?, signature?` / `account, email`: the addresses the account sends from (saved with it, `[[account.identity]]`); undoable | Wave 2 |
| `set_mailbox_identity` | Mutation | `mailbox, email?`: the From of new drafts written from it | Wave 2 |
| `set_draft_from` | Mutation | `id, email` (one of the account's identities; the own address clears `draft.from`); an agent's change withdraws its send request | Wave 2 |
| `check_threads` / `uncheck_threads` | View | `ids?` (none: every thread the list shows / all) → `{checked}`; `view.checked`, cleared when the list shows another mailbox or search; triage from the GUI and the TUI acts on it when not empty (`palette::targets`) | Wave 2 |
| `expand_messages` / `collapse_messages` | View | `ids?: [MessageId]` of the open thread (none: all) → `{messages, expanded}`; sets `view.expanded`, cleared when another thread opens or the reader closes; error without an open thread | Wave 2 |
| `search` (Wave 2 additions) | Query | `order = newest | relevance` (full-text hits first, best first) | Wave 2 |
| `set_sync_window` | Mutation | `mailbox, window?` (newest messages to keep cached; 0 = all; none = `initial_window`) → `{mailbox, window, previous}`; starts the history backfill (§5.2); undo restores the previous window | Wave 2 |
| `compact_store` | Mutation (no history) | — → `{bodies_kept}`; emits `CompactStore` (§6); outcome in `status.compaction` | Wave 2 |
| `check_for_updates` | Mutation (no history) | — → `update` (`AppState.update`); emits `CheckForUpdates{running}` unless a check, download or install runs; works with periodic checks off; needs `update.host` (`update_host_detected` from the runtime), else `invalid_argument`; `update_checked{manifest}` (signature verified by the runtime) → `available{release}` if newer than the host and not skipped, else `idle`; `update_failed{reason}` → `failed` (ADR 0011) | Wave 7 |
| `download_update` | Mutation (no history) | — (status `available`, a desktop install) → `downloading`; emits `DownloadUpdate{version, artifact}` (the artifact whose target and kind fit `host.install`) → `update_downloaded{version, path}` → `ready{release, path}` | Wave 7 |
| `install_update` | Destructive (user only, §9) | `confirm` (status `ready`) → `installing`; emits `InstallUpdate{version, install, path}` → `update_installed{version}` → `installed` (takes effect on restart) | Wave 7 |
| `skip_update_version` | Mutation | `version` (`x.y.z`, or null to offer every version again) → `settings.updates.skipped_version`; an `available` of that version becomes `idle`; undoable | Wave 7 |
| `set_update_policy` | Mutation | `checks?: unset | on | off, interval_hours?` (1–720, default 24) → `settings.updates`; undoable. With `on`, a check is due `interval_hours` after `update.last_check_ms` (at once if none): it starts on `update_host_detected`, on a tick or on this command, otherwise a `ScheduleTick` is emitted once for the due time (`update.next_check_ms`) | Wave 7 |
| `open_unified_inbox` | View | — (`g u`) → `{threads}`: `view.unified`, the list shows every account's inbox newest first (`listing::unified_threads`); `open_mailbox` leaves it, a search shows instead of it | Wave 2 |
| `open_mailbox` / `select_threads` / `open_thread` / `close_thread` | View | `id` / `ids` / `id` / — (`open_mailbox` clears selection and reader; `open_thread` selects the thread unless it is already selected and downloads missing bodies; nothing is marked read) | M3 |
| `mark_mailbox_read` | Mutation | `id` → the `mark_read` receipt for the cached unread messages located in the mailbox plus `mailbox`, `uncached_affected`; when the server counts more unread than are cached, a second op `flag_mailbox{mailbox, flag, value}` runs `RemoteStep::StoreAll` (`UID STORE 1:* +FLAGS.SILENT (\\Seen)`); the mailbox's `unread` drops to 0 until the next fetch; undo (*Mark all read (n messages)*) restores only the cached ones; no default key; the palette and the GUI toolbar ask first above 50 unread (`palette::MARK_ALL_CONFIRM_ABOVE`); agents allowed | UX-7 |
| `set_list_filter` | View | `unread?, starred?, attachments?: bool` (given ones replace) → `{filter, threads}`; `view.filter`, saved in `[view]` as `list_filter`; `listing::visible_threads` keeps the threads passing every filter that is on (`list_threads`/`search` are not filtered); checked threads it hides are unchecked; the palette asks for words (`unread starred attachments`) | UX-7 |
| `mark_read` / `mark_unread` | Mutation | `ids?: [ThreadId], messages?: [MessageId]` (at least one; a thread means all its messages, a message includes its copies in other folders) → `{changed, ops, cancelled}`; undo restores exactly `changed` | M3 |
| `star` / `unstar` | Mutation | same as `mark_read`; `star` on a thread stars its newest message | M3 |
| `pin_threads` / `unpin_threads` | Mutation | same as `star` / `unstar` (pinning a thread marks its newest message, unpinning all) → `{changed, ops, cancelled}`; `Flags.pinned`, the IMAP keyword `$Pinned` (`FlagKind::Pinned`, a `set_flag` op), so the pin syncs like a star; pinned threads come first in every list (`listing::sort_newest_first`: pinned, then newest; `list_threads` has `pinned`); `Shift+P` toggles; agents allowed. Where the server keeps no keywords (a `SELECT` whose `PERMANENTFLAGS` has neither `\\*` nor `$Pinned`) the runtime skips the keyword `STORE` and reports `KeywordsRefused{account, path}` before the op's success: the mailbox gets `custom_keywords: false`, its pins stay on the device (server flag merges keep them, no store is queued) | MISC-4 |
| `archive_threads` / `trash_threads` | Mutation | `ids?, messages?` → `{moved, copied, cancelled, ops, to}`; error `no_mailbox_for_role` if the account has no archive/trash (§5.3) | M3 |
| `move_threads` | Mutation | `ids?, messages?, to: MailboxId` (same account) | M3 |
| `list_pending_ops` | Query | `account?` → `{ops: [{id, account, action: set_flag\|move\|copy, detail, state, attempts, last_error, next_attempt_ms, created_ms}]}`, oldest first | M3 |
| `retry_pending_ops` / `discard_pending_op` | Mutation (no history) | `id?` / `id`. Retry: failed ops get one more attempt, waiting ops run now → `{retried}`. Discard (not a running op): drops the op, reverts its local change where no later op touches it (flag back, `moved_to` cleared, copy placeholder deleted) and re-fetches the mailboxes involved → `{discarded, reverted, undoable: false}` | M3 |
| `create_draft` | Mutation | `account, to?, cc?, bcc?: [Addr], subject?, body?` → the draft `{id, account, reply_to, message_id, created_ms, updated_ms, …, untrusted_content: {in_reply_to, to, cc, bcc, subject, body, quote}}`; undo discards it | M4 |
| `reply` / `reply_all` / `forward` | Mutation | `message, body?` (creates a draft with recipients, `Re:`/`Fwd:`, threading headers and a quote; starts the original's body download) | M4 |
| `update_draft` | Mutation | `id, to?, cc?, bcc?, subject?, body?` (at least one; given fields replace the old values) | M4 |
| `attach_file` / `remove_attachment` | Mutation | `draft, path (absolute), index?` / `draft, index` (attach emits `InspectAttachment`; the attachment's `state` becomes `ready{size, content_type}` or `failed{error}`, 25 MiB total) | M4 |
| `discard_draft` | Mutation | `id` → `{discarded, draft}`; undo is `restore_draft` | M4 |
| `restore_draft` | Mutation | `draft: Draft` (puts a discarded draft back with its ID and checks its attachments again; not in the palette; user only when dispatched directly, §9) | M4 |
| `send_draft` | Outbound | `id` → the draft with `send_at`; error `not_sendable` (no recipients, attachment checking/failed, already scheduled). From `Origin::Mcp`: `{state: "awaiting_approval", approval}` instead (§4.6) | M4 |
| `cancel_send` | Mutation | `id` (only while scheduled; undo schedules the send again) | M4 |
| `search` | Query | `query: String, account?, offset = 0, limit = 50 (1–500), server = true` → `{query (canonical), total, threads: [list_threads entry without in_mailbox + matches: [MessageId]], server: [{account, state: started\|searching\|done, hits, error}]}`, newest first; matches the cached envelopes (subject and addresses for text; `in:` uses `location()`) plus the server's hits for the query (§5.2 step 10); errors `invalid_argument` on `query` (with column) or empty query | M5 |
| `set_search` / `clear_search` | View | `query` / — (`set_search` sets `view.search`, clears selection and reader, starts a server search round and returns the first 50 results like `search`; `clear_search` → `{cleared}`) | M5 |
| `open_settings` / `close_settings` | View | `section?: appearance | keys | general | accounts` / — (the settings page in place of the list and the reader; without a section the open one, else `appearance`; `get_view` reports `settings`); keys `Ctrl+,` and `g ,` / `Escape` (before the other `Escape` commands); agents allowed | UI-12 |
| `open_composer` / `close_composer` | View | `id: DraftId` / — (shows the draft in the GUI composer and focuses it / closes it, keeping the draft; a composer whose draft is discarded or sent closes by itself) | M6 |
| `collapse_folders` / `expand_folders` | View | `keys?: [{account, path}]` (an empty `path` is the whole account; none: every account and every folder with folders below it / all) → `{changed, collapsed}`; idempotent; `view.collapsed`, saved in `[view]`; error `not_found` for an unknown account, `invalid_argument` for a path that is no folder of the tree; keys `ArrowLeft` / `ArrowRight` (vim also `h` / `l`) fold the open mailbox's folder while the sidebar has the focus; agents allowed | UX-3 |
| `focus_mailbox` / `clear_focus` | View | `id?` or `account?` (exactly one) / — → `{focused, mailbox}` / `{cleared}`; `view.focused` (`{mailbox: id}` or `{account: id}`, saved in `[view]`): the sidebar shows only that mailbox's subtree (`app_front::mailbox::sidebar_tree`) or account and a *Show all folders* chip, the unified inbox is hidden, new-mail notifications come only from it (`app_front::alerts`), the list title shows *Focus*. Focusing opens the mailbox (an account: its inbox unless one of its mailboxes is open); `open_mailbox` outside the focus and `open_unified_inbox` end it, and it ends with its mailbox or account. Keys `Ctrl+Shift+F`, `g f`; agents allowed | UX-4 |
| `focus_pane` / `toggle_sidebar` | View | `pane: sidebar\|list\|reader\|composer` / — (`reader` needs an open thread, `composer` an open draft, else `invalid_argument`; focusing the sidebar expands it. After every command and event the engine moves the focus off a pane that is no longer shown: to the composer, the reader or the list) | M6 |
| `get_view` | Query | — → the `ViewState` (`palette_open, mailbox, selected, thread, search?, composer, focus, sidebar_collapsed, collapsed?, focused?, filter?`); also the MCP resource `dail://state/view` | M7 |
| `list_approvals` | Query | — → `{approvals: [{id, draft, account, origin, requested_ms, message: <draft as in create_draft>}]}`, oldest first | M7 |
| `approve_request` / `reject_request` | Mutation | `id` (user only: GUI/TUI/CLI/batch origin, else `origin_not_allowed`). Approve → the `send_draft` receipt plus `approved` (undoable: cancels the send); reject → `{rejected, draft, undoable: false}` (no history) | M7 |

Search query language (pure parser in `app-core::search`, `Query::parse` → `Query { terms: [Term { negated, condition }] }`):
`from: to: cc: subject: is:unread|read|starred has:attachment in:<mailbox> before:YYYY-MM-DD after: "phrase" -negation`.
Terms are separated by whitespace and must all match; text matches are case-insensitive substrings. Field names and
keywords are case-insensitive; values may be quoted (`from:"Alice Example"`, `\"` and `\\` escapes). `in:` names a
mailbox by role (`inbox`, `sent`, …) or path. Dates are UTC days (`YYYY-MM-DD` or `YYYY/MM/DD`): `after:` includes the day,
`before:` excludes it, like IMAP `SINCE`/`BEFORE`. A word with an unknown `prefix:` is plain text (`re:plans`, URLs); a
lone `-` is ignored. Errors (`invalid_argument` on `query`) give the 1-based column and the fix. `Display` prints the
canonical form, which parses back to the same `Query`.

## 9. Security & agent policy

| Threat | Control |
|---|---|
| Prompt injection via email | `untrusted_content` envelopes; MCP instructions; agents can't escalate via content; no auto-actions on content in the MVP |
| Agent sends mail | `Origin::Mcp` + `send_draft` (also via `undo` of a `cancel_send` or `redo` of an approval) → `ApprovalRequest`, unless `--allow-send`; `approve_request`/`reject_request` refused from agent origins (`origin_not_allowed`); an agent's edit withdraws its request; IPC clients can't claim the `gui` origin (§5.4) |
| Agent goes beyond triage and drafting | One policy per command in the registry (`CommandSpec.agents`: `allowed`, `approval`, `user_only`; ADR 0002), enforced by `Engine::dispatch`. User only: `add_account`, `update_account`, `set_password`, `set_identity`, `remove_identity`, `save_attachments`, `restore_draft`, `approve_request`, `reject_request`, `install_update` → `origin_not_allowed` (`install_update` runs downloaded code; `save_attachments` writes files where the caller says; an agent could plant one that gets run). `undo`/`redo` apply the engine's own history and are not checked again. The policy binds the MCP origins: a process that runs `dail exec` as the user (or claims `cli` on the profile socket) is the user |
| Agent exfiltrates local files | `attach_file` takes any readable path. The file leaves the machine when the draft is sent (agent sends need approval; the card lists attachments with their paths) and when the draft is saved to its account's Drafts mailbox, on a server only the user sets up (`add_account`/`update_account` are user only). An agent can't hand the engine a draft with a forged attachment check: `restore_draft` is user only, and it checks every attachment again anyway |
| Agent mass-deletes | No permanent delete in the MVP besides `remove_account` (agents may run it: `Destructive`, needs `confirm`, MCP `destructiveHint`); trash is undoable |
| Credential leak | Keyring only; `SecretRef` in state; `Secret` redacts itself in `Debug` and `Serialize` (receipts, effects, replay logs); the password goes only to the servers of its account, which agents can't change |
| Tracking pixels / remote content | Never fetched; links shown with real target. `show_images_from` (agents may run it) only lets the reader draw parts carried in the message itself (`cid:`), decoded off the UI thread with size limits (PNG, JPEG, GIF, WebP; downscaled to 4096 px per side) |
| Terminal escape injection (TUI) | Email text and agents' drafts are drawn through one sanitizer (controls, ESC, DEL, bidi overrides replaced; `app-tui/tests/it/untrusted.rs`); no OSC 8 links, no window title, no browser or file dialog (they would open on the SSH host) |
| MITM | rustls with app-tls (ADR 0003: verification tested against Wycheproof and RustCrypto, strict DER, whole-encoding PKCS#1 comparison), webpki roots, no plaintext, no "accept invalid certs" setting |
| Malicious update | The manifest is accepted only with a valid minisign signature from the built-in key, a trusted comment naming its version, and a version newer than the running one; an artifact only with the manifest's size and sha256; `install_update` is user only and needs `confirm`; checks are off until the user turns them on and send no identifiers (ADR 0011) |
| Local IPC abuse | Socket in the user-only profile dir (0700) / named pipe with the default DACL (only the owner, SYSTEM and admins may write); clients can't claim the `gui` or `runtime` origin |

## 10. Error model
- Per crate: `CoreError`, `IoError`, `SyncError`, `McpError` via `thiserror`. In `app-core` every variant is `Serialize + JsonSchema`, so
  agents get structured errors such as `{"error":"not_found","entity":"thread","id":"…"}`,
  `{"error":"no_mailbox_for_role","account":"…","role":"trash"}`,
  `{"error":"already_exists","entity":"account","key":"…"}`, `{"error":"confirmation_required","command":"…"}`,
  `{"error":"origin_not_allowed","command":"approve_request","origin":"mcp"}` or `{"error":"not_sendable","draft":"…","reason":"…"}`.
- Remote failures are state (`PendingOp.last_error`/`attempts`/`state`, `Account.status`, `SyncStatus.last_error`), not just logs.
  The GUI's status chip and `dail exec list_pending_ops` show them; `retry_pending_ops` and `discard_pending_op` act on them.

## 11. Testing strategy

Each crate builds its integration tests as one binary, `tests/it/main.rs`, with one module per file
(`tests/it/<name>.rs`) and the shared helpers in `tests/it/support/`. Every test binary links all of the crate's
dependencies again (a GUI one is ~0.2 GB), so a new test file is a module, never a new `tests/<name>.rs`.

| Layer | Tooling | What |
|---|---|---|
| app-core unit | std test, `proptest` | each command: ok, every error, undo→redo restores `AppState` equality; the agent policy (`tests/it/agents.rs`: every `user_only` example refused from both MCP origins, undo through history still works); random triage sequences with op results undo and redo to the same view (`tests/it/undo.rs`); threading; search parser (`tests/it/search.rs`, canonical form round trip) and search commands with the server fallback (`tests/it/searching.rs`) |
| app-core replay | JSONL fixtures + `insta` | replaying a log yields a snapshot-identical state and receipts |
| app-io | `insta`, sample `.eml` corpus (`tests/corpus`) | MIME parsing (multipart, charsets, attachments), HTML→text, redb store + migrations, blobs |
| scale | `app-core` `tests/it/scale.rs`, `app-gui` `tests/it/scale.rs` (`--release -- --ignored`) | 100k messages: first fetch, 250 backfill batches, list projection, an incremental fetch; one GUI frame. Budgets catch order-of-magnitude regressions |
| app-sync | `app-testkit` fake transport | effect execution, retries, reconciliation, UIDVALIDITY reset; compose & send (`app-testkit/tests/it/compose.rs`: draft saves, Sent copies, Gmail's own copy, failures); undo-send timing on the virtual clock (`app-testkit/tests/it/undo_send.rs`: undo inside the window cancels, after it the send is irreversible, the window follows `undo_send_ms`, closing inside the window never sends); search (`app-testkit/tests/it/search.rs`: hits outside the window and in bodies merged into the cache, rounds and offline failures in `status.searches`; the fake `UID SEARCH` and its translation in `app-sync/tests/it/live.rs`) |
| app-tls | RFC 8439/7748/4231/5869 vectors, Wycheproof (`testdata/wycheproof/`), `proptest` | every primitive against published vectors (`valid` must pass, `invalid` and `acceptable` fail, except X25519 twist and non-canonical points, which RFC 7748 defines); cross-checks against RustCrypto (`chacha20poly1305`, `x25519-dalek`, `p256`, `p384`; dev-dependencies only); random input never panics a parser or verifier; the provider's algorithm set pinned (`src/tests.rs`); TLS 1.3 and 1.2 handshakes in memory between a rustls client and server on `provider()` with ECDSA and RSA test chains (`testdata/certs/`), 1 MiB each way, and refusals (unknown issuer, wrong name, expired, flipped bit, all-zero X25519 secret) |
| parity | test in app-mcp/app-cli | registry ⇔ MCP tools ⇔ CLI ⇔ schema |
| live (opt-in) | `#[ignore]`, `DAIL_IT_{HOSTINGER,GMAIL}_{EMAIL,PASSWORD}`, `DAIL_IT_KEYRING=1`, `DAIL_IT_TLS=1` | `app-sync/tests/it/live_it.rs`: preset login (and wrong-password rejection) on Hostinger + Gmail, OS keyring round trip, app-tls handshakes with Gmail and Hostinger (IMAP 993, SMTP 465 and 587 STARTTLS; TLS 1.3, ChaCha20-Poly1305, X25519; no credentials); run with `cargo test -p app-sync --test it live_it -- --ignored`. Optional Stalwart/GreenMail via Docker later |
| offline | `app-testkit/tests/it/offline.rs`, `proptest` | triage with the server unreachable (`set_offline`), reconnect, and the fake server matches what the user sees; random sessions (commands, undo, offline toggles, virtual time) converge, and undoing everything restores the server |
| TUI | `app-tui/tests`, ratatui `TestBackend`, `insta` | `Tui` over a real `Engine` (requests dispatched with `Origin::Tui`, replies fed back, a thread-local clock): layout snapshots at 40×20, 80×24 and 120×40; list, reader, palette (every registry command reached by keys), status and help, approvals, composer, sending, mouse, account setup; the sanitizer across every widget (`untrusted.rs`); terminal keys (every bound command has a key a plain terminal delivers); the event loop over a real session (quit, hang-up, signal); attached over IPC end to end; the editor's private file (`editor.rs` unit tests) |
| smoke | `dail batch --backend memory tests/smoke/basic.jsonl` | end-to-end JSON round trip (gate 6) |
| agent e2e | `app-cli/tests/e2e_agent.rs` (CI via `cargo test`) | an MCP client drives `dail mcp --backend memory --fixture basic --profile-dir …` over stdio: discovers tools and resources, is refused `add_account`, waits for the user's `dail account add` (attached over IPC), syncs, reads the inbox through resources, marks a thread read, archives another, replies and edits the draft; `send_draft` stays an approval request that the agent can't approve |
| IPC & audit | `app-sync/tests/it/{ipc,audit}.rs`, `app-cli/tests/it/cli.rs`, `app-gui/tests/it/bridge.rs`, `app-tui/tests/it/attached.rs` | attach over a real local socket (origins, password hand-over, disconnects, `watch`), the profile lock; the shared bridge (tagged replies in order, an owned session shut down, an attached one left running, the owner going away); audit logs that replay to the live state; `exec` attaching to a running `dail mcp`; an attached agent's archive reaching the GUI's state snapshot and waking it; `dail replay` |

`app-testkit` provides `MemoryStore`, `MemoryBackend` (effect log via `executed()`, one-shot fault injection via
`fail_next(reason)`, `set_offline(bool)` to fail every server effect while store and keyring keep working, server edits via `edit_server(|fixture| …)`, an in-memory keyring, a fake local file system for attachments (`add_file(path, bytes)`), a fake SMTP server (`sent()`
lists accepted messages with login, `MAIL FROM` and `RCPT TO`; a fixture account's `smtp_saves_to` makes it file each message
itself, as Gmail does), `VerifyAccount` against the fixture's accounts and optional `password`) and `Fixture` (fake-server content as JSON; built-ins in `crates/app-testkit/fixtures/`, compiled in — `basic` is alice@example.com
with INBOX, Sent, Drafts and Archive; generated: `large` (scale) and `media`, `basic` plus a message from Bob carrying
`fixtures/media/` — a 6000 × 4000 `photo.jpg` with EXIF orientation 6, a 3-page `report.pdf` and a `broken.pdf`
(MISC-19; the ignored `media::regenerate` writes them);
`Fixture::builtin("basic")`, `Fixture::from_path`).

**Android** (ADR 0009): `cargo xtask android check` runs clippy (`-D warnings`) for `aarch64-linux-android` and
`x86_64-linux-android` over the crates the app links (app-core, app-io, app-sync, app-tls, app-front, app-theme,
app-mobile, app-android), then Gradle's `spotlessCheck`, `licensee`, `lint` and the JVM tests; `android apk`
assembles `liveRelease` and `demoDebug`; `android apk-check` checks them (§7.5). `android all` runs the three: it is
the `android` gate (`cargo xtask verify android`), not in the default run; without the rustup targets, a JDK, the SDK
or its build-tools it reports "cannot run". CI runs it as the `android` job. `cargo xtask android test [ARGS…]` builds
the test binaries of app-core, app-io, app-tls and app-sync for `x86_64-linux-android` (API 29) with cargo-ndk's
environment and the NDK's clang, pushes them and each crate's `tests/` and `testdata/` to `/data/local/tmp/dail` on
the device adb sees, and runs them there (`CARGO_MANIFEST_DIR` and `INSTA_WORKSPACE_ROOT` point at the pushed copy,
which is why tests read their data through the run-time `CARGO_MANIFEST_DIR`). The `ipc::` tests are skipped there:
adb's `shell` SELinux domain may not create socket files, and the app does not use the desktop's IPC attach. Without cargo-ndk, the target, the NDK
or a booted device it reports "cannot run". CI runs it as the `android-emulator` job (KVM, API 36 x86_64).

**Packaging** (`packaging/`): the desktop entry and icon (`dail`, matching the window's `app_id` and the notifications'
`desktop-entry` hint), and `[package.metadata.deb]` on `app-cli`: `cargo deb -p app-cli` builds a `.deb` with
`/usr/bin/dail`, the entry and the icon; dependencies come from the binary (`$auto`), a keyring service is recommended.
`scripts/release-ubuntu.sh` also builds an AppImage (`linux/AppRun`, appimagetool pinned by sha256; it needs
`desktop-file-validate` from `desktop-file-utils`) and
`scripts/release-windows.ps1` an MSI (WiX v5, `windows/dail.wxs`, per-machine, fixed UpgradeCode). `release.yml` runs on
`vX.Y.Z` tags behind a guard job (`cargo xtask release check-tag`; the version sites are `SITES` in
`xtask/src/release.rs`, which `cargo xtask release bump <x.y.z>` rewrites); by hand it is a dry run (ADR 0011).
`bump.yml` makes the tag: after `ci` passes on a merge into `main` (`workflow_run`), it bumps the version (patch unless
a `release:minor|major|skip` label), commits it and pushes `vX.Y.Z` with a GitHub App token, so `main` runs
ci → bump → release; `ci` skips the bump commit, and a commit a tag already holds is not released again (MISC-9,
`packaging/README.md`).
`main` is protected by the ruleset in `.github/rulesets/main.json` (MISC-16): PRs only, CI's jobs required, and only
admins and the release App may update it.

**Dependency automation** (Wave 7 D, ADR 0011): Dependabot opens grouped minor/patch PRs for actions, Cargo and Gradle
(`.github/dependabot.yml`); `gradle-verification.yml` regenerates `android/gradle/verification-metadata.xml` on
Dependabot's Gradle PRs into `main` and pushes it to their branch; `toolchain.yml` opens a monthly PR with the newest stable Rust
(`rust-toolchain.toml`) and NDK (the workflows); `advisories.yml` runs `cargo deny check advisories` on `main` every
night and opens or updates an issue when it fails. The gates stay cargo-deny, Gradle's verification and the licence
allowlist; bot PRs are merged like any other.

## 12. Key dependencies

| Purpose | Crate | Why |
|---|---|---|
| IMAP | `async-imap` (tokio) | Mature async client. Hidden behind the `Backend` trait so it can be swapped for `imap-next` (sans-IO) later |
| SMTP | `lettre` (tokio1-rustls, rustls-no-provider) | Submission + MIME builder |
| TLS | `rustls` 0.23 (no built-in provider) + `app-tls`; `rustls-webpki`, `webpki-roots` | rustls keeps the protocol and certificate path building; app-tls (ADR 0003) supplies the crypto in safe Rust, so no C is compiled |
| MIME | `mail-parser` (`full_encoding`), `mail-builder` | Pure Rust (Stalwart), robust charset handling incl. CJK via `encoding_rs` |
| HTML→text | `html2text` | Pure Rust, link-aware |
| HTML sanitizer | `ammonia` | "Open original"; shares `html5ever` with `html2text` |
| Full-text index | `tantivy` 0.27 (app-io; no default features: zstd would compile C) | Body words found offline (ADR 0005) |
| HTML parser | `html5ever` | The reader's documents (`app_io::document`); the parser `html2text` and `ammonia` use too |
| Update signatures | `minisign-verify` (app-io), `minisign` (xtask only) | Ed25519 in minisign format, pure Rust, no dependencies on the verifying side; `app_io::update::verify_manifest` checks signature, trusted comment and version (ADR 0011) |
| Self-update | `self-replace` (app-sync, desktop targets only) | Replaces the running executable, also on Windows where it is locked; pure Rust (ADR 0011) |
| Browser | `webbrowser` (app-gui) | Opens clicked links and the original page |
| Notifications | `zbus` 5 (Linux), `windows` 0.62 `UI_Notifications` (Windows), both app-gui | Desktop notifications; both crates already in the tree (keyring, accesskit); `notify-rust` would add `mac-notification-sys`, which compiles C |
| PDF pages | `hayro` (git, upstream `main` at a pinned commit; `embed-fonts`, `embed-cmaps`) + vendored `hayro-postscript` | Attachment previews on mobile (ADR 0012); pure Rust, no system PDF renderer; `vendor/README.md` says when to drop the patch |
| Images | `image` 0.25 (app-io; png, jpeg, gif, webp), `jpeg-decoder` 0.3 (`platform_independent`) | Inline `cid:` images and attachment previews; pure-Rust decoders; `jpeg-decoder` decodes JPEGs over 4096 px DCT-scaled (forbids unsafe) |
| Store | `redb` | Pure-Rust embedded ACID KV; no `-sys` |
| Secrets | `keyring` 4 | Secret Service via zbus (pure Rust) / Windows Credential Manager |
| MCP | `rmcp` | Official Rust SDK; schemars 1.x |
| HTTP server | `hyper` 1 + `hyper-util` (app-mcp) | `dail mcp --http` on loopback (ADR 0007); pure Rust |
| TUI | `ratatui` 0.30 (crossterm 0.29 through `ratatui::crossterm`), `ratatui-textarea` 0.9 | Immediate-mode terminal UI with a `TestBackend` for headless tests; pure Rust (ADR 0004) |
| GUI | `eframe`/`egui` 0.37 (MSRV 1.95; glow, accesskit, wayland, x11); `egui_kittest` for tests | Immediate mode suits a projection-of-state UI |
| Design tokens | `app-theme` (serde only) | Themes stay data, (de)serializable for theme files later (ADR 0008) |
| Fuzzy | `nucleo-matcher` | Helix's matcher; fast palette |
| IPC | `interprocess` | Cross-platform local sockets / named pipes |
