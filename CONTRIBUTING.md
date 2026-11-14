# Contributing to Devid

Thanks for helping. Devid is a personal project: one maintainer, who uses it daily on an Intel Mac. Bug reports, small fixes and docs fixes are the easiest to take. For anything bigger, open an issue first, so we can agree on the approach before you write the code.

## Where things go

| You have | Where |
|---|---|
| A bug, or an idea for Devid | [Issues](https://github.com/hadziqmtqn/devid/issues/new/choose) in this repository |
| A PHP extension you want in the catalog | A feature request here, with its name, its PECL or source link and the PHP versions you need |
| A security problem | Privately, as [SECURITY.md](SECURITY.md) explains; never in a public issue |
| A change to the PHP, nginx or Caddy builds, or to the extension catalog's build scripts | [`devid-binaries`](https://github.com/hadziqmtqn/devid-binaries) |
| A change to the website or the user guide | [`devid-site`](https://github.com/hadziqmtqn/devid-site) |

## What Devid is, and is not

These limits are decisions (see [`docs/structure-of-app/DECISIONS.md`](docs/structure-of-app/DECISIONS.md)), so a change that breaks one needs a discussion first:

- **No Homebrew and no VM at runtime.** Devid downloads its own static builds.
- **PHP 8.3, 8.4 and 8.5**, each with php-fpm in `pm = ondemand`.
- **One web server for every site:** nginx by default, Caddy or Apache instead.
- **No databases.** Use DBngin, Docker or anything else next to Devid.
- **Idle cost matters.** Devid is built on a 2-core Intel Mac with 16 GB of RAM. Background work, polling and memory at idle need a reason.
- **One platform at a time** (D-044): Intel macOS today, then Apple silicon, then Debian-based Linux, then Windows. Code for one operating system stays in as few places as possible (D-019), such as `crates/daemon/src/helper.rs` for the system setup.

## The code

A Rust workspace:

- `crates/daemon`: the daemon library: config, sites, PHP and web server processes, certificates, updates, and the socket API;
- `crates/cli`: the `devid` binary: the CLI, the TUI, `devid setup` and the daemon process;
- `crates/gui`: the desktop app, Tauri v2. The UI is plain HTML, CSS and JavaScript in `crates/gui/ui`, with no build step. It is not a default member, so build it with `-p devid-gui`;
- `scripts/bundle-app.sh`: makes `Devid.app`;
- `spike/`: throwaway scripts from early experiments, kept for reference;
- `docs/`: plans, architecture and decisions. Start at [`docs/README.md`](docs/README.md).

## Setting up

You need Rust stable with `rustfmt` and `clippy` (`rustup component add rustfmt clippy`).

- **macOS:** the Xcode Command Line Tools (`xcode-select --install`). That is enough for everything, the desktop app included.
- **Linux:** you can build, lint and test everything, but Devid only runs on macOS. The desktop app needs `libwebkit2gtk-4.1-dev`, as in CI.

```sh
cargo build                  # daemon and CLI
cargo build -p devid-gui     # the desktop app
```

## Running a development copy

A development copy can run next to an installed Devid without touching it. Point `DEVID_HOME` at a separate folder, and give it other ports.

```sh
export DEVID_HOME=~/.devid-dev   # keep it short: macOS limits socket paths to 104 bytes
mkdir -p "$DEVID_HOME"
cat > "$DEVID_HOME/config.toml" <<'TOML'
http_port = 28088
https_port = 28443
dns_port = 28535
update_check_minutes = 0
TOML

cargo run -- daemon run           # in one terminal
cargo run -- php install 8.5      # in another, with the same DEVID_HOME; 8.5 is the default
cargo run -- server install nginx
cargo run -- link --path ~/code/my-app
cargo run -p devid-gui            # the desktop app, on the same daemon
```

The PHP and nginx builds come from Devid's releases, as for a normal install.

Without the system setup, `.test` names do not resolve and the certificate is not trusted. Reach a site with `curl -k --resolve my-app.test:28443:127.0.0.1 https://my-app.test:28443/`.

Do not run `devid setup`, `devid helper` or `devid uninstall` from a development build unless that is what you are working on. They ask for your password and change the system: `/etc/resolver/test`, the pf rules and the trusted certificate authority. The same goes for `devid self-update`, which replaces the installed `devid`.

## Before you open a pull request

Run the same checks as CI:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

If you changed the desktop app, also run:

```sh
cargo clippy -p devid-gui --all-targets -- -D warnings
cargo test -p devid-gui
```

The tests use fake `php`, `php-fpm` and `nginx` scripts, so they need no PHP build and run on Linux too. A new behaviour comes with a test, and so does a bug fix when a test can catch it.

## Pull requests

- Branch from `main`, and keep one change per pull request.
- Say what changes for someone using Devid, and why. Link the issue (`Fixes #123`).
- Say how you tested it, on which macOS version and which Mac. Add screenshots when a screen changes.
- Update the docs in the same pull request:
  - `docs/` is the source of truth for plans and behaviour;
  - a new or reversed decision is appended to `DECISIONS.md`; entries are never deleted, only marked superseded;
  - a change users can see also needs the user guide in `devid-site`. Say so in the pull request, and the maintainer will make that change when the release ships.
- CI runs the checks above on every pull request. The automatic Claude review does not run on pull requests from forks, which get no secrets; the maintainer reviews those.
- The maintainer merges with a merge commit and cuts releases. Do not change the version in `Cargo.toml`.

## Style

- `rustfmt.toml` sets the format, and Clippy warnings fail CI.
- Code, comments, docs and commit messages are in English.
- A comment says why, not what. Keep new dependencies rare; each one needs a reason.
- Commit messages: a short summary line, then what changed and why.
- UI text is short and plain, in sentence case.

## License

Devid is under the [GNU AGPL v3](LICENSE) (`AGPL-3.0-only`). By opening a pull request, you agree that your contribution is licensed under the same terms, and that you have the right to submit it. There is no separate contributor agreement.

Be kind and constructive in issues and reviews.
