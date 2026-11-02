# Phase 5 R4: devid releases, install.sh and self-update

> Date: 2026-11-02 · Branch: `phase5/r4-release` · Plan: [Phase 5](../../roadmap/phase-5.md)

## Outcome

- **Release workflow (`.github/workflows/release.yml`):** on a `v<version>` tag that matches `Cargo.toml`, it tests and builds on `macos-15-intel`, ad-hoc signs the binary, and publishes `devid-darwin-x86_64`, `install.sh`, `devid-version.txt` and `checksums.txt` to `devid-binaries`.
  - The releases are `devid-v<version>` (immutable) and `devid-latest` (rolling), both with `--latest=false`.
  - Uploads use the fine-grained `DEVID_BINARIES_TOKEN` (Contents: write on `devid-binaries` only).
- **`install.sh`:**
  - downloads from the public `devid-latest` (or `devid-v<x>`) release with plain `curl`;
  - verifies SHA-256, installs to `~/.devid/bin/devid`, and hands over to `devid setup`;
  - no longer needs `gh`.
- **`devid self-update [--check]`:** reads `devid-version.txt`, downloads and verifies the binary, and runs the new binary's `setup`, which copies it into place and reloads the LaunchAgent. A binary with a checksum mismatch never runs.
- `releases::fetch` and `sha256_hex` are shared with R3. `DEVID_RELEASE_BASE` points tests at a `file://` release.

## Verification

- `cargo test`: 30 tests pass.
  - `self_update_checks_verifies_and_hands_over`: up to date, available, `--check` installs nothing, update runs the new binary's setup, cleanup, tampered binary refused.
  - `install_sh_downloads_verifies_and_installs` (macOS only, since install.sh checks `uname`): installs and reports the version, refuses a checksum mismatch, reports a missing version tag.
- The release workflow itself runs only on a tag; the first real run happens when `v0.1.0` is pushed.
