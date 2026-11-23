# 0017 — Self-hosted server: `dail serve` and remote profiles

- Status: proposed
- Date: 2026-11-20
- Plan: `ROADMAP.md`, Wave 18 (SRV-0 to SRV-8, SRV-QR1 to SRV-QR3)

## Context

Sorting rules, polling and scheduled sends run only while a dail is open (ADR 0015: "rules run in dail"). The plan was
to export rules as a Sieve script (SET-9). Neither reference provider offers ManageSieve (Hostinger and Gmail, probed
2026-11-04), so that export would help only self-hosted Dovecot users. Every device also repeats the same work: each one
syncs, runs rules and holds the account passwords.

Self-hosters and users with a home server or a VPS often already run an always-on machine on a private network such as
Tailscale. A dail on that machine could do the work for every other device. Three shapes were considered:

1. **An IMAP and SMTP server in front of dail's cache.** Any mail client could use it. But it means writing an
   IMAP4rev1 server (UIDs, UIDVALIDITY, IDLE, flags, MOVE, CONDSTORE), which is many times the size of the other two
   options. dail's own features (approvals, undo, receipts, the registry) would not cross it.
2. **An effect-level proxy.** Each device keeps its own engine and forwards its network effects to the server. Two
   engines would then decide about the same mail, and keeping them consistent is the hard part.
3. **The command bus over the network.** The server runs the only engine that talks to mail servers. Devices send it
   `Command`s, as `dail tui` and `dail mcp` already do to a GUI over the profile socket (ARCHITECTURE §5.4).

## Decision

Shape 3, with offline use on the devices from the first version.

### The server (A)
- `dail serve [--listen <addr:port>]… [--advertise <host>] [--allow-send]` owns a profile headless. It runs a live
  session (sync, rules, retries, IDLE, the outbox, scheduled sends) and the local profile socket, and logs through
  `tracing`. It has no GUI. It never updates itself unless the user runs `dail update` there.
- Any listen address is allowed, because the transport is always TLS with a device token. Addresses that are neither
  loopback, private (RFC 1918), CGNAT (`100.64.0.0/10`, Tailscale) nor ULA get a warning.

### Transport and authentication
- TLS always, one code path, even inside Tailscale. The server's P-256 key is generated once into `<profile>/serve/`
  with user-only permissions. Its self-signed certificate is built with RustCrypto's `x509-cert` and signed with `p256`,
  which is already a dependency. `rcgen` is not used: its key generation needs ring or aws-lc (AGENTS.md §5). The
  server side uses `tokio-rustls` with the app-tls provider (ADR 0003, 0014).
- Clients pin the certificate's SHA-256 fingerprint. They never consult web roots for the server.
- **Pairing.** `start_pairing {role, name}` (UserOnly; also `dail serve pair`) creates a single-use code: 8 base32
  characters, valid 10 minutes. After five wrong attempts per minute from one peer, the code is burnt. The code
  appears in the receipt and never in state. Exchanging it returns a device token of 32 bytes from `getrandom`. The
  device keeps the token in its OS keyring behind a `SecretRef`. The server keeps only the token's SHA-256, with the
  device ID, name, role and times, in `config.toml` `[[serve.devices]]`. `revoke_device` is Destructive and UserOnly.
- **QR code.** The code is also shown as a QR code of the URI
  `dail://pair?v=1&h=<host>&p=<port>&c=<code>&fp=<sha256 hex>&n=<name>`. The QR code appears in the terminal
  (half-blocks), in the TUI, and in the GUI's *Pair a device…* dialog, so a phone can be paired from any connected
  desktop. Phones read it through a `dail://pair` deep link (from the system camera) or through an in-app scanner:
  CameraX with `zxing-core` on Android (pure Java, no Play services), and AVFoundation on iOS. The raw string goes to
  Rust, and `app_front::pairing::parse_uri` validates it strictly as untrusted input. The matrix comes from the pure-Rust
  `qrcode` crate without its `image` feature, in `app_front::qr`. This adds the dependency edge `app-cli ← app-front`.
- **Roles.** A `user` device may claim the `gui`, `tui`, `cli` and `batch` origins, so a remote GUI can approve. An
  `agent` device may claim only `mcp` (`mcp_allow_send` when the server runs `--allow-send`). The agent policy
  (ADR 0002) and approvals apply on the server unchanged.

### Protocol
The newline-delimited JSON-RPC 2.0 of the profile socket, generalized over any byte stream:
- `hello {token, protocol, client_version}` comes first. In version 1 the protocol numbers must match exactly.
- `submit {origin, command, request_id, seed}`. `seed` is the client's `id_seed`, so IDs created offline (a draft) are
  the same on the server and later commands still name them. `request_id` makes a resend after a dropped connection
  harmless: the server answers the last 1024 per device from memory.
- `preview` and `flush` (as today), `state` (a snapshot with a `revision`), and `watch {since}`. `watch` sends
  `changed {revision, diff}`, or `resync` when the client is too far behind or the server restarted.
- `blob {id}` for bodies and attachments, and `put_file` for files attached on a device.

### The device (B)
- A **remote profile**: `config.toml` `[remote] host, port, fingerprint, token`, and no accounts. A config with both
  is refused.
- B runs its own engine over a **mirror** of the server's state. The engine gains two pure pieces:
  - `Command::placement()`, an exhaustive match. `Server` covers mutations, outbound, destructive, account and rule
    commands, and searches. `Client` covers every `View` command (each device keeps its own view) and update commands
    (they concern the device's own binary). Commands with local paths are `ServerWithUpload` (`attach_file`) or
    `ClientWithBlob` (`save_attachments`).
  - `StateDiff` (`diff`, `apply`; `view` excluded) and `Event::RemoteSynced`, ingested through the existing second
    entry point. The Core Law keeps exactly two entry points.
- **Mirror mode.** While `state.link` is set, B applies server commands locally at once (optimistic) but emits no
  network effects. The server's next diff overwrites the result. Rules, polling and scheduled sends run only on the
  server.
- **Offline.** Commands are queued in `state.link.queued` (persisted) and replayed in order on reconnect, before the
  snapshot is taken (so the snapshot already holds them, and takes back only what the server refused). A rejected command becomes `state.link.rejected`, visible like any other problem (AGENTS.md §4).
  Destructive and UserOnly account commands are refused offline with a new error code instead of being queued.
  Outbound commands are queued, and the server applies its undo window when they arrive.
- **Bodies offline.** B caches blobs in its own store and pre-loads the newest bodies through `blob`.

## Consequences

- Rules, polling and scheduled sends run all the time for users with a server, on every provider, without Sieve.
  SET-9 is postponed, not dropped.
- Passwords live on the server only. A phone holds a revocable device token, so a lost phone is one `revoke_device`.
- Every front end gets the remote mode from one `Connection::Remote`, because they all reach a session through
  `Connection` and `Bridge`.
- dail now listens on the network. The attack surface is TLS, `hello` with a 256-bit token, and pairing with
  rate-limited single-use codes. A stolen token can do what its role allows until it is revoked. A user token can
  approve sends.
- Only dail can be a client. Other mail clients keep using the mail servers directly.
- New dependencies: `x509-cert` (with `der` and `spki`) and `qrcode` (`rqrr` for tests only). On Android, CameraX and
  `zxing-core`. Each is checked against `deny.toml` or the Gradle licence allowlist when it is added. Their exact
  feature sets are confirmed in SRV-3 and SRV-QR1.
  - SRV-3: `x509-cert` 0.3 with `default-features = false, features = ["builder", "std"]` (`der` and `spki` through
    its re-exports, not as direct dependencies), `p256` with `pkcs8` in app-sync, and `base64ct` (`alloc`) for the wire.
  - SRV-5a: `[remote]` in `config.toml` is the source of truth for the link (`state.link.server` mirrors it; the engine
    writes it through `PersistDelta::Remote`). A forwarded command returns the server's receipt; a refusal of what the
    mirror applied resets the mirror to a fresh snapshot. Until SRV-5b the runtime is online only: a server command
    while the link is down is refused with `ERR212` and nothing is applied (ARCHITECTURE §5.6).
  - SRV-QR3: CameraX 1.7.0 (`camera-core`, `camera-camera2`, `camera-lifecycle`, `camera-compose` for a Compose
    preview; not `camera-view`, which brings `camera-video` and media3) and `com.google.zxing:core` 3.5.4, all
    Apache-2.0, with checksums in `verification-metadata.xml`. `camera-core` ships two AOSP-built JNI libraries
    (`libimage_processing_util_jni.so`, `libsurface_util_jni.so`) that CameraX loads itself, so they join
    `PREBUILT_ALLOWLIST` beside Compose's graphics-path one, each with its reason (AGENTS.md §5); like it, they come
    from Google's Maven repository, which F-Droid accepts for AndroidX. `android.permission.CAMERA` is asked for only
    when the scanner opens. iOS uses AVFoundation, and its camera prompt is the app's first localised resource
    (`InfoPlist.strings` in seven `.lproj` folders, written from the i18n table).
- The wire format is public API like `Command` JSON (AGENTS.md §5): versioned through `hello.protocol`.

## Amendment 1 — accounts from devices (2026-11-21, proposed)

Plan: `ROADMAP.md`, Wave 18.5 (SRV-9 to SRV-12).

### Context
The mail accounts live on the server. A paired user device can already send the account commands (`add_account`,
`update_account`, `set_password`, `set_identity`, `remove_identity`, `remove_account`, `test_account`,
`discover_servers`), because they are Server-placed and a `user` role may claim the `gui`, `tui`, `cli` and `batch`
origins. But two things are missing:

1. **A bug.** The wire form of a command (`wire_command`, app-sync) restores the real value of a `Secret` only for
   `add_account`. `set_password` therefore reaches the server, or the owner of a local profile socket, as the literal
   text `<redacted>`, which would then be stored as the password. The local socket has had this bug since Wave 1.
2. **No choice per device.** Every device mirrors every account. A user wants, for example, a phone to carry 3 of the
   server's 5 accounts while the desktop carries all 5.

A device keeping accounts of its own next to the server's (a hybrid profile) was considered and left out: it would
undo the rule that only the server talks to mail servers, and with it mirror mode, the offline queue and the
"passwords only on the server" guarantee.

### Decision
- **Secrets on the wire.** Every `Secret` field of a `Command` is carried by the wire form. The commands that hold one
  are named by an exhaustive match (no `_ =>`), so a new one cannot be forgotten. Receipts, logs and replay files keep
  `<redacted>`.
- **Account scope per device.** `Device` gains `accounts: AccountScope` (`All` | `Only(set of AccountId)`), kept by the
  server in `state.devices` and in `config.toml` `[[serve.devices]]` (`accounts = [..]`; missing means `All`, so
  existing servers keep their behaviour).
  - `set_device_accounts {device, accounts}` sets it: UserOnly, Server-placed, refused offline. A device learns its own
    `DeviceId` from the `hello` answer (`state.link.device`).
  - `start_pairing` takes an optional `accounts`, so a phone can be paired with its scope chosen in advance.
  - An account added from a scoped device joins that device's scope. Other scoped devices do not get it.
- **The server enforces the scope.** It is not only a filter on the screen:
  - `state`, `watch` diffs, `blob` and search answers for a device contain only its accounts and what belongs to them
    (mailboxes, threads, messages, drafts, outbox, approvals, per-account status). A change of scope sends that device
    a `resync`.
  - A command from a device that names an account outside its scope is refused with a new error code.
  - The device drops the mirror and the cached bodies of an account that leaves its scope.
  - So an `agent` device limited to one account can neither read nor act on the others. A `user` device can widen
    its own scope, so for user devices the scope is a choice of what to carry, not a security boundary.
- **Front ends.** *Devices* in settings gets an account checklist per device. A linked device gets *Accounts on this
  device* in its Accounts settings, and the *Pair a device…* dialog offers the choice. The account controls of a
  linked device act on the server's accounts, and say so.

### Consequences
- A phone stores, downloads and pre-loads only the accounts it carries. A lost phone exposes fewer accounts.
- `watch` computes a diff per scope instead of one for every watcher, and the server keeps one previous state per
  scope.
- The wire protocol gains the scope in `hello`'s answer and in `state`/`watch`. Protocol version 1 is not yet
  released, so it stays 1. After the first release, a change like this needs version 2.
- Rules, polling and scheduled sends still run for every account on the server, whatever the devices carry.
