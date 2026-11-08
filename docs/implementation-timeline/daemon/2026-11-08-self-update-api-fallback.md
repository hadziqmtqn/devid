# Fix: self-update falls back to the GitHub API (v0.9.1)

> Date: 2026-11-08 · Branch: `fix/self-update-api-fallback`

## Problem

After v0.9.0 was published, `devid self-update` on the owner's Mac failed with `download failed: https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/devid-version.txt: curl: (56) The requested URL returned error: 504`.
- The 504 came from github.com itself (`x-github-edge-region: southeastasia`) in about 0.1 s, before any redirect, for every release asset, including v0.8.0's.
- It was not a rate limit: no `x-ratelimit-*` or `retry-after` header, and no 403 or 429. githubstatus.com reported all systems operational.
- The GitHub API (`api.github.com`, what `gh release download` uses) answered normally the whole time.
- The owner was updated by hand: the assets were fetched with `gh` into a local folder and `devid self-update` ran against it through `DEVID_RELEASE_BASE=file://…`.

## Fix

- `releases::fetch_devid` (used by `self-update`, the daemon's and TUI's update check, and the Devid.app install in `setup`) tries the download link first. When that fails for an official release, it reads the release from `api.github.com/repos/hadziqmtqn/devid-binaries/releases/tags/<release>` and downloads the asset through its API `url` with `Accept: application/octet-stream`.
- Only an asset URL inside the official repository's API is followed, HTTPS only (redirects included). The binary is still checked against `checksums.txt`.
- A test release (`DEVID_RELEASE_BASE`) never falls back. When both routes fail, the error names both.
- Unauthenticated API use is limited to 60 requests an hour; one update check through the fallback takes two.
- Version 0.9.1.

## Verification

- `cargo test`: all pass, including `the_api_fallback_only_follows_official_asset_links`. The `sites` integration tests failed once while the Mac's load average was about 50 and passed on the rerun.
- `cargo test -p devid-daemon --lib -- --ignored releases::tests::the_api_fallback` (live): the binary downloaded through the API matches its line in `checksums.txt`.
- With github.com sent to a dead proxy (`https_proxy=http://127.0.0.1:9`, `no_proxy=api.github.com,githubusercontent.com`), `devid self-update --check` read the version through the API. With both routes blocked it failed with both errors.
- Not covered: `install.sh` still downloads with plain curl from github.com.
