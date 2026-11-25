# Phase 14 D5: CI, docs and 0.21.0

> Date: 2026-11-25 · Branch: `db-d5` · Plan: `docs/roadmap/phase-14.md`, "D5 plan" · Decision: D-061

The owner started D5 after D4 merged ("Mulai D5"). 0.20.1 already carries D2 and D3, off by default. 0.21.0 adds the app's page and makes databases part of the docs. This record grows with each step.

## Step 1: databases checked on Arch, and 0.21.0-rc.1

- **`scripts/arch-vm.sh db-e2e <devid>`:**
  - copies the devid build and `scripts/db-e2e.sh` into the Arch VM and runs the script there as the user `arch`;
  - the script needs no sudo and uses a throwaway `DEVID_HOME` under `/tmp`, so it does not matter what the e2e step left behind;
  - python3, which the script uses for JSON and ports, comes with cloud-init. The step installs it with pacman if it is missing.
- **`.github/workflows/ci.yml`:**
  - `arch-e2e` runs that step after setup and uninstall, with the musl build;
  - every pull request now checks MySQL, PostgreSQL and MongoDB on macOS 15, Ubuntu 24.04 and Arch.
- **The version:**
  - `0.21.0-rc.1` in `Cargo.toml` and `Cargo.lock`;
  - the release notes in `release.yml` add that Devid runs MySQL, PostgreSQL and MongoDB on request, and that `--purge` keeps their data unless `--delete-databases` is given.

### Verification

- **Locally:**
  - `bash -n scripts/arch-vm.sh`, and the workflow's YAML parses;
  - `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass, `-p devid-gui` included;
  - `cargo test` passes, 169 tests in 19 binaries. It ran under a fake `HOME`, which skips only `passwd_home_matches_home` because that test needs the real home.
- **On the pull request:** the new Arch step is the check that matters, since only CI can boot the VM.

## Next

- The owner tags `v0.21.0-rc.1` once this merges.
- The rc goes on the owner's M1 (with the owner's OK) and on the Ubuntu, Arch and CachyOS VMs.
- Then the docs, the guide page and 0.21.0.
