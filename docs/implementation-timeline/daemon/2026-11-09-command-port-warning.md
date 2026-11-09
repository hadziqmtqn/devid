# Starting a command warns when the dev server port is held outside Devid (v0.12.2)

> Date: 2026-11-09 · Branch: `feat/command-port-warning`

## Problem

The owner pressed Start on erd-builder-pro's `start` command (`npm run start`) in Devid.app. It ended at once with `listen EADDRINUSE: address already in use 0.0.0.0:3000`, which only its log showed.
- Port 3000 was held by an older `npm start` of the same project, started on 2026-11-05 from a Terminal that had since closed (its npm's parent was pid 1).
- That instance was the one `erd-builder-pro.test` proxied to.
- With the owner's go-ahead it was stopped (SIGINT to its process group; all five processes ended at once), and `start` was started from Devid. Port 3000 answered after 8 s, and `https://erd-builder-pro.test` answered 200.

## Change

- `command_start` adds `warning` to its reply when the site is a proxy site, none of its commands runs, and its dev server port (`upstream`) already has a listener. The warning names the holder from `lsof`: "port 3000 is already in use by node (pid 83670), which Devid did not start; a dev server on that port cannot start until it stops".
- The start still goes through, because the command may not use that port (a queue worker, for example).
- When one of the site's own commands runs, there is no warning: the port may be that command's.
- **Clients:** the CLI prints `warning: …`, the app shows it as a red toast, and the TUI's footer shows it after the done message, marked as a problem.
- **Version:** 0.12.2.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`), `cargo test`: pass.
- New `a_busy_dev_server_port_is_named_when_a_command_starts`: a listener the test holds on the site's upstream port gives the warning, and the command still runs. With a command of the site running, a second start gives no warning.
- With the condition inverted on purpose, the test fails.
