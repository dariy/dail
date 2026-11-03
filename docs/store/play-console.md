# Play Console: drafted answers (STORE-19)

Drafts for the declarations STORE-22 submits. They follow the code as of 2026-11-03; check each against the app before
submitting, and update this file when the app changes what it does.

## Data safety

- **Does your app collect or share any of the required user data types?** No.
  - Mail, contacts and account details are processed on the device and sent only to the mail servers the user adds.
    Google's definition excludes data sent to a server the user chose as the app's function, and data processed only
    on the device: dail has no server of its own.
- **Is all of the user data collected by your app encrypted in transit?** Not applicable (nothing collected); every
  connection to mail servers uses TLS, and plaintext is refused.
- **Do you provide a way for users to request that their data is deleted?** Not applicable: dail holds no data off the
  device. Uninstalling deletes everything on it.
- **Privacy policy URL:** <https://github.com/dariy/dail/blob/main/PRIVACY.md>.

## Foreground services

The manifest declares two types (STORE-14 removed `dataSync`):

| Service | Type | Declaration text |
|---|---|---|
| `SendService` | `shortService` | No declaration needed. Keeps a send alive from the undo window to the server's answer, at most 3 minutes. |
| `PushService` | `specialUse` (subtype "Keeps IMAP IDLE connections open so that new mail arrives as it comes") | The user turns on *Push* in Settings. The service holds IMAP IDLE connections to the user's own mail servers, which have no push API to replace them (FCM needs a server dail doesn't have). It shows a persistent notification with *Stop*; the periodic JobScheduler check stays as the fallback. Video: Settings → Background sync → Push, the notification, a new message arriving. |

If Play refuses `specialUse`, push leaves the Play build and stays on F-Droid (STORE-22).

## Content rating (IARC questionnaire)

- Category: Utility, Productivity, Communication or Other → **Communication**.
- Violence, sexuality, language, controlled substances, gambling: **No** for all.
- **Does the app allow users to interact or exchange content?** Yes: email between users. dail shows content from the
  user's own mail accounts; it has no social features, no public content and no user discovery.
- **Does the app share the user's location?** No. **Digital purchases?** No.

## Target audience and content

- Target age groups: **18 and over** (a general-purpose mail client; not designed for children).
- **Appeals to children?** No.
- Ads: **No ads.**

## App access

dail needs a mail account to show anything. Reviewer access: the dedicated reviewer mailbox of STORE-22 (never the
developer's own address), with its IMAP and SMTP servers and an app password, entered in *Add an account → Other
IMAP / SMTP*. The demo build is not uploaded to Play.

## Other declarations

- **Government app, financial features, health:** No.
- **News app:** No.
- **COVID-19 contact tracing:** No.
- **Permissions:** `INTERNET`, `ACCESS_NETWORK_STATE`, `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE`,
  `RECEIVE_BOOT_COMPLETED` (re-arms the check and push after a reboot), `POST_NOTIFICATIONS` (new mail, failed sends;
  asked for at run time). xtask's `APK_PERMISSIONS` pins the list.
