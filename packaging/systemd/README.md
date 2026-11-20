# dail server under systemd (user unit)

`dail-serve.service` runs `dail serve` (ADR 0017, `ARCHITECTURE.md` §5.6) as a systemd **user** unit: it runs as you,
with your profile (`~/.local/share/dail/profiles/default` unless `--profile`) and your keyring, and needs no root.

## Install

```sh
install -Dm755 dail ~/.local/bin/dail                      # or the .deb: then ExecStart=/usr/bin/dail …
install -Dm644 dail-serve.service ~/.config/systemd/user/dail-serve.service
systemctl --user edit --full dail-serve                    # adjust ExecStart: --listen, --advertise, --profile
systemctl --user daemon-reload
systemctl --user enable --now dail-serve
loginctl enable-linger "$USER"                             # start at boot, keep running after you log out
```

Add the mail accounts to the profile before (`dail account add …`, ARCHITECTURE §7.2); `dail serve` refuses to start
while another dail owns the profile.

## Use

```sh
systemctl --user status dail-serve
journalctl --user -u dail-serve -f                         # listeners, fingerprint, pairings (info)
dail serve pair --role user --name Laptop                  # attaches to the running server; prints the code
dail serve devices
dail serve revoke <device-id>                              # asks first; --yes for scripts
systemctl --user stop dail-serve                           # SIGTERM: saves drafts, exits 0
```

`dail exec`, `dail batch`, `dail tui` and `dail mcp` on the same profile attach to the running server.

The certificate's SHA-256 fingerprint is in the journal at every start (`dail serve: certificate SHA-256 fingerprint
…`) and in the output of `dail serve pair`. Compare it with the one `dail remote add` shows on the device.

## The keyring on a headless machine

Account passwords live only in the OS keyring (AGENTS.md §4). On Linux dail reads them through the **Secret Service**
API on the user's D-Bus session bus (the `keyring` crate's zbus store); there is no file fallback and dail never
prompts. Under a user unit this needs:

1. **A session bus.** systemd's user manager provides one (`$XDG_RUNTIME_DIR/bus`) when `dbus-user-session` is
   installed (`sudo apt install dbus-user-session` on a server install of Ubuntu).
2. **A Secret Service provider whose collection is unlocked** when dail reads a password, for example
   `gnome-keyring` (`sudo apt install gnome-keyring`). The login keyring is unlocked by PAM only when you log in with
   your password; with `loginctl enable-linger` and no login after a reboot, it stays locked. Choose one:
   - log in once after each boot (desktop, or SSH with password authentication and `pam_gnome_keyring`), then
     `systemctl --user restart dail-serve`;
   - unlock it by hand after each boot: `gnome-keyring-daemon --replace --unlock --components=secrets` (reads the
     keyring password from stdin), then restart the unit;
   - give the keyring an empty password. It is then stored unencrypted in `~/.local/share/keyrings/`: acceptable
     only on an encrypted disk of a machine you control.

If the keyring is unreachable or locked, each account's login fails with the keyring's error: it shows in the
account's status (`dail exec list_accounts`) and in the journal. `dail doctor` checks that every account's password
is in the keyring (never printing it); run it in the unit's environment while the unit is stopped:
`systemctl --user stop dail-serve && systemd-run --user --pty --wait dail doctor`.

## Hardening

The unit sets the sandboxing options a user unit can use without privileges: `NoNewPrivileges`, a system-call filter
(`@system-service`), `MemoryDenyWriteExecute`, `RestrictNamespaces`, `RestrictAddressFamilies` (Unix sockets for the
profile socket and D-Bus, IPv4/IPv6, netlink) and `UMask=0077`. Options that need user namespaces (`ProtectSystem`,
`ProtectHome`, `PrivateTmp`) are left out: they fail on systems that disable unprivileged user namespaces.

## Over Tailscale

See [`docs/serve-tailscale.md`](../../docs/serve-tailscale.md).
