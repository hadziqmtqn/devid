# Phase 5 Plan: build pipeline, releases and updates

> Scope: building and shipping Devid's binaries, and how an installed Devid gets updates
> Verified against: 2026-11-02
> Status: proposed (milestones); D-016 accepted

Today Devid installs PHP and nginx with `--from <spike folder>`, and an upgrade means `git pull && cargo build && ./target/release/devid setup`. Phase 5 replaces both with releases that the installed Devid downloads, verifies and updates by itself. `install.sh` (D-014) starts working at the end of this phase.

## Two repositories (D-016)

| | `hadziqmtqn/devid` (private) | `hadziqmtqn/devid-binaries` (public) |
|---|---|---|
| Contents | Devid's Rust code | build scripts (PHP, nginx), CI, releases |
| CI cost | small: the `devid` binary only (about 3 min on macOS) | free: public repositories have no minute quota |
| Publishes | the `devid` binary into `devid-binaries` releases | PHP, nginx, Caddy (mirrored) and `manifest.json` |

The only link between them is a URL. The `devid` code knows where `manifest.json` lives, and `manifest.json` lists every artifact with its version, URL and SHA-256. Updating PHP needs no change to the `devid` repo.

```text
devid (private) ── CI builds devid ──(fine-grained token)──▶ devid-binaries release
devid-binaries (public) ── CI builds PHP / nginx ─────────▶ devid-binaries release + manifest.json
installed Devid ── reads manifest.json ── downloads + verifies sha256 ── installs / updates
```

## Milestones

| # | Milestone | Where | Done when |
|---|---|---|---|
| R1 | Create `devid-binaries` and move the PHP and nginx build scripts there. Add a CI workflow on `macos-15-intel` that builds PHP 8.3/8.4/8.5 and nginx, packages them as `.tar.gz`, and publishes a release with `checksums.txt` and `manifest.json`. | public | a release built by CI with every artifact listed in the manifest |
| R2 | Mirror Caddy: CI downloads the official release, verifies Caddy's checksum, and republishes it with an entry in the manifest. | public | Caddy in the manifest |
| R3 | `devid php install <ver>` and `devid server install <nginx\|caddy>` download from the manifest when `--from` is not given. `devid update` checks the manifest and installs newer PHP or nginx patch versions. Every download is SHA-256-checked; a mismatch aborts. | private | the owner's PHP and nginx come from a release, no spike folder |
| R4 | A workflow in `devid` builds the `devid` binary on a version tag and uploads `devid-darwin-x86_64` plus `checksums.txt` to `devid-binaries` (D-014 contract). `install.sh` points at `devid-binaries`. `devid self-update` downloads, verifies and runs `setup` from the new binary. | both | `curl … install.sh \| sh` works on this Mac; `devid self-update` upgrades |
| R5 | A weekly scheduled workflow compares php.net, nginx and Caddy release feeds with the manifest and builds whatever is new. | public | a new upstream patch appears in the manifest without a manual step |

## Decisions and constraints

- **Hosting (D-016):** binaries are public in `devid-binaries`. They contain only open-source software (PHP, nginx, Caddy) and the compiled `devid` binary; the Devid source stays private.
- **Signing:** ad-hoc (D-010); notarization only if Devid ever goes public.
- **Architecture:** x86_64 only for now (D-010); the workflows keep a matrix slot for arm64.
- **Integrity:** the client trusts only `https://github.com/hadziqmtqn/devid-binaries/releases/…` URLs and refuses an artifact whose SHA-256 differs from the manifest. The manifest is fetched over HTTPS from the same repository. `shortcut:` there is no signature on the manifest yet; signing it (for example with minisign) can follow if the threat model needs it.
- **Owner actions needed:**
  - creating the public repository (R1, after approval);
  - creating one fine-grained token, limited to "Contents: write" on `devid-binaries`, and storing it as a secret in `devid` (R4).
- **Open, verify in R1:** whether `macos-15-intel` runs as a free standard runner for public repositories.
