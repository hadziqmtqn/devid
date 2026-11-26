# Phase 14 D5: CI, docs and 0.21.0

> Date: 2026-11-25 to 2026-11-26 · Branch: `db-d5` · Plan: `docs/roadmap/phase-14.md`, "D5 plan" · Decision: D-061

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

## Step 2: the rc on the M1 and the Linux VMs (2026-11-25 and 2026-11-26)

- **The release:** #153 merged and the owner made the pre-release `v0.21.0-rc.1`; release run 45646037964 passed. `devid-latest` stayed on 0.20.1.
- **The M1:** with the owner's OK, `install.sh` put the rc there over 0.20.1, app included. The app's `--page` did not know `databases`; commit b4a2808 in the release pull request fixes it, with a test. The owner then tried the page in the app: PostgreSQL on 5433 next to DBngin's 5432, an account added and its password reset, and Open in Terminal in Terminal and in cmux. "Semua berjalan normal".
- **The VMs** (Ubuntu 24.04, Arch with KDE Plasma, CachyOS with fish; old Mac, over SSH): `install.sh` replaced the 0.20.0 each one still had. The same 32 checks passed on each: the three engines through the systemd user unit and the shims, an account per engine, a unit restart with the data kept. Then `uninstall --purge` kept only `~/.devid/data`, a reinstall used it again with the old passwords, and `--purge --delete-databases` left no `~/.devid`. The numbers and two side notes are in `phase-14.md`, "D5: the rc on the Linux VMs". The VMs are left without Devid.

## Step 3: 0.21.0

- `0.21.0` in `Cargo.toml` and `Cargo.lock`; the README's status says v0.21.
- D5's results in `phase-14.md` (its status, the M1 and the VM sections, the D5 row), D-061's row and a D5 bullet, the docs index and Phase 14's row in PROJECT_CONTEXT, dated 2026-11-26.
- The release notes in `release.yml` already name the databases since the rc.

## Step 4: released

- The owner merged #154 (main c785148) and tagged `v0.21.0`. Release run 45755040242 passed every job on its first run, `arch-e2e` included.
- `devid-v0.21.0` is Latest in devid-binaries, with 9 files, and `devid-latest` carries it. The arm64 binary's sha256 matched `checksums.txt`, and it answers `devid 0.21.0`.

## Follow-ups

- **To merge now:** hadziqmtqn/devid-site#23, the guide's `/docs/databases` page and the roadmap's 0.21 entry.
- **The owner's M1:** still on 0.21.0-rc.1; `devid self-update` takes it to 0.21.0.
- **Small, not changed:** right after the daemon starts, `devid doctor` reports a database that is still starting as a problem, for up to about 20 s.
- **Next:** Phase 15, `devid new` (D-063), from N1.
