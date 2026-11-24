# Phase 13 A1 spike: Arch-based Linux

These scripts answer the A1 questions in `docs/roadmap/phase-13.md` through `.github/workflows/spike-arch.yml`, on GitHub's Ubuntu runners. The workflow runs on `spike/arch-*` branches only and publishes nothing.

| Script | Question | What it does |
|---|---|---|
| `vm.sh` | 7 | boots Arch's official cloud image under QEMU with KVM on the runner. cloud-init gives the user `arch` the run's SSH key and passwordless sudo. Also runs commands and copies files over SSH |
| `prepare.sh` | – | turns the cloud image into what a desktop install is to Devid: NetworkManager with systemd-resolved off, plus the packages the checks need |
| `system.sh` | 1–5, fish | Devid 0.19.0 as released, on stock Arch. Then: 0.19's nftables table; DNS through systemd-resolved turned on (option 2) and through NetworkManager's dnsmasq plugin (option 1), each undone and compared with the state before; the CA through `trust anchor`; fresh Chromium and Firefox profiles with the CA only in the system store; imagick's fonts; a fish PATH file |
| `fonts.sh` | 5 | writes a `type.xml` that lists every font in both the Debian and the Arch layout, and draws text with a given one |
| `browser.py` | 3 | opens a URL in headless Chromium or Firefox through Selenium, with Arch's own drivers |

The workflow's `fonts-ubuntu` job checks the two-layout `type.xml` on Ubuntu too. Its `cachyos` job reads CachyOS's Docker image: repositories, NSS, systemd presets and its settings packages (question 8).

Question 6 (PATH in a desktop session) needs the KDE Plasma machine, not CI.

The results go in `docs/roadmap/phase-13.md`, "A1 results".

CI's `arch-e2e` jobs (A3) use `scripts/arch-vm.sh`, which joins `vm.sh` and the parts of `prepare.sh` that `scripts/linux-e2e.sh` needs.
