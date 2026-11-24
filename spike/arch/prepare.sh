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
# Run 4: with /etc/resolv.conf gone, systemd-tmpfiles (systemd-resolve.conf, `L!`) put back the
# link to systemd-resolved's stub at boot, and NetworkManager left DNS to a resolved that was not
# running: nothing resolved, and the clock never synced (what held sshd in run 3). A desktop
# install has the filesystem package's regular file, which NetworkManager writes.
rm -f /etc/resolv.conf
printf '# Resolver configuration file.\n# See resolv.conf(5) for details.\n' >/etc/resolv.conf
# Run 2: after the reboot SSH never came back, and the serial console showed eth0 down. That table
# comes from cloud-init while NetworkManager is still starting, so it proved little; the explicit
# connection for eth0 stays anyway, as cloud-init's networkd files would no longer apply.
rm -f /etc/systemd/network/*.network
mkdir -p /etc/NetworkManager/conf.d /etc/NetworkManager/system-connections
printf '[device-spike-eth0]\nmatch-device=interface-name:eth0\nmanaged=1\n' >/etc/NetworkManager/conf.d/10-spike-eth0.conf
printf '[connection]\nid=eth0\ntype=ethernet\ninterface-name=eth0\nautoconnect=true\n\n[ipv4]\nmethod=auto\n\n[ipv6]\nmethod=auto\n' \
  >/etc/NetworkManager/system-connections/eth0.nmconnection
chmod 600 /etc/NetworkManager/system-connections/eth0.nmconnection
# Run 3: SSH again did not come back, and sshd never even started. The cloud image enables
# systemd-time-wait-sync, and its pacman-init.service (Before=sshd.service, After=time-sync.target)
# makes sshd wait for an NTP sync on every boot. A desktop install has neither, so the wait goes.
systemctl disable systemd-time-wait-sync.service
# The second boot's journal on the serial console, and the state of the network, the clock and
# systemd's pending jobs half a minute in, for when SSH still does not come back.
mkdir -p /etc/systemd/journald.conf.d
printf '[Journal]\nForwardToConsole=yes\n' >/etc/systemd/journald.conf.d/spike-console.conf
printf '%s\n' '[Unit]' 'After=NetworkManager.service' '[Service]' 'Type=simple' 'SyslogIdentifier=spike-netinfo' \
  "ExecStart=/bin/sh -c 'sleep 30; systemctl list-jobs --no-pager; ip -br addr; ip route; nmcli -t device; ls -l /etc/resolv.conf; cat /etc/resolv.conf; timedatectl show -p NTPSynchronized; systemctl --no-pager status sshd | head -5'" \
  '[Install]' 'WantedBy=multi-user.target' >/etc/systemd/system/spike-netinfo.service
systemctl enable spike-netinfo.service
set +x
echo "before the reboot: sshd $(systemctl is-enabled sshd.service), time-wait-sync $(systemctl is-enabled systemd-time-wait-sync.service), NTP synchronized $(timedatectl show -p NTPSynchronized --value)"
