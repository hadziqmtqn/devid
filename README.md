# Devid

Devid is a local PHP development environment for macOS. Every folder you link or park is served at `https://<name>.test` with a trusted certificate. Devid runs PHP 8.3, 8.4 and 8.5 side by side, and nginx, Caddy or Apache as the web server. It needs no Homebrew, no Docker and no virtual machine. You use it from the command line, from a terminal UI (TUI) or from the menu-bar desktop app.

> **Status:** v0.12, used daily on one Mac. It runs on macOS on Intel only. Apple Silicon builds come later (D-010). This repository holds the source, under the [GNU AGPL v3](LICENSE) (D-043); the binaries and the user guide are published in the [`devid-binaries`](https://github.com/hadziqmtqn/devid-binaries) repository.

## Contents

- [Using Devid](#using-devid)
- [Roadmap](#roadmap)
- [Developing Devid](#developing-devid)
- [License](#license)

## Using Devid

The user guide is public: [`GUIDE.md` in devid-binaries](https://github.com/hadziqmtqn/devid-binaries/blob/main/GUIDE.md) (D-033). It covers install options, sites, PHP and extensions, web servers, the desktop app, the TUI, HTTPS, updates, troubleshooting, uninstall and the files in `~/.devid`.

```sh
# install with the desktop app
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh -s -- --gui
# or the CLI and TUI only
curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
```

A change users can see updates the guide in a `devid-binaries` pull request, merged when the release ships.

## Roadmap

The direction after v1 is D-019. Each item lists the decision it comes from. The decisions are in [`docs/structure-of-app/DECISIONS.md`](docs/structure-of-app/DECISIONS.md). Nothing below v1 has a date yet.

### v1: done (v0.2)

- Sites at `*.test`: park, link, and per-site PHP versions (D-011).
- PHP 8.3, 8.4 and 8.5 as static builds, with Xdebug and pcov switches (D-008, D-009).
- One web server: nginx (default), Caddy or Apache (D-015).
- Automatic HTTPS through a local CA.
- The CLI and the TUI (D-012).
- Install, self-update and binary updates from public releases (D-014, D-016).
- `devid setup` also installs PHP and a web server, chosen with `--php` and `--server` (v0.2.3).

### Next: small v1.x improvements

- A shorter install command on its own domain (deferred, `docs/roadmap/phase-5.md`).

### Done: more PHP extensions (Phase 6)

- **Done in v0.3.0:** `devid php ext install <version> <name>` adds an extension from Devid's catalog: apcu, mongodb, ldap and imagick (D-021, `docs/roadmap/phase-6.md`). See [PHP](https://github.com/hadziqmtqn/devid-binaries/blob/main/GUIDE.md#php).
- **Done in v0.4.0:** the TUI's PHP page installs several catalog extensions at once and removes them one at a time; the CLI takes several names too.
- **Done 2026-11-05:** igbinary, msgpack, memcached and swoole joined the catalog. A recipe pull request now builds and tests a new extension, and its merge publishes it. See [PHP extensions](https://github.com/hadziqmtqn/devid-binaries/blob/main/GUIDE.md#php-extensions).

### Done: desktop app (Phase 7)

- A menu-bar app with a window, based on the approved prototype.
- Built with Tauri v2 and plain HTML ported from the prototype, with no Node. It is another client of the daemon's API, so the daemon needs no rework.
- **Done 2026-11-05:** the first step (G1) measured it on an Intel Mac. The menu-bar icon alone uses 21 MB and no CPU, and closing the window brings it back to that (D-027, `docs/roadmap/phase-7.md`).
- **Done 2026-11-06:** the window pages (G2), the menu-bar menu with login settings and update checks (G3), and Devid.app with `devid setup --gui` (G4). The last step (G5) put the one-line install with `--gui` in the docs; Devid v0.7.0 ships the app. See [The desktop app](https://github.com/hadziqmtqn/devid-binaries/blob/main/GUIDE.md#the-desktop-app).

### v2 and later: planned, not scheduled

- **More platforms:**
  - **Apple Silicon (arm64) macOS:** the build pipeline can already produce these (D-010).
  - **Linux:** a second platform implementation for CA trust, `.test` DNS and ports 80/443 (D-013, D-018).
  - **Windows:** a goal, and the largest effort, since it needs its own PHP builds and has no php-fpm (D-018, D-019).
- **Public release (D-010):**
  - a license: done, the GNU AGPL v3 with the source public (D-043);
  - signed and notarized macOS builds;
  - a clear notice about the system changes Devid makes.
- **Developer tools (D-011):** a mail catcher, a dump viewer, a full log viewer, and sharing a site through a tunnel.
- **Preview mode:** run more than one web server at the same time on different ports, to compare nginx, Caddy and Apache side by side.
- **TUI extras (Phase 4):** mouse support, themes, and changing the domain and ports from the TUI.

### Out of scope

- Databases. Use a separate tool such as DBngin (D-006).
- Docker or VM-based setups.

## Developing Devid

The source is a Rust workspace:

- `crates/daemon`: the daemon library;
- `crates/cli`: the `devid` binary and the TUI;
- `crates/gui`: the desktop app (Tauri v2). It is not a default member, so build it with `-p devid-gui`; `scripts/bundle-app.sh` makes `Devid.app`.

```sh
cargo build
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on every pull request. To publish a release, push a `v*` tag. The release workflow then publishes `devid` to `devid-binaries`.

The plans and decisions are in [`docs/`](docs/README.md). Start with the project context, then read the architecture and the decisions.

## License

Devid is free software under the [GNU Affero General Public License v3.0](LICENSE) (`AGPL-3.0-only`). It comes without any warranty. If you distribute Devid, changed or not, you must offer its source under the same license, and a changed version offered to users over a network must offer its source too.
