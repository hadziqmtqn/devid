# devid is GitHub's Latest release; PHP/nginx builds from `runtime-latest` (D-038, v0.10.1)

> Date: 2026-11-08 · Repositories: `devid` (branch `feat/github-latest`), `devid-binaries` (branch `feat/runtime-latest`)

## What changed

- **Client (`daemon::releases`):**
  - The newest devid is read from `releases/latest/download/devid-version.txt`, GitHub's Latest release. When the download link fails, the API fallback asks `/releases/latest`.
  - `manifest.json` (PHP, nginx, Caddy, extensions) is read from `releases/download/runtime-latest/`.
  - `newer` counts a release as newer than its own pre-release (`0.11.0` > `0.11.0-rc.1`) and does not offer a pre-release an older release.
- **`release.yml` (devid):**
  - A stable tag creates `devid-v<version>` with `--latest`, plus a copy of `runtime-latest`'s `manifest.json` for devid 0.10.0 and older, and still updates `devid-latest`.
  - A tag with a suffix (`v0.11.0-rc.1`) creates a pre-release only.
- **`devid-binaries`:**
  - The PHP/nginx release is created with `--latest=false` and copied to `runtime-latest`.
  - Its manifest is also uploaded to the current Latest devid release.
  - `check-upstream.yml` compares with `runtime-latest`.
- **Unchanged:** `install.sh` and its public link (`devid-latest/install.sh`); `devid setup`'s app download from `devid-v<version>`.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`: pass. New unit test `latest_is_githubs_latest_release`; `versions_compare_numerically` covers pre-releases. The self-update integration test publishes to `latest/` and install.sh's test to `devid-latest/`.
- The publish step of `release.yml`, run locally with a fake `gh` that logs its calls:
  - `VERSION=0.10.1`: downloads `runtime-latest`'s manifest, creates `devid-v0.10.1` with `--latest` and the manifest, then updates `devid-latest`.
  - `VERSION=0.11.0-rc.1`: creates `devid-v0.11.0-rc.1` with `--prerelease --latest=false` and nothing else.

## Switch-over (once)

1. Merge `devid-binaries`' `feat/runtime-latest`.
2. Create `runtime-latest` from `2026.11.05-7`'s `manifest.json`, with `--latest=false`.
3. Merge this branch, then tag `v0.10.1`. Its release becomes Latest and carries the manifest, so devid 0.10.0 and older keep updating PHP and nginx.
