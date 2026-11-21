# Fix: `devid … | head -1` no longer panics on the broken pipe

> Date: 2026-11-22 · Branch: `fix/cli-broken-pipe`

## Problem

When the reader of `devid`'s output stopped early (`devid status | head -1`, `devid cmd front | grep -q x`), the CLI could end with `thread 'main' panicked at …/std/src/io/stdio.rs: failed printing to stdout: Broken pipe (os error 32)` and exit 101. It happened whenever `devid` wrote after the reader had exited, so it came and went with timing. During Phase 10 O4 testing (2026-11-21) it made the O3 smoke check `devid cmd front | grep -q 'on demand: running'` fail at random under `set -o pipefail`. O4 worked around it with `grep … > /dev/null`.

## Findings

- Rust starts every program with SIGPIPE ignored, so a write to a closed pipe returns EPIPE, and std's `println!` panics on any write error.
- The usual fix, restoring SIGPIPE's default action, does not fit here:
  - `devid daemon run` is the daemon in the same binary and process. A client that disconnects from its socket must not end the daemon.
  - The TUI writes to `pbcopy`'s stdin.
  - A process killed by SIGPIPE exits 141. Under `set -o pipefail`, `devid cmd front | grep -q x` would still fail at random, only without the message.
- The daemon crate has no `println!`. Only the CLI writes to stdout.

## Fix

- `crates/cli/src/main.rs` defines its own `println!` and `print!` before its modules, so they replace std's in the whole crate. They write through `out()`. A broken pipe ends the process with exit 0, because the reader stopped reading and that is not a failure of `devid`. Any other write error (a full disk) prints `devid: cannot write to stdout: …` and exits 1 instead of panicking.
- `devid logs <name> -f` flushes through `flush_stdout()`, which handles errors the same way. Before, a broken pipe there ended with `devid: Broken pipe (os error 32)` and exit 1.
- SIGPIPE stays ignored, so the daemon, the TUI and every socket and pipe still get an error, not a signal.

## Verified

- New test `crates/cli/tests/output.rs` (`closed_stdout_ends_quietly`): against a test daemon, `devid status`, `status --json`, `sites` and `php list` run with stdout on a pipe whose read end is already closed. Each must exit 0 with nothing on stderr. With the old `main.rs` it fails (`devid ["status"]` exited 101 with the panic). With the fix it passes.
- A throwaway daemon on the M1 (its own `DEVID_HOME` and `HOME`, ports 29088/29443/29535). `set -o pipefail; devid status | true`, 30 runs: the installed 0.16.1 failed and panicked 30 times, the fixed build 0 times. `devid status | head -1` and `devid sites | head -1` print their first line with nothing on stderr.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

The `devid … | grep -q` checks in `scripts/smoke.sh` (the O3 one and the Mail ones) no longer depend on timing. O4's `> /dev/null` workaround can stay or go.
