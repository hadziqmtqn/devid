# M1: workspace, daemon socket and `devid status`

> Date: 2026-11-02 · Branch: `phase2/m1-skeleton` · Plan: [Phase 2](../../roadmap/phase-2.md)

## Outcome

- A Cargo workspace with `crates/daemon` (library) and `crates/cli` (binary `devid`).
- `devid daemon run` starts the daemon on `~/.devid/run/devid.sock` (mode 0600, JSON lines). It refuses a second instance and removes a stale socket file.
- `devid status [--json]` and `devid daemon stop` talk to the daemon over that socket.
- `~/.devid/config.toml` is created with defaults on first start: `tld = "test"`, `server = "nginx"` (D-015), `default_php = "8.5"`. Invalid TOML is an error and is never silently overwritten.
- `DEVID_HOME` overrides `~/.devid`; the tests use it.

## Verification

- `cargo test`: the config unit test and an end-to-end daemon lifecycle test pass. The lifecycle test covers no daemon, start, status, the duplicate-start refusal, stop with socket cleanup, and restart over a stale socket.
- `cargo clippy --all-targets -- -D warnings` is clean.
- Release binary: 768 KB, linking only `libSystem`. Daemon idle RSS: 0.9 MB.

## Dependencies added

`serde`, `serde_json`, `toml` and `clap`, as listed in the Phase 2 plan.
