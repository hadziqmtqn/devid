# Commands run again after Devid restarts for an update (D-040, v0.12.3)

> Date: 2026-11-09 · Branch: `feat/resume-after-update`

## Why

After the update to 0.12.2 from the app, erd-builder-pro's `start` was stopped: the update restarts the daemon, and a daemon stop ends every command (D-036). The owner asked for the commands that were running to come back.

## Change

- **New request `remember_running`:** writes the running commands (not shares) to `~/.devid/run/resume.json` with the time.
- **`devid setup`:** sends it before it reloads the LaunchAgent. `self-update` runs the new binary's `setup`, and the app's update runs `self-update`.
- **New daemon:** after `apply`, a background thread starts the noted commands again, each log headed "started again after Devid restarted for an update", then removes the note. Removing it last makes "note gone" mean "resume done".
  - A note older than 10 minutes, a removed command and a site that is gone are skipped.
- **Unchanged:** login, crash, `devid daemon stop` and logout write no note, so nothing starts. Shares are not started again.
- **Version:** 0.12.3. **Decision:** D-040, which amends D-036.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (also `-p devid-gui`), `cargo test`: pass.
- New `commands_run_again_after_an_update_restart_only`:
  - With `dev` running and `queue` stopped, `remember_running` notes `dev` only.
  - After a stop and a new daemon, `dev` runs again with the log header, `queue` stays stopped, and the note is gone.
  - A plain stop starts nothing, and a note from long ago is dropped without starting anything.
- **Mutation check:**
  - With the age check removed, the first version of this test still passed, because the note was removed before the commands started and the test checked too early.
  - Removing the note last fixed both. The same mutation now fails the test.
- Works from the update after 0.12.3 is installed: the daemon being replaced must know `remember_running`.
