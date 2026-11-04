# dail — privacy policy

Effective 2026-11-03. This policy covers dail on Android (Google Play, F-Droid and the GitHub releases), on Linux,
Windows and iOS.

## In short

dail collects nothing. There are no dail servers, no accounts with dail, no analytics, no advertising and no crash
reporting. Your mail goes only between your device and the mail servers you add.

## What dail stores, and where

- **Your mail**, the accounts you add and your settings are stored on your device, in dail's own app storage.
- **Passwords** are stored only in the system's protected credential store: Android Keystore-protected storage, the
  iOS Keychain, the Secret Service on Linux, the Credential Manager on Windows. dail never writes them anywhere else.
- **Logs** stay on your device and hold no passwords and no message text. On Android they are limited to about 2 MiB
  and the replay logs to 14 days.

Uninstalling the app deletes all of it from the device. Your mail stays on your mail servers.

## Who dail talks to

- **Your mail servers**, over IMAP and SMTP with TLS, to read, sync and send the mail of the accounts you add.
- **Server lookup**, only when you tap *Find servers* while adding an account: your provider's autoconfiguration
  address and Mozilla's public list of mail servers (autoconfig.thunderbird.net). They receive the domain of your
  address, not the address or a password.
- **Update checks** on Linux and Windows, only if you allow them: GitHub, for dail's signed list of releases. Nothing
  about you or your mail is sent. dail on Android and iOS never checks; the stores update it.

dail never loads remote images on its own and never opens a link without showing you its real target first. Opening
a link hands it to your browser.

## Diagnostics

If dail stops working, *Copy diagnostics* puts the app version, the device model and the last log lines on your
clipboard. Nothing is sent: you choose whether to paste it into a message or an issue.

## Children

dail is a general-purpose mail client and is not directed at children.

## Changes and contact

Changes to this policy are made in this file, with the date above. Questions: open an issue at
<https://github.com/dariy/dail/issues>.
