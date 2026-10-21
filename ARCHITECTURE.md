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
| Platforms | Ubuntu (X11/Wayland), Windows | macOS, Web/PWA (Android/iOS) |
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
  front --> core
  cli --> mcp
  cli --> gui
  cli --> tui
  cli --> sync
  cli --> io
  cli -.ADR 0001.-> tk
  tk -.dev.-> sync
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
commands to submit or keeps the next step).

`app-theme` (ADR 0008) holds the design tokens without a toolkit: colour roles in two surface groups (content and
chrome), the type scale, the metrics, the built-in themes (`dail-dark`, `dail-light`, `dail-dark-hc`,
`dail-light-hc`) and the WCAG contrast `audit`, a unit test over every built-in. It depends on `serde` only. It also
bundles the fonts (`fonts`: Geist and JetBrains Mono, variable in `wght`, with their OFL texts). `app-gui` registers
each font once per weight (400, 500, 600) as the families `sans`, `sans-500`, `sans-600`, `mono`, `mono-500` and
`mono-600`, with egui's own fonts as fallbacks, and maps egui's `TextStyle`s onto the type scale. The icons are a
Material Symbols Outlined subset (`icon`: `Icon` names the glyphs; families `icons` and `icons-filled`). The GUI paints
them (`ui::icon`), so screen readers never read a private-use character; the widget around carries the name.
Mailbox roles map onto icons in `theme::role_icon`, an exhaustive match. Emoji in mail still come from egui's emoji
fonts.

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
                      history_complete: bool /* the oldest server message is cached */ }
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
                      moved_to: Option<MailboxId> }  // mailbox/uid = server location (uid 0: placeholder); shown in
                                                     // location() = moved_to.unwrap_or(mailbox)
pub struct Thread   { id, account, messages: Vec<MessageId> /* oldest first */, last_date_ms: i64, subject: String }
pub struct Addr     { name: Option<String>, email }   pub struct Flags { seen, flagged, answered, draft }
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
                      Move { to: MailboxId, messages }, Copy { to, copies: Vec<CopyRef { source, copy }> } }
pub enum  RemoteStep{ Store { path, uid_validity, uids, flag, value },     // resolved when the op starts
                      Move { path, uid_validity, uids, to: String, create }, Copy { … same … } }
```

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
  `BodyFetched{message, blob, has_attachments}`, `BodyFetchFailed{message, error}`,
  `FlagsChanged{account, mailbox, uid_validity, from_uid, to_uid, flags: Vec<UidFlags>, existing?, highest_modseq?}`, `RemoteOpSucceeded{op}`, `RemoteOpFailed{op, error, permanent}`,
  `AttachmentInspected{draft, path, size, content_type}`, `AttachmentFailed{draft, path, error}`,
  `AttachmentsSaved{message, files}`, `AttachmentsSaveFailed{message, files, error}`,
  `MailboxWatched{account, mailbox, changed}`, `WatchFailed{account, mailbox, error, unsupported}`,
  `OlderFetched{account, mailbox, uid_validity, envelopes, complete}`, `BackfillFailed{account, mailbox, error}`,
  `StoreCompacted{removed_blobs, freed_bytes}`, `CompactFailed{error}`, `IndexSearched{search, hits}`,
  `IndexSearchFailed{search, error}`, `ServersDiscovered{email, imap, smtp, source}`, `DiscoveryFailed{email, error}`, `Closing`,
  `DraftSaved{draft, revision}`, `DraftSaveFailed{draft, error}`, `DraftDeleted{draft}`, `DraftDeleteFailed{account, draft, error}`, `MessageSent{draft, sent_copy_error?}`, `SendFailed{draft, error}`, `SyncFailed{account, mailbox?, error}`, `Tick{now_ms}`,
  `SearchResults{account, mailbox, uid_validity, search, started_ms, mailboxes, envelopes}`, `SearchFailed{account, mailbox, search, started_ms, mailboxes, error}`,
  `Persisted`, `PersistFailed{error}`. JSON form: `{"event": "<name>", "args": {…}}`.
- **`Effect`** — a request to the world: `VerifyAccount{account, login, imap, smtp, secret}`, `ListMailboxes{account, imap: ImapLogin}`, `FetchEnvelopes{account, mailbox, path, imap, uid_validity?, since_uid?, limit}`,
  `ResyncFlags{account, mailbox, path, imap, uid_validity, from_uid, to_uid, changed_since?}`,
  `FetchBody{account, message, path, imap, uid_validity, uid}`, `RunRemoteOp{op, account, imap, steps: Vec<RemoteStep>}`, `InspectAttachment{draft, path}`, `SaveAttachments{message, blob, parts, dir}`, `SaveDraft{draft, account, imap, path, revision, message: OutgoingMessage}`,
  `DeleteDraft{draft, account, imap, path, message_id}`, `SearchMailbox{account, mailbox, path, imap, search, started_ms, mailboxes, query: Query, gmail_raw, limit}`,
  `SmtpSend{draft, account, smtp: SmtpLogin, message: OutgoingMessage,
  sent_copy?: SentCopy{imap, path}}`, `StoreSecret{account, secret, value: Secret}`,
  `DeleteSecret{account, secret}`, `PersistState{delta}`, `ScheduleTick{at_ms}`, `WatchMailbox{account, mailbox, path, imap}`,
  `FetchOlder{account, mailbox, path, imap, uid_validity, before_uid, limit}`, `CompactStore{keep}`, `SearchIndex{search, words, limit}`, `DiscoverServers{email, domain}`. JSON form: `{"effect": "<name>", "args": {…}}`.
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
   `app_io::mime::has_attachments`) or `BodyFetchFailed` (`body` becomes `failed{error}`). Bodies don't count as sync pending.
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
  senders whose inline images show) and one `[[account]]`
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
  `AccountRemoved` or `Mail(MailDelta)` (`mailboxes`, `messages`, `threads`, `ops`, `drafts` upserted; `removed_mailboxes`,
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
  (`get_settings`, `search`, …) open in a read-only window. Commands chosen lately rank higher (frecency, kept for the
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
  It shows `view.composer` in place of the reader (UI-8: a header with the title, the save state and *Close*; labelled
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
  file system) writes each part under `app_io::mime::safe_file_name` (last path component, no control or
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
  (`images::decode`: the `image` crate, PNG/JPEG/GIF/WebP sniffed from the bytes, at most 4096 px per side and 96 MiB),
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
- Layout: a resizable sidebar (accounts and their mailboxes from `app_core::listing::mailboxes`, unread counts; collapsed
  to a rail by `toggle_sidebar`), the thread list (`listing::visible_threads`: the `view.search` results, else the open
  mailbox, newest first; rows from `listing::ThreadSummary`, laid out only while visible), and the
  reader (toolbar acting on the open thread, one card per message, body text). The list has a 48 px header (title, the
  unread badge, *Clear* during a search, the checked-threads bar) and the footer; its rows come in two densities
  (`list::Density`): cozy, 48 px on two lines (check box, account tag, unread pip, the sender in 600 when unread, star,
  clip, message count and the date in mono; then the subject), and compact, 40 px on one line (the sender in a fixed
  column). Hover and selection fills, the keyboard cursor's 2 px bar and the hairline come from `ui::row_frame`. The
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
  cozy (0) or compact (1) list rows. The `text_scale` setting (percent)
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
  and bold when focused, only from 80 columns. Only the terminal's own colours: default foreground and background,
  bold, dim and reverse, so light and dark themes both work and `NO_COLOR` holds.
- **Mailboxes and list.** The mailboxes pane lists each account (bold) and its mailboxes (`listing::mailboxes`: role
  names, unread counts), the open one marked `▸` and reversed. The thread list is `listing::visible_threads` with rows
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
  (`get_settings`, `search`, …) open in a scrollable read-only view (`ResultView`) marked as untrusted data, closed
  with `q`, Esc or Backspace. `app-tui/tests/it/palette.rs` walks every registry command through the TUI palette by
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
- **Untrusted text** (`app_tui::text`): every widget draws email content and an agent's draft text through one
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
  `Origin::Tui`, replies fed back) and snapshot the screen with `insta`. `cargo run -p app-tui --example demo [open]
  [compose] [agent] [palette]` opens it on the memory backend.

## 8. Initial Command catalogue (MVP)

| name | kind | args (sketch) | milestone |
|---|---|---|---|
| `get_settings` | Query | — | M0 |
| `set_keybinding` / `set_key_preset` | Mutation | `command, keys?` (none: back to the preset's; `[]`: unbound) / `preset: gmail | vim`; in `settings` (`config.toml`), undoable | Wave 2 |
| `set_setting` | Mutation | `key: SettingKey, value` (`undo_send_ms`, `poll_secs`, `initial_window`, `history_cap`, `theme`: 0 system / 1 light / 2 dark (3 and 4 still accepted: they set dark mode with `dail-dark-hc` or light mode with `dail-light-hc`), `text_scale`: 50–300 %, `notifications`: 0 off / 1 on, `list_density`: 0 cozy / 1 compact) | M0 |
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
| `list_mailboxes` | Query | `account?` → `{mailboxes: [{id, account, path, role, unread, total, cached, window, backfilling}]}` (inbox first) | M2 |
| `list_threads` | Query | `mailbox, offset = 0, limit = 50 (1–500)` → `{total, threads: [{id, messages, in_mailbox, unread, flagged, has_attachments, last_date_ms, untrusted_content: {subject, from}}]}`, newest first | M2 |
| `get_thread` / `get_message` | Query | `id, include_body = false` (+ `rich_body = false` on `get_thread`: also `body.document` for HTML bodies) → thread `{id, account, last_date_ms, untrusted_content: {subject}, messages: [msg]}` / `{message: msg}`; msg = `{id, account, mailbox, thread, date_ms, flags, size, has_attachments, body: {state}, downloading, untrusted_content: {subject, from, to, cc, message_id, body?: {text, text_source, attachments}}}`; `include_body` starts downloads of missing bodies | M3 |
| `save_attachments` | Mutation (no history; user only, §9) | `message, parts? (MIME part numbers; none: all), dir` (absolute; the body must be downloaded) → emits `SaveAttachments`; progress in `status.saves` (one per message, at most 16) | Wave 2 |
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
| `open_unified_inbox` | View | — (`g u`) → `{threads}`: `view.unified`, the list shows every account's inbox newest first (`listing::unified_threads`); `open_mailbox` leaves it, a search shows instead of it | Wave 2 |
| `open_mailbox` / `select_threads` / `open_thread` / `close_thread` | View | `id` / `ids` / `id` / — (`open_mailbox` clears selection and reader; `open_thread` selects the thread unless it is already selected and downloads missing bodies; nothing is marked read) | M3 |
| `mark_read` / `mark_unread` | Mutation | `ids?: [ThreadId], messages?: [MessageId]` (at least one; a thread means all its messages, a message includes its copies in other folders) → `{changed, ops, cancelled}`; undo restores exactly `changed` | M3 |
| `star` / `unstar` | Mutation | same as `mark_read`; `star` on a thread stars its newest message | M3 |
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
| `focus_pane` / `toggle_sidebar` | View | `pane: sidebar\|list\|reader\|composer` / — (`reader` needs an open thread, `composer` an open draft, else `invalid_argument`; focusing the sidebar expands it. After every command and event the engine moves the focus off a pane that is no longer shown: to the composer, the reader or the list) | M6 |
| `get_view` | Query | — → the `ViewState` (`palette_open, mailbox, selected, thread, search?, composer, focus, sidebar_collapsed`); also the MCP resource `dail://state/view` | M7 |
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
| Agent goes beyond triage and drafting | One policy per command in the registry (`CommandSpec.agents`: `allowed`, `approval`, `user_only`; ADR 0002), enforced by `Engine::dispatch`. User only: `add_account`, `update_account`, `set_password`, `set_identity`, `remove_identity`, `save_attachments`, `restore_draft`, `approve_request`, `reject_request` → `origin_not_allowed` (`save_attachments` writes files where the caller says; an agent could plant one that gets run). `undo`/`redo` apply the engine's own history and are not checked again. The policy binds the MCP origins: a process that runs `dail exec` as the user (or claims `cli` on the profile socket) is the user |
| Agent exfiltrates local files | `attach_file` takes any readable path. The file leaves the machine when the draft is sent (agent sends need approval; the card lists attachments with their paths) and when the draft is saved to its account's Drafts mailbox, on a server only the user sets up (`add_account`/`update_account` are user only). An agent can't hand the engine a draft with a forged attachment check: `restore_draft` is user only, and it checks every attachment again anyway |
| Agent mass-deletes | No permanent delete in the MVP besides `remove_account` (agents may run it: `Destructive`, needs `confirm`, MCP `destructiveHint`); trash is undoable |
| Credential leak | Keyring only; `SecretRef` in state; `Secret` redacts itself in `Debug` and `Serialize` (receipts, effects, replay logs); the password goes only to the servers of its account, which agents can't change |
| Tracking pixels / remote content | Never fetched; links shown with real target. `show_images_from` (agents may run it) only lets the reader draw parts carried in the message itself (`cid:`), decoded off the UI thread with size limits (PNG, JPEG, GIF, WebP; 4096 px per side) |
| Terminal escape injection (TUI) | Email text and agents' drafts are drawn through one sanitizer (controls, ESC, DEL, bidi overrides replaced; `app-tui/tests/it/untrusted.rs`); no OSC 8 links, no window title, no browser or file dialog (they would open on the SSH host) |
| MITM | rustls with app-tls (ADR 0003: verification tested against Wycheproof and RustCrypto, strict DER, whole-encoding PKCS#1 comparison), webpki roots, no plaintext, no "accept invalid certs" setting |
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
with INBOX, Sent, Drafts and Archive;
`Fixture::builtin("basic")`, `Fixture::from_path`).

**Packaging** (`packaging/`): the desktop entry and icon (`dail`, matching the window's `app_id` and the notifications'
`desktop-entry` hint), and `[package.metadata.deb]` on `app-cli`: `cargo deb -p app-cli` builds a `.deb` with
`/usr/bin/dail`, the entry and the icon; dependencies come from the binary (`$auto`), a keyring service is recommended.
AppImage, MSI and a release workflow are still to come (`ROADMAP.md`).

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
| Browser | `webbrowser` (app-gui) | Opens clicked links and the original page |
| Notifications | `zbus` 5 (Linux), `windows` 0.62 `UI_Notifications` (Windows), both app-gui | Desktop notifications; both crates already in the tree (keyring, accesskit); `notify-rust` would add `mac-notification-sys`, which compiles C |
| Images | `image` 0.25 (app-gui; png, jpeg, gif, webp) | Inline `cid:` images; pure-Rust decoders, already in the tree via eframe |
| Store | `redb` | Pure-Rust embedded ACID KV; no `-sys` |
| Secrets | `keyring` 4 | Secret Service via zbus (pure Rust) / Windows Credential Manager |
| MCP | `rmcp` | Official Rust SDK; schemars 1.x |
| HTTP server | `hyper` 1 + `hyper-util` (app-mcp) | `dail mcp --http` on loopback (ADR 0007); pure Rust |
| TUI | `ratatui` 0.30 (crossterm 0.29 through `ratatui::crossterm`), `ratatui-textarea` 0.9 | Immediate-mode terminal UI with a `TestBackend` for headless tests; pure Rust (ADR 0004) |
| GUI | `eframe`/`egui` 0.37 (MSRV 1.95; glow, accesskit, wayland, x11); `egui_kittest` for tests | Immediate mode suits a projection-of-state UI |
| Design tokens | `app-theme` (serde only) | Themes stay data, (de)serializable for theme files later (ADR 0008) |
| Fuzzy | `nucleo-matcher` | Helix's matcher; fast palette |
| IPC | `interprocess` | Cross-platform local sockets / named pipes |
