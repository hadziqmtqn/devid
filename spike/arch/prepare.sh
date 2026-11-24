#!/bin/bash
# Phase 13 A1: make Arch's cloud image look like a desktop install to Devid, then reboot.
# - The packages the checks need: NetworkManager, NSS (certutil), nftables, Chromium and Firefox
#   with their WebDriver servers, Arch's fonts, fish, dnsmasq (DNS option 1), dig and tcpdump.
#   Selenium is only in the AUR, so it goes in a venv.
# - The cloud image runs systemd-networkd and systemd-resolved. A desktop install runs
#   NetworkManager with systemd-resolved off, and NetworkManager writes /etc/resolv.conf itself.
# Run as root: sudo bash prepare.sh
set -euxo pipefail
pacman -Sy --noconfirm --needed archlinux-keyring
pacman -Su --noconfirm
pacman -S --noconfirm --needed networkmanager nss nftables chromium firefox geckodriver python \
  ttf-dejavu ttf-liberation fish dnsmasq bind tcpdump openssl curl
python -m venv /opt/selenium
/opt/selenium/bin/pip install -q selenium
systemctl disable systemd-networkd.service systemd-networkd.socket systemd-networkd-wait-online.service \
  systemd-resolved.service || true
systemctl enable NetworkManager.service
rm -f /etc/resolv.conf
