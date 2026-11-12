# A proxy site's commands get PORT from "Forward to" (D-041, v0.12.4)

> Date: 2026-11-12 · Branch: `feat/command-port-env`

## Why

The owner set devid-site's "Forward to" to 3200 and the site answered 502. Nothing listened there: its `npm run dev` is `next dev -p 4321`, and no command was saved. The owner then asked how a project avoids a port another project already uses (3000), and asked for `PORT` to follow "Forward to".

## Change

- **`Commands::start`** takes the port to set as `PORT`. It is set after the login shell's environment, so it replaces a `PORT` from the profile, and a `PORT=` in the command line still replaces it.
- **`Site::dev_port`:** the upstream for a proxy site, else none.
- **Callers:**
  - `command_start` passes the site's `dev_port`.
  - The resume after an update (D-040) does the same.
  - Share passes none.
- **App:** "Forward to" now says "Commands get it as PORT."
- **Version:** 0.12.4. **Decision:** D-041, which amends D-036.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`: pass.
- New `a_proxy_sites_commands_get_its_port`:
  - A PHP site's command prints no `PORT`.
  - After `devid app set app --mode proxy --upstream 45123`, a command prints `port=45123`.
  - `PORT=4999 sh -c 'echo port=$PORT'` prints 4999.
- **Mutation check:** without the `PORT` line, the test fails at the proxy case.
- **A first version of the test** restarted the same command right after it printed. The daemon could still count it as running and skip the start. Each case now has its own command.
- The owner's saved commands do not change behaviour (see D-041's trade-off).
