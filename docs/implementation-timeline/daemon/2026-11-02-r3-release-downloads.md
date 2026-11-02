# Phase 5 R3: install and update from releases

> Date: 2026-11-02 · Branch: `phase5/r3-downloads` · Plan: [Phase 5](../../roadmap/phase-5.md)

## Outcome

- `devid php install <ver>` and `devid server install <nginx|caddy>` no longer need `--from`. They read the release `manifest.json` from `hadziqmtqn/devid-binaries` (D-016), download the artifact, **verify its SHA-256 before unpacking**, install it and clean up the download.
- `devid update [--check]` lists or installs newer PHP patch versions, nginx and Caddy from the manifest.
- Safety rules:
  - only URLs under the release repository's downloads are accepted;
  - downloads are HTTPS only;
  - a checksum mismatch aborts and leaves the installed version untouched;
  - manifests with an unknown schema are refused.
- `--from <dir>` still installs local builds, and Apache still uses the system httpd.

## Verification

- `cargo test`: 28 tests pass. The new `install_and_update_from_a_release` test uses a local `file://` release with the devid-binaries layout and schema. It covers:
  - install without `--from` and `update --check` when up to date;
  - a new release: `--check` changes nothing, then `update` installs PHP 8.5.13 and nginx 1.30.6;
  - a tampered asset refused with "checksum mismatch" while 8.5.13 stays installed;
  - an asset outside the manifest's folder refused before download;
  - cleanup of the download folder.
- Unit tests cover version comparison and artifact lookup.
- The servers test now points its daemon at a missing local manifest, so no test touches the network.
- **Real release** (`2026.11.02-2`, temp home):
  - `php install 8.4` downloaded, verified and installed 8.4.27 in 9.9 s;
  - PHP 8.5 and nginx installed the same way;
  - erdbpro-saas `/login` returns 200 over HTTP/2 on 8.4.27;
  - `update --check` reports "up to date";
  - Caddy reports "the release has no caddy" until R2 ships a new release.
