# M5: DNS responder and system helper

> Date: 2026-11-02 · Branch: `phase2/m5-system` (stacked on M4) · Plan: [Phase 2](../../roadmap/phase-2.md)

## Outcome

- **DNS:** the daemon answers `*.test` on `127.0.0.1:53535`.
  - A → 127.0.0.1, AAAA → no data (IPv4 only, where nginx listens), other names → REFUSED.
  - It replaces Valet's dnsmasq once `/etc/resolver/test` points here.
  - The bind result shows in `devid status`.
- **Helper** (`devid helper install | uninstall | status`, `--dry-run`) is the only root code.
  - The CLI prints every system change, then runs `sudo devid helper apply …` with the values passed explicitly; the helper validates them as root.
  - It installs `/etc/resolver/test` (backing up Valet's file), a pf anchor `com.apple/devid` redirecting 127.0.0.1:80/443 to 8088/8443 with a LaunchDaemon that reloads it at boot, and SSL-only trust for the name-constrained Devid CA in the System keychain.
  - `uninstall` reverses every file and command and restores the backup.
- **Port decision (owner):** pf redirect. Binding 80/443 on 0.0.0.0 would work without root, but it was rejected because it exposes the sites to the LAN; binding 127.0.0.1:80 needs root on macOS 14.
- Once the redirect is installed, site URLs drop the port.

## Verification

- `cargo test`: 17 tests pass.
  - DNS unit tests: A for any depth and case, AAAA empty, label-boundary suffix match, FORMERR for truncated packets and compression pointers, no reply to responses.
  - Helper unit tests: validation (bad TLD, privileged target ports, a wrong CA path, a symlinked CA), generated files, every written file removed by uninstall, foreign-file backup and restore including idempotency.
  - Integration test `dns_and_helper_without_root`: real UDP queries to the daemon, the install and uninstall dry runs, and `helper apply` refusing to run without root.
- `devid helper install --dry-run` on this Mac lists the exact changes. `devid helper status` shows nothing installed yet.

## Not yet

The system switch-over from Valet (`devid park ~/Projects`, then `devid helper install` run by the owner) is a separate, owner-approved step.
