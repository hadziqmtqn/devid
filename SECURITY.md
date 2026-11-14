# Security policy

## Reporting a vulnerability

Report it privately through [Report a vulnerability](https://github.com/hadziqmtqn/devid/security/advisories/new) (the repository's Security tab). Do not open a public issue, pull request or discussion for it.

Please include:

- the Devid version (`devid --version`), and your macOS version and Mac (Intel or Apple silicon);
- what an attacker can do, and what they need first (a local user, a website open in your browser, a file in a project folder);
- steps to reproduce it, or a proof of concept.

Devid is a personal project, so replies are best effort. You will get an answer in the advisory, and credit in it when the fix ships, if you want it.

## Supported versions

Only the latest release gets fixes. `devid self-update` installs it, and the desktop app does so on its own.

## What counts

Devid changes the system once, during `devid setup` (with your password), and then runs as your user. Reports about these parts matter most:

- the system setup: the `.test` resolver, the pf rules that forward ports 80 and 443, and Devid's certificate authority, which is trusted in the System keychain;
- the private key of that certificate authority, and the site certificates it signs;
- the daemon's socket, and anything that lets another local user or a website drive it;
- downloads and updates: `install.sh`, `devid self-update`, `devid update` and their checksum checks;
- Share, which opens a local site to the internet through a tunnel.

The PHP, nginx and Caddy builds come from [`devid-binaries`](https://github.com/hadziqmtqn/devid-binaries). Report a flaw in Devid's build or packaging of them here. Report a flaw in PHP, nginx or Caddy themselves to those projects.
