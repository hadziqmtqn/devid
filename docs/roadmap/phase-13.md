# Phase 13 Plan: Arch-based Linux

> Scope: Devid 0.19's Linux support (the daemon, the CLI and the TUI) extended from Ubuntu to Arch Linux and the distributions built on it
> Verified against: 2026-11-24 (Devid 0.19.0; Phase 12 done, see `phase-12.md`)
> Status: accepted 2026-11-24 (D-059, "Terima, mulai A1"), with the owner's answers in "Owner's answers". A1 is in progress.

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
| imagick's fonts | `/usr/share/fonts/truetype/dejavu/` | `/usr/share/fonts/TTF/` (`ttf-dejavu`, `ttf-liberation`) | A1.5 |
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
