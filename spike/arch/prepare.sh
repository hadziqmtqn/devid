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
# Run 2: after the reboot NetworkManager left eth0 down, so SSH never came back. cloud-init wrote
# networkd's .network files on the first boot; they go, and eth0 gets an explicit connection.
rm -f /etc/systemd/network/*.network
mkdir -p /etc/NetworkManager/conf.d /etc/NetworkManager/system-connections
printf '[device-spike-eth0]\nmatch-device=interface-name:eth0\nmanaged=1\n' >/etc/NetworkManager/conf.d/10-spike-eth0.conf
printf '[connection]\nid=eth0\ntype=ethernet\ninterface-name=eth0\nautoconnect=true\n\n[ipv4]\nmethod=auto\n\n[ipv6]\nmethod=auto\n' \
  >/etc/NetworkManager/system-connections/eth0.nmconnection
chmod 600 /etc/NetworkManager/system-connections/eth0.nmconnection
# The network's state on the serial console after the reboot, for when SSH does not come back.
printf '%s\n' '[Unit]' 'After=NetworkManager.service' '[Service]' 'Type=oneshot' \
  "ExecStart=/bin/sh -c 'sleep 20; ip -br addr; nmcli -t device; journalctl -b -u NetworkManager --no-pager | tail -25'" \
  'StandardOutput=tty' 'TTYPath=/dev/ttyS0' '[Install]' 'WantedBy=multi-user.target' >/etc/systemd/system/spike-netinfo.service
systemctl enable spike-netinfo.service
