# Setting up a dail server

`dail serve` runs one headless dail on an always-on machine: a home server, a NAS or a VPS. It holds your mail
accounts and passwords, and it runs sync, sorting rules, polling and scheduled sends all the time. Your desktop, laptop
and phone connect to it as *devices*: they send it commands, mirror its mail, and keep working when they cannot reach
it.

The design is in ADR 0017. `ARCHITECTURE.md` §5.6 describes how it works.

You need:

- a machine that stays on, with dail installed (the same binary as on the desktop);
- a way for your devices to reach it. A private network such as Tailscale is the easiest; see
  [`serve-tailscale.md`](serve-tailscale.md);
- on Linux, a Secret Service keyring (for example `gnome-keyring`) for the account passwords.

The steps below use the default profile and port `7443`. Every `dail` command accepts `--profile <name>` to use
another profile.

## 1. Prepare the server profile

Add your mail accounts on the server machine first. They live only there.

```sh
dail account add --preset gmail --email you@gmail.com          # asks for the app password, then tests the login
dail account add --preset custom --email you@example.net \
    --imap imap.example.net:993 --smtp smtp.example.net:465
```

`dail doctor` checks that every account's password is in the keyring. It never prints a password.

## 2. Start the server

Try it in the foreground first:

```sh
dail serve --listen 100.101.102.103:7443 --advertise home.tail1234.ts.net
```

| Option | Meaning |
|---|---|
| `--listen ADDR:PORT` | Where to accept devices. Repeat it for several addresses. Without it, dail listens on `127.0.0.1:7443` only, so no other machine can reach it. |
| `--advertise HOST` | The name or address devices use to reach the server, such as a Tailscale MagicDNS name. It goes into the certificate and the pairing link. Repeatable. |
| `--allow-send` | Lets *agent* devices send mail without your approval. Without it, an agent's send creates an approval request. |

On start, dail prints each listening address and the certificate's **SHA-256 fingerprint**. Keep the fingerprint: you
compare it on each device you pair. A listen address that is not loopback, private (RFC 1918), Tailscale (CGNAT) or
IPv6 ULA gets a warning, because the port would then be open to the internet.

The connection is always TLS, with a self-signed certificate that devices pin. The certificate and its key are created
once in `<profile>/serve/`, readable only by you, and stay the same across restarts.

Stop the server with Ctrl+C. It saves drafts and exits.

## 3. Run it as a service (Linux)

To keep the server running after you log out and start it at boot, install the systemd user unit:

```sh
install -Dm644 packaging/systemd/dail-serve.service ~/.config/systemd/user/dail-serve.service
systemctl --user edit --full dail-serve        # put your --listen and --advertise into ExecStart
systemctl --user daemon-reload
systemctl --user enable --now dail-serve
loginctl enable-linger "$USER"
journalctl --user -u dail-serve -f             # the listeners and the fingerprint
```

The keyring must be unlocked when the server starts, which is the main difficulty on a headless machine.
[`packaging/systemd/README.md`](../packaging/systemd/README.md) explains your options.

On Windows, `dail serve` runs in a terminal and stops with Ctrl+C.

## 4. Pair a device

Each device needs a single-use pairing code from the server. Open one on the server machine while `dail serve` runs:

```sh
dail serve pair --role user --name Laptop
```

It prints the code (8 characters, valid 10 minutes), the fingerprint, the server's addresses, a `dail://pair` link
and the link's QR code. Options:

- `--role user` for your own devices. They can do everything you can, including approving sends.
- `--role agent` for a machine that runs an AI agent. It may only act as MCP, under the approvals.
- `--host HOST` sets the host in the link and QR code.
- `--no-qr` hides the QR code.

A device that is already paired can open codes too: the GUI and the TUI have **Pair a device…** in the Accounts
settings, which shows the code and the QR code.

A device must be a profile **without mail accounts**. dail refuses to link a profile that has its own accounts, so use
a new profile for it.

### A desktop or laptop: command line

```sh
dail --profile home remote add home.tail1234.ts.net:7443
```

dail shows the fingerprint the server presents. Compare it with the one from step 2. Only if they are equal, answer
yes and type the code. For scripts, pass both values: `--fingerprint <hex> --code <CODE>`. A different fingerprint is
refused.

### A desktop or laptop: GUI or TUI

On a profile without accounts, the account wizard's first step offers **Connect to a dail server**. Enter the server
as `host[:port]`, or paste the `dail://pair` link. Compare the fingerprint, then type the code. If you pasted a link,
dail refuses a server whose fingerprint differs from the one in the link.

### A phone (Android, iPhone, iPad)

On first run, choose **Connect to a dail server**. You can type the server and the code. You can also scan the QR code
from `dail serve pair` or the **Pair a device…** dialog, in one of two ways:

- tap **Scan QR code** in dail, which asks for camera access the first time;
- use the system camera app, which opens dail through the `dail://pair` link. A link only fills in the form. Nothing is contacted until you tap **Connect**, and the screen names the server
the link points to. Only continue if you opened the code yourself on your own server. The device token is kept in the
Android Keystore or the iOS Keychain.

## 5. Check the link

```sh
dail --profile home remote status
```

It shows whether the link is online, the server and its pinned fingerprint, the number of queued commands, refused
commands and the last error. The GUI and TUI show the same in the status bar.

After pairing, every dail on that profile uses the server: `dail --profile home gui`, `tui`, `exec …`, and `mcp` for an
agent. Commands that change mail run on the server. What you look at (the open mailbox, the theme, the text size)
stays on the device.

## Working offline

If a device cannot reach the server, it keeps working on its copy:

- The mail it has mirrored stays readable, and so do the newest bodies (the latest 200 on a desktop, 100 on a phone,
  INBOX first), which are downloaded after each connection.
- Triage, drafts and sends are applied at once and queued. The queue survives a restart, and it goes to the server in
  order when the link is back. Sends then get the server's undo window.
- A queued command the server refuses is kept aside as a problem. Use **Retry all** or **Dismiss**, or `retry_offline`
  and `dismiss_offline_rejection`.
- Commands that must not run on an old copy are refused offline with `ERR014`, and nothing changes. These include
  emptying the trash, deleting folders, changing accounts and certificates, pairing and revoking devices, approvals
  and `sync_now`.

## Managing devices

```sh
dail serve devices                  # name, role, paired, last seen
dail serve revoke <device-id>       # asks first; --yes for scripts
```

The GUI and TUI list devices under **Devices** in the Accounts settings, with **Revoke**. A revoked device's token stops
working and its connections close at once, so a lost phone is one command away from losing access. To pair it again,
open a new code.

To turn a device back into an ordinary profile, disconnect it (`disconnect_server`, or **Disconnect…** in the link
details). That removes the link and the mirrored mail from the device. The server keeps everything.

## Security notes

- Passwords exist only on the server. Each device holds a revocable token of 32 random bytes in its OS keyring, and
  the server keeps only its SHA-256.
- Devices never trust web certificate authorities for the server. They pin the fingerprint you compared when pairing.
- A wrong pairing code is counted against the peer's address. After five within a minute, every open code is burnt and
  you open a new one.
- Do not forward the port on your router. Prefer a private network and a firewall rule for its interface; see
  [`serve-tailscale.md`](serve-tailscale.md).
- A stolen *user* token can approve sends until you revoke it.

## Troubleshooting

| Symptom | What to check |
|---|---|
| `dail serve` says dail is already running for the profile | Another dail (GUI, TUI) owns that profile. Close it, or serve a different profile. |
| Accounts fail to log in under systemd | The keyring is locked or missing; see `packaging/systemd/README.md`. |
| `remote add` refuses the profile (`ERR013`) | The profile has mail accounts. Use a new profile for the device. |
| `remote add` refuses the fingerprint | The server presented a different certificate. Do not continue until you know why. |
| The pairing code is refused | It is used, expired (10 minutes) or burnt after wrong attempts. Open a new one. |
| `dail serve pair` says the owner is not `dail serve` | The profile is owned by a GUI or TUI, not by the server. Start `dail serve` on it. |
| The device stays offline | `dail remote status` shows the last error. Check that the device reaches the listen address (firewall, Tailscale). |
