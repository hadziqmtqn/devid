# Phase 12 Plan: Linux

> Scope: Devid on Ubuntu LTS (the daemon, the CLI and the TUI), with `.test` DNS, HTTPS and ports 80/443 as on macOS (D-057)
> Verified against: 2026-11-23 (v0.18.0; GitHub's `ubuntu-latest` is Ubuntu 26.04; spike run 45104513332; runtime release `2026.11.23-19`)
> Status: accepted 2026-11-23 (D-057), with the owner's answers in "Owner's answers". L1 and L2 done 2026-11-23, see "L1 results" and "L2 results"; L3 is next.

D-044 puts Debian-based Linux after Apple silicon, which shipped in 0.14.0 (D-046). On 2026-11-23 the owner chose Linux, not Windows, as the next platform. The research backs that order: Linux is Herd's most-voted request, and neither Herd nor ServBay runs on it (`docs/research/competitors-2026-11.md`).

## Where things stand (2026-11-23)

- **Portable already:**
  - The daemon's core: the supervisor, the DNS server, the CA and certificates (rcgen with ring, no OpenSSL), nginx and Caddy configs, sites, commands, Mail and on-demand.
  - The CLI and the TUI.
  - CI's `rust` and `gui` jobs run clippy and every test on Ubuntu. On 2026-11-02 they caught a race that macOS hid (M6).
- **Never run end to end on Linux:** the release smoke test, `scripts/smoke.sh`, runs on macOS only.
- **macOS-only today:**

  | Part | macOS (where) | Linux |
  |---|---|---|
  | `.test` DNS | `/etc/resolver/test` (`helper.rs`) | systemd-resolved |
  | Ports 80/443 | a pf anchor and a LaunchDaemon that loads it at boot (`helper.rs`) | an nftables redirect, loaded at boot by a system unit |
  | CA trust | the System keychain (`helper.rs`) | the system store, plus NSS for browsers |
  | Start at login | LaunchAgents (`install.rs`) | a systemd user unit |
  | PATH line | `~/.zshenv` and `~/.zshrc`, or `~/.bash_profile` for bash (`install.rs`) | `~/.bashrc` for bash: Ubuntu's terminal starts non-login shells, and a new `~/.bash_profile` would stop bash from reading `~/.profile` |
  | On-demand's open-connection check | libproc (`ondemand.rs`) | `/proc/net/tcp` and `tcp6`. On Linux the check now always says "none", so an idle timer could stop a dev server while a tab still holds its HMR socket |
  | Runtime: PHP, extensions, nginx | built on macOS runners with `xcrun`, `otool` and `install_name_tool` (devid-binaries) | Linux build scripts. Caddy and Mailpit stay mirrors of upstream builds |
  | Runtime manifest | `manifest.py` accepts only `-darwin-` file names, with `x86_64`/`arm64` keys | Linux entries, without changing the keys that installed macOS copies read |
  | Apache | the macOS system `httpd` (`apache.rs`) | none to borrow |
  | Install and self-update | `install.sh`, `devid-darwin-*` and `Devid-darwin-*.zip` (`setup.rs`, `releases.rs`) | `devid-linux-x86_64`, with no app bundle |
  | The app | WKWebView, NSUserNotification, an `osascript` folder picker | not in this phase |

## Ground rules

- **Where it is tested (D-013):**
  - **GitHub's Ubuntu runners**, for CI and most of the spike. They allow `sudo` without a password, so the helper's root steps run there for real.
  - **The Xubuntu machine:** the owner's old Intel Mac running Xubuntu, set up by the owner and reached over SSH on the local network.
    - Claude runs the checks a runner cannot: real browsers in the Xfce session, and reboots.
    - It is a separate machine, so D-013's rule against a VM on the owner's laptop still holds.
- **The owner's M1 stays as it is:** no step of this phase runs `sudo`, installs anything or changes a setting there. It has 8 GB of RAM (D-052).
- **`sudo` on the Xubuntu machine:**
  - Claude asks before a step that needs it, as CLAUDE.md requires.
  - Claude never types a password: the test user has passwordless `sudo`, or the owner types it.
- **Same model as macOS:**
  - One root step at setup, which `devid uninstall` undoes; a file Devid would replace is backed up first.
  - Everything else runs as the user, in `~/.devid`.
  - The CA stays name-constrained to `.test`, so trusting it vouches for no real domain.
- **Resources:** measure idle cost on Linux the same way as on macOS: the daemon, nginx and one php-fpm master with 0 workers. Linux gets no resident process that macOS does not have.
- **One interface (D-013, D-019):** OS-specific code stays behind one interface with a macOS and a Linux implementation. The rest of the daemon does not branch on the OS, and the clients only show what the daemon says (D-012).

## Proposal (D-057)

- **Target:** Ubuntu 24.04 and 26.04 LTS, on x86_64. The desktop checks run on Xubuntu, which is Ubuntu with Xfce. Debian and arm64 come after Ubuntu ships (see "Not in this phase").
- **Interfaces:** the CLI and the TUI, as v1 was on macOS (D-012). The app waits.
- **Web servers:** nginx (the default) and Caddy. Apache waits: Linux has no system `httpd` to borrow, and Ubuntu's `apache2` package brings its own service on port 80.
- **The root step** (`sudo devid helper apply`; L1 confirms each part):
  - **DNS:** systemd-resolved sends `.test` lookups to Devid's DNS port and nothing else. L1 picks how.
  - **Ports:** an nftables table redirects `127.0.0.1` and `[::1]` ports 80/443 to the web server's high ports, the twin of the pf anchor. A small system unit loads it at boot.
  - **CA:** the Devid CA goes into `/usr/local/share/ca-certificates`, then `update-ca-certificates` runs. Browsers get it through NSS (see L1).
- **As the user:**
  - a systemd user unit for the daemon (`~/.config/systemd/user`);
  - the PATH line in `~/.bashrc`, or in the zsh files for zsh;
  - `~/.devid`, as on macOS.
- **PHP:** glibc builds from static-php-cli. Everything except glibc is linked in.
  - A fully static musl PHP cannot load `.so` files, and Xdebug, pcov and the extension catalog are `.so` files (D-008, D-021). static-php-cli's docs build shared extensions on Linux with its glibc targets.
  - They are built against a glibc old enough to run on Debian 12 (glibc 2.36) and later, so Debian can follow without new builds.
- **nginx and Devid itself:** static musl builds, since neither loads `.so` files. Devid uses ring, not OpenSSL. L1 checks that both build.
- **Not chosen:**
  - **`net.ipv4.ip_unprivileged_port_start=80`:** it would let every user's process take any low port, not just Devid's web server.
  - **`setcap` on the web server:** the capability is lost each time an update replaces the binary, and the binary sits in the user's home.
  - **A VM or container per site:** Devid has no VM dependency (D-001).

## L1: what the spike must answer

All on GitHub's Ubuntu runners (24.04 and 26.04), except where a question names the Xubuntu machine. L1 can start on the runners before that machine is ready.

1. **PHP:**
   - Does a static-php-cli glibc build of 8.4 (cli and fpm, with `--build-shared` for Xdebug and pcov) load both `.so` files in cli and in php-fpm?
   - Which glibc floor does it need, and how long does it take to build on a runner?
2. **Catalog extensions:** does one catalog extension (apcu, plus imagick for its library chain) load as a `.so` against that PHP?
3. **nginx:** does a static musl build with static-php-cli's OpenSSL and zlib run?
4. **DNS:**
   - Which systemd-resolved setup sends only `.test` to `127.0.0.1:53535` and leaves every other name alone? The choices are a global drop-in with `Domains=~test`, or a link of its own. Check with `resolvectl query` and `getent hosts`.
   - On Debian, systemd-resolved is a separate package, and installing it takes over `/etc/resolv.conf`. Record what Debian 13 needs, for later.
5. **Ports:**
   - Does an nftables `output`-hook redirect work for `127.0.0.1` and `[::1]`?
   - Does it live alongside Docker's and ufw's rules?
   - Does it survive a reboot (on the Xubuntu machine)?
6. **CA trust:**
   - After `update-ca-certificates`, do `curl` and PHP's own bundle reach `https://<site>.test`?
   - Do headless Chrome and Firefox reach it? Which stores do they read? (If the runner image lacks a browser, the job installs it.)
   - Do Chrome and Firefox in the Xfce session reach it, the way a user opens them (on the Xubuntu machine)?
   - On Ubuntu and Xubuntu, Firefox is a snap with its own profile path.
   - `certutil` (`libnss3-tools`) is not installed by default: find a way that does not need it, or have setup say so.
7. **Login:**
   - Does the daemon run under a systemd user unit?
   - What happens at logout? Without lingering, the user's units stop.
   - Does Devid come back after a reboot and login (on the Xubuntu machine)?
8. **On-demand:** does `/proc` find an established connection for a process group, without starting a process?
9. **Idle cost on Linux:** measure the daemon, nginx and one php-fpm master.
10. **Devid's own binary:** does a musl build of the CLI and daemon pass the test suite?

## L1 results (2026-11-23)

### Setup

- **Where:** `.github/workflows/spike-linux.yml`, which runs only on `spike/linux-*` branches and publishes nothing. The scripts are in `spike/linux/` (see its README).
- **Runners:** Ubuntu 24.04.5 (glibc 2.39) and 26.04.1 (glibc 2.43), kernel 7.0, Chrome 157 and Firefox 160. Docker runs there, and ufw is installed but off.
- **System checks:** Devid's own daemon (the musl build) ran under a systemd user unit, so the CA and DNS were Devid's. The site certificate was signed with openssl from the Devid CA's key, with the same fields as `ca.rs`. nginx and php-fpm came from the spike's builds.
- **Runs:** 45099637805 to 45104513332. The fifth passed every job: 49 of 49 system checks on each runner. Runs 1 to 4 failed on things the spike fixed, listed under "Gotchas".

### Answers

| # | Question | Answer |
|---|---|---|
| 1 | PHP with `.so` support | **Yes.** static-php-cli 2.8.5 builds PHP 8.4 (cli and fpm) for glibc with `SPC_TARGET=x86_64-linux-gnu.2.17`; `spc doctor --auto-fix` installs zig. Xdebug and pcov load in cli and php-fpm. php links only glibc (libc, libm, libdl, libpthread, librt, libutil, libresolv), and its newest symbol is `GLIBC_2.17`. 71 MB each for php and php-fpm; 10 to 19 minutes on a 4-core runner, plus 1.5 to download. The same build runs in Ubuntu 22.04, 24.04 and 26.04 and Debian 12 and 13 containers |
| 2 | A catalog extension | **Yes for apcu:** phpize against the build root, compiled by the same zig for the same target, loads and works. imagick, with its library chain, is left for L2 |
| 3 | nginx | **Yes.** nginx 1.30.7 with OpenSSL 3.6.5, fully static with musl ("statically linked", no dynamic loader). It serves HTTP and HTTPS and passes PHP to php-fpm over a unix socket |
| 4 | DNS | **A global drop-in works.** `/etc/systemd/resolved.conf.d/devid.conf` with `DNS=127.0.0.1:53535` and `Domains=~test`, then a restart of systemd-resolved: `resolvectl query`, `getent hosts` and the browsers resolve `.test`, other names still resolve, and tcpdump saw 0 queries outside `.test` reach Devid's port. A dummy link of its own (`resolvectl dns devid0 …`) did not work: resolved gave the address-less link no DNS scope. Devid's DNS already answers `REFUSED` to other names |
| 5 | Ports | **Yes.** An `inet devid` table with an `output` nat hook redirects `127.0.0.1:80/443` to 8088/8443. A oneshot system unit loads it (`nft -f`) and removes it on stop. It works next to Docker's tables and with ufw on, survives `systemctl restart`, and stopping the unit takes the redirect away. A reboot is left for the Xubuntu machine |
| 6 | CA trust | **System store:** `update-ca-certificates` makes curl trust `https://<site>.test`, and PHP trusts it through Devid's `cacert.pem` (D-008). PHP's compiled-in default is `/etc/ssl/cert.pem`, which Ubuntu does not have, so Devid must keep setting the CA file. **Firefox 160** (Mozilla's build in `/usr/lib/firefox`) trusted it from the system store alone, and `policies.json` (`Certificates.Install`) works too. **Chrome 157 does not read the system store.** It trusts the CA from `~/.pki/nssdb` (`certutil`, as the user) or from the `CACertificates` policy in `/etc/opt/chrome/policies/managed/` (as root), and rejects it again once that is removed |
| 7 | Login | **Yes.** `systemctl --user enable --now devid` runs the daemon. On the runner it needed lingering, since nothing logs in there. Login, logout and reboot on a desktop are left for the Xubuntu machine |
| 8 | On-demand's check | **Yes.** Reading `/proc/net/tcp{,6}` and matching socket inodes against the group's `/proc/<pid>/fd` finds an established connection and sees it close, as the user and without starting a process: 16 to 19 ms per call in the Python prototype with 170 to 210 processes |
| 9 | Idle cost | Devid's daemon 2.8 MB RSS; nginx master 1.9 MB and one worker 6.0 MB; the php-fpm 8.4 master with 0 workers 8.6 to 8.8 MB RSS (7.2 to 7.4 MB PSS), against about 15 MB on macOS (Phase 1). About 19 MB in all |
| 10 | Devid's binary | **Yes.** `x86_64-unknown-linux-musl` gives a 3.3 MB static-pie binary, and the test suite passes on musl: 129 passed, 0 failed, 1 ignored. The macOS-only tests do not build on Linux |

### Gotchas

- **zig's `native` target means the build machine's CPU.** The first PHP build used `native-native-gnu.2.17`, and on some runners php died at once with exit 132 (SIGILL), seemingly at random: on a run where Debian 12 failed, the next run's Ubuntu 24.04 failed instead. With `x86_64-linux-gnu.2.17` the build is for baseline x86_64, and every binary now also runs under QEMU's Nehalem CPU (x86-64-v2) in CI. In run 5, php with Xdebug, php-fpm, nginx and Devid all ran there.
- **static-php-cli needs a GitHub token in CI:** without `GITHUB_TOKEN`, its source downloads hit the anonymous API limit (403), as devid-binaries already knows.
- **GNU ld reads static archives in order,** unlike Apple's linker: nginx's configure did not find OpenSSL until `-lssl -lcrypto -lz` went into one `--start-group`.
- **zig turns `__DATE__` into an error** (`-Werror=date-time`) for phpize builds: apcu needs `CFLAGS=-Wno-date-time`.

### What this changes in the plan

- **PHP:** build for `x86_64-linux-gnu.2.17` in L2. That is older than the plan's Debian 12 floor, so Ubuntu 22.04 and later and Debian 12 and later all run it. Run every binary under QEMU's Nehalem CPU in the release workflow.
- **DNS:** the global drop-in. It is one file in the root step, and uninstall deletes it.
- **Chrome needs its own step,** to be decided in L3 and checked on the Xubuntu machine:
  - **`certutil` into `~/.pki/nssdb`** (as the user, the way mkcert does it) covers the Chromium-based browsers that read that database. It needs `libnss3-tools`, which a desktop may not have; the spike installed it.
  - **The `CACertificates` policy** needs no package and fits the root step. It covers only Chrome, and a managed policy makes Chrome say it is managed by an organization (to see on the Xubuntu machine).
- **Still for the Xubuntu machine:** reboots, logging in and out, Chrome (.deb) and Firefox (snap) in the Xfce session, and whether `libnss3-tools` is there.

## L2 results (2026-11-23)

Merged as hadziqmtqn/devid-binaries#19 (5f0f8f9) and released as `2026.11.23-19` (run 45160460967); the record with every check and gotcha is `docs/implementation-timeline/linux/2026-11-23-l2-runtime.md`.

- **What the release holds for `linux-x86_64`:** 30 archives. PHP 8.3.37, 8.4.28 and 8.5.13 (glibc 2.17, with Xdebug and pcov), the eight catalog extensions for each, nginx 1.30.7 (static musl), and the official Caddy 2.11.9 and Mailpit 1.34.0.
- **The manifest:** Linux builds sit in the same entries under the arch key `linux-x86_64`, next to the macOS keys `x86_64` and `arm64`.
- **Devid 0.18 on macOS:** `devid update --check` on the owner's Mac reads the new release ("everything is up to date (release 2026.11.23-19)"). Devid 0.18.0's own manifest readers, run on that manifest in a throwaway test, pick the `darwin-arm64` builds and ignore the Linux ones.
- **Checks on every Linux build:**
  - glibc's own libraries only, no glibc symbol newer than 2.17, and no rpath;
  - php, php-fpm, nginx and each extension's smoke test also run under QEMU's Nehalem CPU;
  - extensions load with `LD_BIND_NOW`, so a missing symbol fails at once.

### What this changes in the plan

- **imagick's fonts (L3, and the Xubuntu machine):**
  - The Linux build has no fontconfig, so its `type.xml` names the DejaVu fonts (`fonts-dejavu-core`) and Liberation fonts (`fonts-liberation`) in `/usr/share/fonts/truetype/`.
  - Check on the Xubuntu machine that both packages come with the desktop.
  - `devid doctor` can warn when imagick is installed and they are missing.
- **L3:** on Linux the daemon looks up the arch key `linux-x86_64`. On macOS it stays `x86_64` or `arm64`. The runtime release needs no further change for L3 or L4.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| L1 | **Spike** | the answers above are in this doc, and D-057 is accepted or changed to match them (done 2026-11-23, see "L1 results") |
| L2 | **Runtime for Linux** (devid-binaries): PHP 8.3–8.5, the extension catalog and nginx built for Linux; Linux mirrors of Caddy and Mailpit; Linux entries in the manifest | a runtime release holds `linux-x86_64` builds, and Devid 0.18 on macOS still updates from it (done 2026-11-23, release `2026.11.23-19`, see "L2 results") |
| L3 | **Daemon on Linux:** the platform interface; the Linux helper (DNS, ports, CA); the systemd user unit; the PATH file; the `/proc` check; `devid doctor` | unit tests cover the Linux plans; on a runner, `devid setup` serves a site at `https://<site>.test` (curl and headless Chrome), and `devid uninstall` puts every changed file back |
| L4 | **Install, update and CI:** `install.sh` and self-update on Linux; a Linux build in `release.yml`; `smoke.sh` on an Ubuntu runner | a dry run builds macOS and Linux and passes the smoke test on both |
| L5 | **Docs and release 0.19.0:** README, the guide's requirements and install pages | released, after Claude installs it on the Xubuntu machine from `install.sh`, runs setup, opens a site in Chrome and Firefox, updates it and uninstalls it |

L2 lands before L4, the way arm64 did for 0.14.0: the runtime release first, then Devid.

## Owner's answers (2026-11-23)

1. **Ubuntu first**, on Xubuntu. The owner sets up the machine.
2. **The desktop checks:** Claude runs them on the owner's old Intel Mac running Xubuntu, over SSH on the local network.
3. **The CLI and the TUI first,** as proposed. The app on Linux comes after them.

What the Xubuntu machine needs before Claude uses it:

- Xubuntu 24.04 or 26.04 LTS on x86_64.
- SSH with a key from the M1. Claude does not log in with a password.
- A test user with `sudo` (see "Ground rules").
- A logged-in Xfce session, so Claude can open the browsers on its display and take screenshots.
- Chrome (the .deb from Google) and Firefox (Xubuntu's snap), the way users would have them.

## Not in this phase

- **The app on Linux:** Tauri on WebKitGTK. The glib Dependabot alert dismissed on 2026-11-22 has to be looked at again then.
- **arm64 Linux:** GitHub's arm64 Ubuntu runners make it cheap once x86_64 ships.
- **Other distros:** Debian without systemd-resolved, Fedora (`update-ca-trust`) and Arch.
- **Apache on Linux.**
- **Windows:** it starts after Linux ships (D-044).
