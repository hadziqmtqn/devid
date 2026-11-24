#!/bin/bash
# Arch Linux in a VM on a GitHub Ubuntu runner, set up the way a desktop install is to Devid (Phase 13
# A3, D-059), for the arch-e2e jobs. Arch's official cloud image boots under QEMU with KVM; cloud-init
# gives the user `arch` this run's SSH key and passwordless sudo. Then the image is made a desktop:
# NetworkManager instead of systemd-networkd, systemd-resolved off, a regular /etc/resolv.conf, and
# the packages linux-e2e.sh needs (certutil from nss, nftables, Chromium). The Phase 13 A1 spike
# (spike/arch/) found each step below.
# It changes the runner (KVM's permissions, QEMU's packages), so it is for throwaway runners only.
#   scripts/arch-vm.sh boot                 download, check and boot the image, set it up, reboot
#   scripts/arch-vm.sh e2e <path to devid>  run scripts/linux-e2e.sh in the VM with that devid
#   scripts/arch-vm.sh ssh <command>        run a command in the VM as arch
#   scripts/arch-vm.sh copy <local> <remote>
set -euo pipefail
W="${VM_DIR:-$PWD/vm}"
HERE="$(cd "$(dirname "$0")" && pwd)"
IMAGE_URL="https://geo.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg.qcow2"
PORT=2222
SSH_OPTS=(-i "$W/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR)
SSH=(ssh -p "$PORT" "${SSH_OPTS[@]}" -o ConnectTimeout=5 -o ServerAliveInterval=15 arch@127.0.0.1)

wait_ssh() {
  for _ in $(seq 90); do
    "${SSH[@]}" true 2>/dev/null && return 0
    sleep 4
  done
  echo "FAIL: no SSH to the VM. Its serial console, on sshd, the network and the clock:" >&2
  tr -d '\r' <"$W/serial.log" | grep -aE 'sshd|OpenSSH|NetworkManager|eth0|timesyncd|time-wait|resolv' | tail -80 >&2
  echo "... and its last lines:" >&2
  tr -d '\r' <"$W/serial.log" | grep -av '^$' | tail -40 >&2
  return 1
}

# Run as root in the VM, before its reboot.
desktop() {
  cat <<'EOF'
set -euo pipefail
# The mirrors can fail for a moment; pacman does not retry by itself.
retry() { for i in 1 2 3; do "$@" && return 0; echo "retrying ($i): $*" >&2; sleep 10; done; return 1; }
retry pacman -Sy --noconfirm --needed archlinux-keyring
retry pacman -Su --noconfirm
retry pacman -S --noconfirm --needed networkmanager nss nftables chromium ttf-liberation diffutils
# One at a time: systemctl stops at a unit it does not know.
for unit in systemd-networkd.service systemd-networkd.socket systemd-networkd-wait-online.service \
  systemd-resolved.service systemd-resolved-varlink.socket systemd-resolved-monitor.socket; do
  systemctl disable "$unit" 2>/dev/null || true
done
systemctl enable NetworkManager.service
# cloud-init's networkd files no longer apply; NetworkManager gets eth0 explicitly.
rm -f /etc/systemd/network/*.network
mkdir -p /etc/NetworkManager/conf.d /etc/NetworkManager/system-connections
printf '[device-arch-vm-eth0]\nmatch-device=interface-name:eth0\nmanaged=1\n' >/etc/NetworkManager/conf.d/10-arch-vm-eth0.conf
printf '[connection]\nid=eth0\ntype=ethernet\ninterface-name=eth0\nautoconnect=true\n\n[ipv4]\nmethod=auto\n\n[ipv6]\nmethod=auto\n' \
  >/etc/NetworkManager/system-connections/eth0.nmconnection
chmod 600 /etc/NetworkManager/system-connections/eth0.nmconnection
# The cloud image links /etc/resolv.conf to systemd-resolved's stub. A desktop has the filesystem
# package's regular file, which NetworkManager writes; without one, systemd-tmpfiles makes the link
# again at boot and nothing resolves.
rm -f /etc/resolv.conf
printf '# Resolver configuration file.\n# See resolv.conf(5) for details.\n' >/etc/resolv.conf
# The image's systemd-time-wait-sync and pacman-init.service hold sshd until the clock syncs; a
# desktop has neither.
systemctl disable systemd-time-wait-sync.service
# The boot's journal on the serial console, for a boot that loses SSH.
mkdir -p /etc/systemd/journald.conf.d
printf '[Journal]\nForwardToConsole=yes\n' >/etc/systemd/journald.conf.d/arch-vm-console.conf
EOF
}

case "${1:?usage: arch-vm.sh boot|e2e|ssh|copy}" in
  boot)
    mkdir -p "$W"
    # The standard runners have /dev/kvm, but only root may use it until a udev rule opens it.
    echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' |
      sudo tee /etc/udev/rules.d/99-kvm4all.rules >/dev/null
    sudo udevadm control --reload-rules && sudo udevadm trigger --name-match=kvm
    # man-db's index rebuild made installing QEMU take minutes.
    sudo rm -f /var/lib/man-db/auto-update
    sudo apt-get update -qq
    sudo apt-get install -y -qq --no-install-recommends qemu-system-x86 qemu-utils cloud-image-utils >/dev/null
    curl -fsSL --retry 3 -o "$W/arch.qcow2" "$IMAGE_URL"
    curl -fsSL --retry 3 -o "$W/arch.qcow2.SHA256" "$IMAGE_URL.SHA256"
    (cd "$W" && sed 's/ .*cloudimg.qcow2$/  arch.qcow2/' arch.qcow2.SHA256 | sha256sum -c -)
    qemu-img resize "$W/arch.qcow2" +8G >/dev/null
    rm -f "$W/key" "$W/key.pub"
    ssh-keygen -q -t ed25519 -N '' -f "$W/key"
    cat >"$W/user-data" <<EOF
#cloud-config
users:
  - name: arch
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys: [$(cat "$W/key.pub")]
EOF
    echo "instance-id: devid-arch-vm" >"$W/meta-data"
    cloud-localds "$W/seed.img" "$W/user-data" "$W/meta-data"
    qemu-system-x86_64 -enable-kvm -cpu host -smp "$(nproc)" -m 6144 \
      -drive file="$W/arch.qcow2",if=virtio \
      -drive file="$W/seed.img",if=virtio,format=raw \
      -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:$PORT-:22 \
      -display none -serial file:"$W/serial.log" -daemonize -pidfile "$W/qemu.pid"
    wait_ssh
    "${SSH[@]}" 'grep PRETTY_NAME /etc/os-release; uname -r'
    # pacman's progress is left out; sed, unlike grep -v, succeeds when it prints nothing.
    desktop | "${SSH[@]}" 'sudo bash -s' 2>&1 |
      sed -nE '/^(::|  |\(|.*downloading|checking|loading|installing|upgrading|resolving|looking|Packages|Total|Net|warning: .*is up to date)/!p' | tail -20
    "${SSH[@]}" 'sudo systemctl reboot' || true
    sleep 10
    wait_ssh
    "${SSH[@]}" 'echo "NetworkManager $(systemctl is-active NetworkManager), systemd-resolved $(systemctl is-active systemd-resolved)"
      ls -l /etc/resolv.conf
      getent hosts archlinux.org >/dev/null || { echo "FAIL: nothing resolves in the VM"; exit 1; }'
    ;;
  e2e)
    devid="${2:?usage: arch-vm.sh e2e <path to devid>}"
    scp -q -P "$PORT" "${SSH_OPTS[@]}" "$devid" "$HERE/linux-e2e.sh" arch@127.0.0.1:
    "${SSH[@]}" "chmod +x '$(basename "$devid")' && bash linux-e2e.sh './$(basename "$devid")'"
    ;;
  ssh) shift; "${SSH[@]}" "$@" ;;
  copy) scp -q -P "$PORT" "${SSH_OPTS[@]}" "$2" "arch@127.0.0.1:$3" ;;
  *) echo "usage: arch-vm.sh boot|e2e|ssh|copy" >&2; exit 2 ;;
esac
