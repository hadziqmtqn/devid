# T4: Server and Logs pages, stop/start, logs

> Date: 2026-11-03 · Branch: `phase4/t4-server-logs` · Plan: [Phase 4](../../roadmap/phase-4.md)

## Outcome

- **Daemon + CLI:**
  - `devid stop|start <service|all>`: a stopped service stays stopped across site changes until started; a daemon restart resets this.
  - `devid logs` lists logs; `devid logs <name> [-n N] [--follow] [--clear]` shows, follows or clears one (log names are validated, never paths).
  - `server_list` reports version, config path and paused state.
- **TUI Server page:**
  - nginx / Caddy / Apache with status, version and active;
  - per-server actions: install (Caddy/nginx from the release, Apache from the system), use (switch), restart, stop/start, view the generated config in `$PAGER`;
  - `S` stop all (confirmed), `A` start all.
- **TUI Logs page:** the log list and the selected log's tail; `f` follow (refreshes every second), `c` clear (confirmed).

## Verification

- `cargo test`: 47 tests pass.
  - New: `logs` unit test (list only `*.log`, tail, read since offset, shrunk file, clear, path traversal refused).
  - New: `stop_start_and_logs` integration test (stop survives a site change, restart refused while stopped, start, stop/start all, unknown service, logs list, `-n`, clear, bad name).
  - New: `server_and_logs_pages` key test, plus Server/Logs render checks.
- **pty session against a temp daemon (release PHP 8.5 + nginx):**
  - stopping nginx in the TUI took the site to `000`, and starting it brought `200`;
  - installing Apache (system httpd) and switching to it served `200`;
  - switching back to nginx served `200`;
  - follow and clear on the Logs page ran, and the TUI exited 0.
- The log cleared in that session was already nearly empty, so its effect was not visible there. Clearing is proven by the CLI integration test (file emptied) and the key test (correct `log_clear` request).
