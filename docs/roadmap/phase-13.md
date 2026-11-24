# Phase 13 Plan: Arch-based Linux

> Scope: Devid 0.19's Linux support (the daemon, the CLI and the TUI) extended from Ubuntu to Arch Linux and the distributions built on it
> Verified against: 2026-11-24 (Devid 0.19.0; Phase 12 done, see `phase-12.md`; spike run 45311846301)
> Status: accepted 2026-11-24 (D-059, "Terima, mulai A1"), with the owner's answers in "Owner's answers". **A1 done 2026-11-24** (see "A1 results"), except question 6, which needs the Arch machine.

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
