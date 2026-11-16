# The inbox in the daemon and the CLI (Phase 9 M6a, D-049)

> Date: 2026-11-16 · Branch: `feat/mail-inbox-api`

## What changed

- **Requests:**
  - `mail_list {query, unread, limit}` (newest first, or Mailpit's search, with `is:unread` for unread only);
  - `mail_message {id}`, `mail_headers {id}`, and `mail_raw {id}` (cut at 512 KB);
  - `mail_read {ids, read}` (no IDs: all), `mail_delete {ids}`, and `mail_save {id, part}`.
  - They need a running Mailpit, like `mail_test` and `mail_clear`, and share one dispatch, `mail_request`.
- **`daemon::mail`:**
  - The HTTP/1.0 helper returns bytes, so attachments come through whole.
  - `check_id` keeps message IDs to letters and digits.
  - `encode` percent-encodes the search.
  - `listed` and `shown` turn Mailpit's JSON into what the clients show.
  - `save_part` writes to ~/Downloads: `file_name` keeps only the last path part, without control characters or a leading dot; `free_path` adds " (2)", " (3)" instead of overwriting.
- **CLI:**
  - `devid mail list [query] [--unread] [--limit n]`, one line per message with `*` for unread;
  - `devid mail show <id>`: the From, To, Cc, Date and Subject lines, each attachment with its `save` command, then the text;
  - `devid mail delete <id>…` and `devid mail save <id> <part>`.
- **`scripts/smoke.sh`:** after the test mail, `list` must find it and `show` must print its text.

## Checks

- Unit tests:
  - message IDs (`../info`, `a/b`, `a?b`, spaces and over-long ones are refused);
  - the search encoding;
  - the list and message shapes, including a sender without a name and a message without Cc;
  - attachment names (`../../.ssh/authorized_keys` becomes `authorized_keys`, a dot file loses its dot);
  - free paths, and replies as bytes.
- End to end on the owner's Mac: a test daemon with the real Mailpit 1.33.1, its HOME in a temp folder.
  - `list`, the filter `--unread invoice`, `show` (which marked the message read, 2 unread to 1), `delete`, and a refused `../info` all behaved as expected.
  - `save` stored an attachment the sender named `../invoice.pdf` as `Downloads/invoice.pdf`.
