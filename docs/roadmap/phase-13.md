# Phase 13 Plan: Arch-based Linux

> Scope: Devid 0.19's Linux support (the daemon, the CLI and the TUI) extended from Ubuntu to Arch Linux and the distributions built on it
> Verified against: 2026-11-24 (Devid 0.19.0 and 0.20.0-rc.1; Phase 12 done, see `phase-12.md`; spike run 45311846301; A2's Arch run 45318939599; A3's CI run 45351167579 and release dry run 45351179773; rc.1's release run 45377684962)
> Status: accepted 2026-11-24 (D-059, "Terima, mulai A1"), with the owner's answers in "Owner's answers". **A1 done 2026-11-24** (see "A1 results"); question 6 was answered on the Arch machine the same day (see "A1.6 results"). **A2 done 2026-11-24** (see "A2 results"), with the desktop session's PATH fixed by #139. **A3 done 2026-11-24** (see "A3 results"). **A4:** 0.20.0-rc.1 passed on the owner's Arch, Ubuntu and CachyOS machines the same day (see "rc.1 on the machines"); 0.20.0 is next.

On 2026-11-24, after 0.19.0 shipped Devid for Ubuntu, the owner named Arch-based distributions as a likely next target.

## What already works on Arch

These parts of 0.19 do not depend on Ubuntu:

- **The binaries:**
  - `devid` is a static musl binary.
  - The runtime (PHP 8.3 to 8.5, the extensions, nginx) needs glibc 2.17 or newer, and Arch always ships the newest glibc.
  - Caddy and Mailpit are static.
- **systemd:** the user unit for the daemon, and the system unit that loads the port redirect.
- **The browsers' databases:**
  - `certutil` comes with Arch's `nss` package, which Firefox and Chromium already need.
  - Firefox profiles live in `~/.mozilla/firefox`, which 0.19 already covers.
  - Chromium reads `~/.pki/nssdb`.
- **`install.sh`:** Arch's base system includes curl, and the script picks the binary from `uname`.
- **The rest:** on-demand's `/proc` check, the DNS server, the CA, the sites and every command.

## What differs from Ubuntu

| Part | Ubuntu (0.19) | Arch | Question for the spike |
|---|---|---|---|
| `.test` DNS | systemd-resolved runs by default; a drop-in sends `.test` to Devid | systemd-resolved is installed with systemd but **off by default**. Desktop installs usually run NetworkManager, which writes `/etc/resolv.conf` itself | A1.1 |
| System CA store | `update-ca-certificates` (Debian) | p11-kit: `trust anchor --store`, or a file in `/etc/ca-certificates/trust-source/anchors/` plus `update-ca-trust`. Fedora works the same way | A1.2 |
| Browsers | Chrome, Brave and Firefox ignore the system store | Arch builds NSS against p11-kit's trust module, so Firefox and Chromium from Arch's repos may read the system store directly | A1.3 |
| Ports 80/443 | `nft` is installed | the `nftables` package may be missing | A1.4 |
| imagick's fonts | `/usr/share/fonts/truetype/dejavu/` and `…/liberation/` | `/usr/share/fonts/TTF/` (`ttf-dejavu`) and `/usr/share/fonts/liberation/` (`ttf-liberation`) | A1.5 |
| Login shell | bash (Devid writes `~/.bashrc` and `~/.profile`; zsh gets `~/.zshenv` and `~/.zshrc`) | bash on Arch. **fish on CachyOS**, which no PATH line of 0.19 reaches | A1.8 |
| PATH for the desktop | `~/.profile`, read at login | Arch's `~/.bash_profile` reads `~/.bashrc`, and a desktop session may not read `~/.profile` | A1.6 |
| Prerequisites | `sudo apt install curl libnss3-tools` | `sudo pacman -S --needed nss nftables`, if anything | A1.4 |

## Proposal (D-059)

- **Target:** plain Arch Linux (rolling, x86_64) with systemd, the owner's choice. Then two derivatives as checks:
  - **CachyOS** (the owner's addition):
    - It uses Arch's repositories plus its own, which hold packages rebuilt for x86-64-v3 and v4 CPUs. Devid's baseline x86-64 builds do not depend on them.
    - It has its own kernel, and installs KDE Plasma by default.
    - Its default shell is fish (see "fish").
  - **EndeavourOS:** the closest derivative to plain Arch.
  - Manjaro lags Arch's packages and has its own defaults, so it follows later. Others are covered as far as they behave like Arch, without being promised.
- **Detect capabilities, not distribution names.** `platform/linux.rs` keeps one Linux implementation and picks each part by what the system has:
  - `update-ca-certificates` or `update-ca-trust`;
  - systemd-resolved running or not;
  - NetworkManager present or not.

  `/etc/os-release` only names the system in messages and `devid doctor`. That way Fedora, Debian and others gain whatever they share with Ubuntu or Arch.
- **The same model as 0.19:**
  - one root step at setup, which `devid uninstall` undoes, with any file Devid replaces backed up first;
  - everything else as the user;
  - no new resident process.
- **DNS where systemd-resolved is off:** the owner allows option 2, turning systemd-resolved on, if A1.1 confirms it works and uninstall restores the system. The options, from least to most change to the system:
  1. **NetworkManager's dnsmasq plugin:** a drop-in that routes `.test` to Devid. It needs the `dnsmasq` package and changes how NetworkManager resolves every name.
  2. **Turning systemd-resolved on**, as Ubuntu has it: enable the service and point `/etc/resolv.conf` at its stub. This works with NetworkManager, but changes the system's resolver. Uninstall would put back the file and the service's state.
  3. **Refusing with a message:** setup stops and links a guide page that explains option 2 for the user to do by hand.
- **CA:** with p11-kit, `trust anchor --store` adds the CA and `trust anchor --remove` takes it out. If A1.3 confirms the browsers read the system store, setup can skip certutil there, and doctor still checks the browsers.
- **Fonts:** devid-binaries' `type-linux.xml` lists the Debian paths and the Arch paths together. ImageMagick drops entries whose files are missing (L2), so one file serves both.
- **PATH:** if A1.6 shows a desktop session misses `~/.profile`, the PATH line also goes in `~/.config/environment.d/`. systemd user sessions read it on every distribution.
- **fish** (the owner's addition), for CachyOS and for fish users on any system, macOS included:
  - With fish as the login shell, setup writes `~/.config/fish/conf.d/devid.fish`, which fish reads in every shell, interactive or not.
  - The file holds one marked line, such as `fish_add_path --global --move --path ~/.devid/bin # added by devid`. `--move` keeps Devid's commands first, as the bash and zsh lines do.
  - `devid path on` and `off`, `devid doctor`, the app's Add to PATH and uninstall handle it like the other startup files. Uninstall deletes the file, since Devid created it.

## A1: what the spike must answer

All on GitHub's runners first. The Arch machine (see "Where it is tested") is only for the desktop checks at the end.

1. **DNS:** on a stock Arch desktop with NetworkManager, does each option above send only `.test` to Devid and leave every other name alone? Do `getent`, curl and the browsers see it? And does uninstall restore the original exactly (`/etc/resolv.conf`, the services' state)?
2. **CA:** do `trust anchor --store` and `--remove` make curl and PHP (with Devid's `cacert.pem`) trust and then distrust a `.test` site?
3. **Browsers:** with the CA only in the system store, do Arch's Firefox and Chromium trust it, and do Chrome and Brave from the AUR? If not, does certutil (already in 0.19) cover them?
4. **Ports:** with the `nftables` package, does 0.19's `devid-ports.service` work unchanged, next to iptables-nft and Docker? What does a system without `nft` need?
5. **Fonts:** does one `type.xml` with both sets of paths draw text on Arch and on Ubuntu?
6. **PATH:** in a GNOME or KDE session on Arch, do apps started from the menu see `~/.devid/bin` with the 0.19 lines? If not, does `environment.d` fix it?
7. **The e2e:** can `linux-e2e.sh` run on Arch in CI, with root steps, systemd and a browser?
8. **CachyOS:**
   - Is fish the login shell for a new user (`getent passwd`)?
   - Is systemd-resolved on or off out of the box?
   - Do its repositories change anything the answers above rely on: NSS with p11-kit, `nftables`, the fonts' paths?
   - Can CI run it too, as its own image or as Arch plus CachyOS's repositories, or does it need a VM of its own?

## A1 results (2026-11-24)

### Setup

- **Where:** `.github/workflows/spike-arch.yml`, which runs only on `spike/arch-*` branches and publishes nothing. The scripts are in `spike/arch/` (see its README).
- **The machine:** Arch Linux's official cloud image (kernel 7.3.5, glibc 2.44) under QEMU with KVM on an `ubuntu-24.04` runner, reached over SSH. `prepare.sh` makes it what a desktop install is to Devid: NetworkManager, systemd-resolved off, and a regular `/etc/resolv.conf` that NetworkManager writes. Chromium 158 and Firefox 160 come from Arch's repositories.
- **Devid:** the released 0.19.0 binary. The root steps 0.19 cannot do on Arch yet (DNS, the CA, the ports) were done by `system.sh` the way A2 would do them, and each was undone and compared with the state before.
- **The other jobs:** `fonts-ubuntu` draws text with the two-layout `type.xml` on Ubuntu 24.04. `cachyos` reads CachyOS's Docker image (build 20261122) and its installer's settings.
- **Runs:** 45304296052 to 45311846301. The sixth passed all 52 checks. Runs 1 to 5 failed on the VM's setup or on the spike's own scripts, listed under "Gotchas".

### Devid 0.19.0 on Arch as released

- The daemon's user unit, `devid php install 8.5`, `devid server install nginx`, `devid link` and HTTPS on port 8443 work unchanged.
- `devid setup` stops before its root step with "the system setup needs ca-certificates: `sudo apt install ca-certificates`". 0.19 looks for `update-ca-certificates`, which Arch does not have, and names apt.
- `devid setup --dry-run` fails with "no Devid CA yet" until the daemon has run once. `devid helper install` checks for the CA before it looks at `--dry-run`, so this happens on every OS.

### Answers

| # | Question | Answer |
|---|---|---|
| 1 | DNS | **Both options work, and both undo exactly.** Option 2, the owner's choice: 0.19's drop-in (`DNS=127.0.0.1:53535`, `Domains=~test`), systemd-resolved enabled and restarted, `/etc/resolv.conf` linked to its stub, then NetworkManager restarted, which hands its own servers to resolved. `getent`, curl, Chromium and Firefox reach `hello.test`, other names still resolve, and tcpdump saw 0 queries outside `.test` reach Devid's port. To undo it, the saved `/etc/resolv.conf` goes back and resolved stops together with the sockets that start it (`systemd-resolved-varlink.socket` and `systemd-resolved-monitor.socket`, from `systemctl show -p TriggeredBy`). The file's content and type and both services' states then match the start. Option 1 (NetworkManager's dnsmasq plugin: `dns=dnsmasq` and `server=/test/127.0.0.1#53535`) gives the same results and needs the `dnsmasq` package |
| 2 | CA | **Yes.** `trust anchor --store` writes `/etc/ca-certificates/trust-source/Devid_Local_CA.p11-kit` and adds the CA to `/etc/ssl/certs/ca-certificates.crt`, and curl trusts the site. `trust anchor --remove` takes it out, curl rejects the site again, and the bundle is byte for byte as before. PHP trusts the site through Devid's `cacert.pem`, which appends the CA to the system bundle (D-008), so PHP does not depend on the system store |
| 3 | Browsers | **Arch's own browsers read the system store.** Arch's NSS links `libnssckbi.so` to p11-kit's trust module (`/usr/lib/pkcs11/p11-kit-trust.so`). With fresh profiles, Chromium 158 and Firefox 160 reject the site before the CA is stored (`net::ERR_CERT_AUTHORITY_INVALID`, and Firefox's insecure-certificate error), trust it once it is stored, and reject it again after `--remove`. Firefox with the CA in its profile as well (0.19's `certutil`) also loads the site, so the two ways do not clash. Chrome and Brave from the AUR were not tested |
| 4 | Ports | **0.19's file and unit work unchanged.** `/usr/sbin` is a link to `/usr/bin` on Arch, so `/usr/sbin/nft` exists. The unit loads the `inet devid` table next to Arch's `iptables` 1.8.13, 127.0.0.1:443 reaches nginx, and `disable --now` removes the table. Arch's `base` does not include `nftables`, so setup has to ask for it. Docker was not tested |
| 5 | Fonts | **Yes, with Arch's real paths.** Arch keeps DejaVu in `/usr/share/fonts/TTF/` but Liberation in `/usr/share/fonts/liberation/`. The shipped `type.xml` cannot draw text on Arch. One that lists each font in both layouts draws DejaVu Sans, Arial and Helvetica on Arch and on Ubuntu 24.04 |
| 6 | PATH in a desktop session | **Not answered:** it needs the KDE Plasma machine |
| 7 | The e2e in CI | **Yes.** The VM has what an e2e needs: root steps, systemd for the system and the user, NetworkManager, nftables, and headless Chromium and Firefox through Selenium. The whole job takes about 3 minutes: 77 s to boot, 64 s for packages and a reboot, 42 s of checks. `linux-e2e.sh` needs A2's Arch steps before it can pass there |
| 8 | CachyOS | **fish:** yes, for the user its installer creates. The installer (Calamares, `CachyOS/cachyos-calamares`) creates the user with `/bin/bash`, then its `shell-setup` script runs `chsh` to fish, or to zsh when fish is missing. Its default package selection installs `cachyos-fish-config` and `cachyos-zsh-config`. `useradd`'s default stays bash. **systemd-resolved:** most likely on, as on Ubuntu. `cachyos-settings` sets NetworkManager's `dns=systemd-resolved`, and the live ISO enables resolved, but the installer's own list of services does not name it. **Repositories:** its `[cachyos]` repository (874 packages) comes before core, extra and multilib. nftables is preinstalled, and the fonts come from Arch's `extra` with Arch's paths. The image has no NSS installed, so its p11-kit link was not checked. **CI:** the Docker image answers package questions only (no systemd, no installer), so the full check stays with A4 |
| – | fish's PATH file (for A2) | **Works as designed.** With `~/.config/fish/conf.d/devid.fish` holding `fish_add_path --global --move --path ~/.devid/bin`, a new fish puts `~/.devid/bin` first even when PATH has it last, `fish -c` finds Devid's `php`, and no universal variable mentions Devid. Without the file, fish no longer sees Devid's `php` |

### Gotchas

- **The cloud image waits for the clock:** it enables `systemd-time-wait-sync`, and its `pacman-init.service` (`Before=sshd.service`, `After=time-sync.target`) makes sshd wait for an NTP sync at every boot. When DNS broke (next point), the clock never synced and SSH never came back. A desktop install has neither, so the spike disables the wait.
- **`/etc/resolv.conf` must never go missing:** at boot, systemd's `tmpfiles.d/systemd-resolve.conf` (`L! /etc/resolv.conf … ../run/systemd/resolve/stub-resolv.conf`) recreates it as a link to resolved's stub. NetworkManager then leaves DNS to resolved, and with resolved off nothing resolves. Arch's `filesystem` package ships a regular file, which NetworkManager writes.
- **Stopping systemd-resolved:** `systemctl disable --now systemd-resolved` leaves its sockets listening, and the next lookup through nss-resolve starts it again.
- **Selenium tells Firefox to accept any certificate** (`acceptInsecureCerts` is true in its Firefox defaults), unless the script sets it to false.
- **Arch's packages:** `python-selenium` is only in the AUR, so the spike installs Selenium in a venv. `ttf-liberation` is not in `/usr/share/fonts/TTF/`.
- **The runner:** removing `/var/lib/man-db/auto-update` before `apt-get install` skips man-db's index rebuild. With that, the boot step took 77 to 279 seconds in runs 4 to 6, against about seven minutes in run 3.
- **A boot that loses SSH:** with journald's `ForwardToConsole=yes`, the boot's whole journal lands in QEMU's serial log, which the job uploads.

### What this changes in the plan

- **DNS (A2):** option 2, done as the spike did it. Option 1 is not needed.
  - Setup records `/etc/resolv.conf` (a file or a link, and its content) and the enabled and active states of resolved and its sockets. Uninstall puts each one back.
  - `/etc/resolv.conf` is never left missing.
  - Where resolved already runs (Ubuntu, and most likely CachyOS), nothing changes from 0.19.
- **CA (A2):** `trust anchor --store` and `--remove` where `update-ca-certificates` is missing.
- **Browsers:** keep 0.19's `certutil` step on Arch too, instead of skipping it as D-059 allowed. It covers browsers from outside Arch's repositories (Chrome and Brave from the AUR), and run 6 showed it does no harm next to the system store. doctor keeps checking the browsers' databases.
- **Prerequisites (A2):** where pacman is the package manager, the message reads `sudo pacman -S --needed …` with whatever is missing: `nftables` on a stock install, and `nss` only where neither Firefox nor Chromium is installed, since both depend on it.
- **Fonts (devid-binaries, A2):** `type-linux.xml` lists every DejaVu and Liberation entry a second time, with `/usr/share/fonts/TTF/` and `/usr/share/fonts/liberation/`.
- **0.19 bugs for A2:** the apt message on Arch, and `setup --dry-run` before the daemon's first start (every OS).
- **A3:** the spike's VM becomes the `arch-e2e` job, with the gotchas above built in.
- **Still open:**
  - A1.6, PATH in a desktop session, on the KDE Plasma machine the owner sets up on the old Mac;
  - CachyOS's resolved and NSS, on its A4 machine;
  - Chrome and Brave from the AUR, on the Arch machine.

## A2 results (2026-11-24)

PR #137 (branch `arch-a2`) and hadziqmtqn/devid-binaries#23 (branch `arch-fonts`).

### What shipped

- **The root step by capability.** `platform/linux.rs` reads what the system has into `System`, which the tests set directly. Ubuntu's steps are unchanged.
  - **CA:** where Debian's `update-ca-certificates` is missing and p11-kit's store is there, the CA is copied to `/etc/ca-certificates/trust-source/anchors/devid-ca.crt`, then `update-ca-trust extract` runs. Uninstall removes the copy and extracts again. A1 had tried `trust anchor --store`. The anchors folder is p11-kit's other documented way, and its uninstall does not need the user's CA file, which `trust anchor --remove` reads and which may have changed or gone by then.
  - **DNS, where systemd-resolved is off:**
    1. `/etc/devid-resolved.state` comes first, so an uninstall after a failed step still turns resolved off;
    2. the drop-in, as on Ubuntu;
    3. `systemctl enable systemd-resolved.service`, then the restart;
    4. `/etc/resolv.conf` is linked to `../run/systemd/resolve/stub-resolv.conf`, with what was there, a file or a link, kept as `/etc/resolv.conf.devid-backup`;
    5. `systemctl try-restart NetworkManager.service`, so NetworkManager hands its servers to resolved.

    A second setup with resolved on takes Ubuntu's steps and keeps the first backup. Uninstall, with the note:
    - puts the backup back. If Devid's link had nothing behind it, it removes the link and NetworkManager writes a new file, so `/etc/resolv.conf` is never left missing;
    - runs `disable --now` on resolved and the sockets that start it (from `TriggeredBy`);
    - restarts NetworkManager and removes the note.

    Setup only turns resolved on when it is off, so the note stands for "disabled and inactive", the state of a stock install for the service and its sockets alike.
  - **Status:** "resolver" also needs systemd-resolved to run.
  - **Helper:**
    - `Link` (with a backup made once) and `Restore` are new;
    - `Remove` treats a dangling link as present, so a backup that is a link goes back instead of the path being deleted.
- **Before sudo:**
  - systemd, nftables and either CA tool, with `sudo pacman -S --needed …` where pacman is the package manager;
  - with resolved off, NetworkManager must run. Elsewhere setup says to turn resolved on by hand.
- **Hints and checks:** certutil (`nss`) and the fonts (`ttf-dejavu ttf-liberation`) are named for pacman, and the DejaVu check finds Arch's path.
- **Opening a terminal:** after Debian's `x-terminal-emulator` and `$TERMINAL`, konsole, gnome-terminal, kgx, xfce4-terminal, alacritty, kitty, foot and xterm, each with its own working-folder option.
- **fish, on every OS:** `~/.config/fish/conf.d/devid.fish` with `fish_add_path --global --move --path '<bin>' # added by devid`, as A1 tried it.
  - Each startup file now has its own PATH line: `rc_line(paths, rc)` and `rc_has_line(paths, rc)`.
  - `devid path`, doctor, the app's Add to PATH and uninstall use them.
  - `DEVID_LOGIN_SHELL` overrides the login shell, for the tests.
- **The 0.19 bug:** the dry runs of `setup` and `helper install` work before the daemon has made the CA.
- **`scripts/linux-e2e.sh` on Arch too:** it compares `/etc/resolv.conf` (link or content) and whether resolved and its two sockets are enabled and active, takes Firefox's profile from where each system keeps it, and uses Chromium where Chrome is missing.
- **Fonts (devid-binaries#23):** `type-linux.xml` lists each of its nine fonts in both layouts. Users get them with the runtime release that follows its merge.

### Verification

| Check | Result |
|---|---|
| macOS, the owner's Mac | `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass. `cargo test --no-fail-fast` with a test HOME: everything passes but `passwd_home_matches_home`, which fails only because HOME is replaced. `~/.zshrc` and `~/.zshenv` unchanged |
| New tests | the Ubuntu, Arch and after-setup action lists and their order; uninstall with and without a backup; the prerequisite messages; the terminal choice; `Link` and `Restore` on files and links, dangling ones too; fish's file and line; `devid path on` and `off` with fish as the login shell; the dry runs before the CA exists |
| Arch, run 45318939599 on the throwaway `spike/arch-a2` branch | This branch's musl build on an Arch VM under QEMU/KVM, set up like a desktop (NetworkManager, systemd-resolved off): <ul><li>the dry runs showed the Arch plan before anything ran;</li><li>`linux-e2e.sh` installed PHP 8.5.13 and nginx;</li><li>curl and headless Chromium got "e2e 8.5.13 Linux" from `https://e2e.test/`;</li><li>doctor said "all good";</li><li>after `uninstall --purge` the snapshot matched: `/etc/resolv.conf` is NetworkManager's file again, resolved and both sockets are disabled and inactive, and the anchors folder is empty.</li></ul>The job took about 3.5 minutes: 47 s to build with a warm cache, 67 s to boot, 58 s for packages and a reboot, and the e2e itself |
| CI on #137 | `rust`, `gui` and the Ubuntu `linux-e2e` |

### Found on the way

- **Arch's bash startup files** (for A1.6):
  - `/etc/skel` gives every user a `~/.bash_profile`, so bash login shells never read `~/.profile`;
  - its `~/.bashrc` returns early in non-interactive shells, before Devid's line at the end.

  A desktop session that starts through a non-interactive login shell would miss `~/.devid/bin`. The KDE Plasma machine shows whether Plasma's does.
- **`systemctl disable --now` with the sockets named** still prints "its triggering units are still active" for the service, before the sockets stop. Afterwards all three are inactive.

### Not in A2

- PATH in a desktop session (A1.6, and `environment.d` if it is needed) waits for the KDE Plasma machine.
- The `arch-e2e` job in CI is A3, built from `spike/arch/vm.sh`, `prepare.sh` and this e2e.

## A3 results (2026-11-24)

PR #138 (branch `arch-a3`).

### What shipped

- **`scripts/arch-vm.sh`** joins the spike's `vm.sh` with the parts of `prepare.sh` that `linux-e2e.sh` needs.
  - **`boot`** opens KVM to the runner's user and installs QEMU (man-db's rebuild skipped). It downloads Arch's latest cloud image, checks it against its `.SHA256`, and boots it with cloud-init's user `arch`. Then, as root in the VM:
    - pacman upgrades and installs `networkmanager nss nftables chromium ttf-liberation diffutils`, retrying a failed mirror twice;
    - systemd-networkd and systemd-resolved and their sockets are disabled one at a time, and NetworkManager is enabled with an explicit connection for `eth0`;
    - `/etc/resolv.conf` becomes the filesystem package's regular file;
    - `systemd-time-wait-sync` is disabled, and journald forwards to the serial console.

    After a reboot the script checks that names resolve.
  - **`e2e <devid>`** copies that devid and `scripts/linux-e2e.sh` into the VM and runs the e2e there.
  - **`ssh`** and **`copy`** are there for debugging.
- **`ci.yml`:** the `arch-e2e` job builds the musl binary, boots the VM and runs the e2e on every pull request and push to main. Its apt step has a 10-minute timeout and the boot a 15-minute one, and a failed run uploads the VM's serial log.
- **`release.yml`:** an `arch-e2e` job runs on the `devid-linux-x86_64` artifact that ships, after `build-linux`. `publish` now needs `build`, `build-linux` and `arch-e2e`.
- **The spike's files** stay in `spike/arch/` as A1's record, and their README points to the script.

### Verification

| Run | Result |
|---|---|
| CI 45351167579 on #138 | `rust`, `gui`, `linux-e2e` and `arch-e2e` pass. In `arch-e2e`, Arch's image (kernel 7.3.4) came up with NetworkManager running and systemd-resolved off. The e2e then passed in full: setup, curl and headless Chromium got "e2e 8.5.13 Linux", doctor said "all good", and the snapshot matched after uninstall. Times: build 53 s, boot and setup 5 min 19 s, e2e 7 s |
| Release dry run 45351179773 (`dry_run=true`, on `arch-a3`) | `build` (x86_64 and arm64), `build-linux` and `arch-e2e` pass; `publish` is skipped. `arch-e2e` ran the same e2e on the downloaded artifact: boot and setup about 3 min, e2e 10 s |

### What this costs

- **Every pull request** gets one more Linux job, which standard runners in a public repository run for free (D-043). At about 8 minutes, it is now the slowest job, so a pull request's checks take about 8 minutes instead of 2 (run 45351167579).
- **A release:** `arch-e2e` takes about 3 minutes after `build-linux`. In the dry run it ended at 07:58, before the Intel macOS build, which ended at 08:01, so `publish` did not start any later.

## A1.6 results (2026-11-24)

### The machine

The owner's Arch Linux VM in Parallels on the old Mac, reached over SSH on the local network:
- **Installed with `archinstall`:** profile Desktop with KDE Plasma (6.8.3, Wayland, SDDM), NetworkManager, the `linux` kernel (7.3.5), zram swap, and `openssh firefox chromium`.
- **4 GB of memory.** The first 942 MiB were too little.
- **The network starts bridged, NAT does not work:** under Parallels' Shared network the VM got `10.211.55.x`, which only the old Mac reaches.
- **What `archinstall` left:** systemd-resolved off and a regular `/etc/resolv.conf` that NetworkManager writes, as A1 assumed. `nftables` was already there as a dependency of `iptables`.
- **No apps of its own:** the KDE profile installs Plasma, Discover and Flatpak but no apps. The owner then got Konsole and Dolphin from Discover, which installed them as Flatpaks.

Devid ran from the build of main in release dry run 45351179773, with `devid setup` for real:
- resolved was turned on and `/etc/resolv.conf` linked to its stub;
- NetworkManager restarted without dropping SSH;
- the CA went in through p11-kit, then PHP 8.5.13 and nginx were installed;
- `https://hello.test` served, doctor said "all good", and Firefox and Chromium opened the site without a warning.

### The answer

**No: with 0.19's lines, apps opened from the menu do not see `~/.devid/bin`.**
- SDDM starts Plasma through `/usr/share/sddm/scripts/wayland-session`, which for bash runs `exec $SHELL --login`.
- That shell is not interactive, so it reads `~/.bash_profile` (Arch's `/etc/skel` gives every user one) and never `~/.profile`.
- `~/.bash_profile` sources `~/.bashrc`, which returns at its first line (`[[ $- != *i* ]] && return`), before Devid's line at the end.

| Logged in with | PATH of `plasmashell` and `systemctl --user show-environment` |
|---|---|
| Devid's line in `~/.bashrc` and `~/.profile` (0.19, and A2) | no `~/.devid/bin` |
| the same line also at the end of `~/.bash_profile` | `/home/khadziq/.devid/bin` first |

A new Konsole tab found Devid's `php` either way, since an interactive shell reads `~/.bashrc`. `environment.d` was not needed.

For the other shells:
- **zsh:** a login shell reads `~/.zshenv`, which Devid already writes.
- **fish:** SDDM's script sources `~/.profile`, then exports what `fish --login` sets, and Devid's `conf.d` file runs there.

### Found on the way

- **Firefox 160 made its profile in `~/.config/mozilla/firefox`** (the XDG layout), where Devid did not look. On Arch, Firefox trusted the CA anyway through p11-kit. Ubuntu's snap reads only its profile's database, so the same move there would break it.
- **Konsole as a Flatpak** (`org.kde.konsole`) starts its shell in the home folder even with `--workdir`. Devid's "open a terminal" could not use it, and with no other terminal it failed with "No such file or directory".
- **The TUI waited for the terminal to close,** since most Linux terminals run until their window does.
- **Flatpak apps** (Dolphin here, or a Flatpak editor) run in a sandbox and never see programs outside it, Devid's `php` included. Native editors from pacman or the AUR do.

### What changed (#139)

- **bash:** the line goes to `~/.bash_profile` where it exists, else to `~/.profile`.
- **Cleaning up:** `devid path off` and uninstall clean every startup file Devid may have written, for every shell.
- **Firefox:** the CA also goes to profiles in the XDG folder, in the snap's XDG folder and in Flathub's Flatpak.
- **The terminal:** with no terminal that opens in a folder, the TUI names one to install. It no longer waits for the terminal to close.
- **`linux-e2e.sh`** checks that a non-interactive login shell puts `~/.devid/bin` first, on Ubuntu and on Arch.

## A4 plan (2026-11-24)

- **0.20.0-rc.1 first, as a pre-release,** as 0.19.0 went.
  - It is not GitHub's Latest, so `devid-latest`, `devid self-update` and every install stay on 0.19.0.
  - Its release notes name Arch, and say that setup turns systemd-resolved on where it is off.
- **On the Arch KDE Plasma machine** (see "A1.6 results"; clean again after #139's run), Claude, over SSH:
  - installs rc.1 with `DEVID_VERSION=0.20.0-rc.1` through `install.sh`, runs setup, and links a site;
  - the owner opens it in Firefox and Chromium, logs out and in, and reboots;
  - Claude checks the desktop session's PATH, systemd-resolved, the ports unit and the user unit after the reboot.
- **On the Ubuntu 24.04 machine,** which still runs 0.19.0 with a test site:
  - rc.1 goes over 0.19.0, as an update would;
  - setup must take Ubuntu's steps as before (systemd-resolved already on, `update-ca-certificates`);
  - the PATH lines stay in `~/.bashrc` and `~/.profile`, and the Firefox snap's profile keeps the CA.
- **CachyOS** (KDE Plasma, fish): the owner sets up a third VM on the old Mac, switched off between uses. There:
  - the same install, with fish as the shell;
  - `php` found in a new terminal and from the menu;
  - a site in Firefox, and uninstall.
- **Then 0.20.0,** with the README and the guide's Linux page naming Arch.
  - The guide also says that Flatpak apps cannot see Devid's commands, and that Discover's Flatpak Konsole cannot open in a folder.
  - On both machines, `devid self-update` goes from rc.1 to 0.20.0, and uninstall is compared with the state before.

## rc.1 on the machines (2026-11-24)

The release run (45377684962) passed every job the first time, `arch-e2e` included, and published `devid-v0.20.0-rc.1` as a pre-release.

### Arch Linux with KDE Plasma

- **Install:** `install.sh` with `DEVID_VERSION=0.20.0-rc.1`, on the machine left clean by #139's run.
  - The PATH line went to `~/.bashrc` and `~/.bash_profile`.
  - The CA went to the system store through p11-kit, to `~/.pki/nssdb` and to the Firefox profile in `~/.config/mozilla/firefox`.
  - `https://hello.test` served, and doctor said "all good".
- **The owner** opened the site in Firefox and Chromium, then restarted the machine. After the reboot, over SSH:
  - `plasmashell` and `systemctl --user show-environment` had `~/.devid/bin` first;
  - systemd-resolved, `devid-ports.service` with its nftables table, and the user unit were running, and no unit had failed;
  - the site served, and doctor said "all good".
- **Browsers:** Chromium showed the site as secure. Firefox's window was caught mid-redraw in the owner's screenshot. Its profile held the CA, and Firefox run headless with a copy of that profile's certificates loaded the page without a warning.

### Ubuntu 24.04, over 0.19.0

- **Install:** `install.sh` put rc.1 over 0.19.0, as an update would.
  - Setup found the PATH lines in `~/.bashrc` and `~/.profile` and the system setup already in place, so it ran no sudo.
  - It restarted the daemon on the new binary.
- **Compared with the state before:** only the version changed.
  - The startup files, the units, the resolved drop-in, the nftables file, the CA file and the system bundle matched byte for byte.
  - No `~/.bash_profile` appeared, and the Firefox snap's profile kept the CA.
- **Ubuntu's steps:** `devid helper install --dry-run` lists the same steps as 0.19, with systemd-resolved left as it is and `update-ca-certificates` for the CA.
- **Afterwards:** doctor said "all good", and the test site kept serving.

### CachyOS with KDE Plasma and fish

The owner installed CachyOS from its 2026-11-23 ISO (Calamares, Limine) in a third Parallels VM. It differs from the Arch machine in four ways:
- **fish** is the login shell, from the installer's "CachyOS shell configuration".
- **Plasma Login Manager** (6.8.3) replaces SDDM. Its `wayland-session` script has SDDM's fish branch: it sources `/etc/profile` and `~/.profile`, then exports what `fish --login` sets.
- **systemd-resolved is on.** NetworkManager has `dns=systemd-resolved` and writes a regular `/etc/resolv.conf` naming the stub (127.0.0.53), and `nsswitch.conf` asks resolved too.
- **UFW is on** and denies incoming connections. The owner allowed SSH for the test.

It also has Firefox 160 (profile in `~/.config/mozilla/firefox`), a native Konsole, `nss`, `nftables` and `update-ca-trust`, and no Chromium.

What rc.1 did there:
- **Setup:**
  - The PATH line went to `~/.config/fish/conf.d/devid.fish`.
  - The system step added the drop-in and restarted systemd-resolved, and left `/etc/resolv.conf` alone.
  - The CA went in through p11-kit, `~/.pki/nssdb` and the Firefox profile.
  - UFW needed nothing, since Devid's traffic stays on 127.0.0.1.
- **Checks:**
  - A fish login shell and a new interactive fish found Devid's `php`.
  - `https://hello.test` served, and doctor said "all good".
  - The owner opened the site in Firefox without a warning, and no exception was stored.
- **After a restart and a login:**
  - `plasmashell`, `systemctl --user show-environment`, and Konsole opened from the menu all had `~/.devid/bin` first.
  - `php -v` in that Konsole ran Devid's PHP 8.5.13.
- **Uninstall (`devid uninstall --purge`):** compared with the record from before setup:
  - Every file matched byte for byte: `.bashrc`, `.bash_profile`, `.zshrc`, `config.fish`, the CA bundle and `nsswitch.conf`.
  - `/etc/resolv.conf` was untouched, and the units, sockets and UFW rules were as before.
  - No Devid CA, nftables table or file was left.
  - Empty folders remained: `~/.config/fish/conf.d`, `~/.config/systemd/user`, and `~/.pki/nssdb` with an empty database, which Ubuntu and Arch also keep.
- **Unrelated:** CachyOS's `ananicy-cpp` crashed at every boot of this VM, before Devid was installed too, reading `/sys/devices/system/cpu/cpu2` from Parallels' CPU list.

### Left for 0.20.0

- On the Arch and Ubuntu machines, `devid self-update` from rc.1 to 0.20.0, then uninstall, compared with the records from before.

## Where it is tested

- **GitHub's runners**, free in a public repository. They carry the spike and then every pull request, like `linux-e2e` for Ubuntu. GitHub has no Arch runners, so Arch runs on an Ubuntu runner, in one of two ways:
  - **A VM:** Arch's official cloud image under QEMU with KVM, booted with an SSH key through cloud-init. This is the closest to a real install: its own systemd, NetworkManager and resolver.
  - **A container with systemd:** `systemd-nspawn --boot` on Arch's root filesystem. It is lighter, but it shares the runner's network, so it cannot answer the DNS question.
- **A second Parallels VM on the old Mac**, beside the Ubuntu one: Arch with KDE Plasma, set up by the owner, and reached over SSH like the Ubuntu machine. It is for the real browsers, logging in and out, and reboots.
  - KDE is the most common desktop among Arch users.
  - Its tray and Wayland session differ from the GNOME that Ubuntu already covers.
- **CachyOS:** in CI if A1.8 finds a way. Otherwise in a third VM on the old Mac, used only for A4's check and switched off between uses, since the old Mac's memory is shared with the other two.
- **Not used:**
  - the M1, where D-013 rules out VMs and RAM is tight;
  - a VPS, which has no desktop and costs money every month.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| A1 | **Spike** on GitHub's runners | the answers above are in this doc, and D-059 is accepted or changed to match them |
| A2 | **The platform layer by capability:** CA through p11-kit, DNS per A1.1, nftables and prerequisite messages, fonts in devid-binaries, PATH per A1.6, and fish's `conf.d` file on every system | unit tests cover the Arch plans and fish next to the Ubuntu ones, a CLI test runs `devid path on` and `off` with fish as the login shell, and the Ubuntu `linux-e2e` still passes |
| A3 | **CI:** an `arch-e2e` job, the twin of `linux-e2e`: setup, a site over HTTPS (curl and a browser), doctor, and uninstall with every file compared | it passes on every pull request, and release builds run it before publishing |
| A4 | **Docs and release:** the guide's Linux page and the README name Arch and its prerequisites; a pre-release goes on the Arch machine first, as rc.1 did on Ubuntu | released, after Claude installs it on the Arch machine from `install.sh`, opens a site in Firefox and a Chromium browser, reboots, updates it and uninstalls it; and on CachyOS, the same install with fish as the shell, `php` found in a new terminal, a site in Firefox, and uninstall |

## Owner's answers (2026-11-24)

1. **Plain Arch first** ("Arch murni"), then EndeavourOS as the derivative check.
   - Later the same day, the owner added CachyOS and fish support ("masukkan CachyOS dan dukungan fish ke rencana").
   - CachyOS becomes the first derivative check, and fish goes into A2.
2. **DNS:** where systemd-resolved is off, setup may turn it on and point `/etc/resolv.conf` at its stub (option 2), with the old file and the service's state restored by uninstall. A1.1 still has to show that it works next to NetworkManager.
3. **The machine's desktop:** no preference. Claude proposes KDE Plasma (see "Where it is tested").
4. **Order:** no preference. Claude proposes this phase next, then Windows (D-044), and the Linux desktop app only when users ask for it: the CLI and the TUI already cover everything the app does.

## Not in this phase

- **The Linux desktop app:** a separate question, answered in chat on 2026-11-24. It is not needed for Linux to be useful.
- **arm64 Linux, Fedora and Debian without systemd-resolved.** They gain from the capability checks, but are not tested or promised.
- **Apache on Linux.**
- **Distributions without systemd** (Artix, Void).
