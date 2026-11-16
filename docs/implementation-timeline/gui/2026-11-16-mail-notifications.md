# Notifications for new mail (Phase 9 M4, D-050)

> Date: 2026-11-16 · Branch: `feat/mail-notify`

## What changed

- **Daemon:** `status.mail.latest` is the newest message: when Mailpit received it, the sender's name (or address) and the subject. It comes from Mailpit's message list (`/api/v1/messages?limit=1`), the same call that gives the counts, which replaces `/api/v1/info`. The version comes from the file `install` writes.
- **`crates/gui/src/notify.rs`:**
  - Posts through `NSUserNotificationCenter` (objc2-foundation; its NSUserNotification feature is the only new part).
  - A delegate shows banners while the window is in front, and opens the Mail page on Show.
  - Nothing happens outside an app bundle.
- **Menu-bar icon (`tray.rs`):**
  - `mail_news` decides what to post:
    - "Devid Mail · Notifications are on" right after they are turned on;
    - the sender and the subject of a message newer than the last one seen;
    - nothing at the first look, or while Mail is off or stopped.
  - The icon checks every 5 s while Mail runs with notifications on, otherwise every 15 s.
- **Mail page:** the "Notify on new mail" switch, with a note that macOS asks the first time (Options, then Allow).

## How it was chosen (on the owner's Mac, with the owner)

- An ad-hoc signed test bundle that asked UserNotifications for permission got UNErrorDomain 1, "Notifications are not allowed for this application", with no prompt.
- With `NSUserNotificationCenter`, the first notification brought macOS's permission banner. After the owner chose Allow, the test banner showed with its title, subtitle and text.
- A DevidTest bundle (`dev.devid.app.test`, this build) with a test daemon and the real Mailpit:
  - after `devid mail notify on` and Allow, a test email showed as "New mail · Devid · Test email from Devid";
  - the owner asked for Mail.app's layout, then found a snippet too much, and settled on two lines: sender, subject.
- One finding: a message that arrives while the app is still starting does not notify, because the first look only records what is there. This is on purpose, so old mail stays quiet.

## Checks

- Test `new_mail_notifications`:
  - the first look;
  - the same message;
  - a newer one, an older one after a delete, and the first message in an empty inbox;
  - turning on, notifications off, Mail stopped, Mail off, and an empty subject.
- Test `reports_the_newest_message`: the sender's address stands in for a missing name, and an empty inbox has no newest message.
- `cargo clippy -D warnings` and `cargo test`, for `-p devid-gui` and for the daemon and CLI.

## Left

- The test bundles' entries (DevidNotifyTest, DevidNotifyTest2, DevidTest) stay in System Settings › Notifications on the owner's Mac. Their app files are deleted.
