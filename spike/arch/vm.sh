#!/bin/bash
# Phase 13 A1 (docs/roadmap/phase-13.md): an Arch Linux VM on a GitHub Ubuntu runner, from Arch's
# official cloud image, under QEMU with KVM. cloud-init gives the user `arch` this run's SSH key
# and passwordless sudo.
#   spike/arch/vm.sh boot     download, check and boot the image; wait for SSH
#   spike/arch/vm.sh ssh ...  run a command in the VM (as arch)
#   spike/arch/vm.sh copy <local> <remote>
#   spike/arch/vm.sh wait     wait for SSH again (after a reboot)
set -euo pipefail
W="${VM_DIR:-$PWD/vm}"
IMAGE_URL="https://geo.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg.qcow2"
PORT=2222
SSH=(ssh -p "$PORT" -i "$W/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR
  -o ConnectTimeout=5 -o ServerAliveInterval=15 arch@127.0.0.1)

wait_ssh() {
  for _ in $(seq 90); do
    "${SSH[@]}" true 2>/dev/null && return 0
    sleep 4
  done
  echo "FAIL: no SSH to the VM; the end of its serial console:" >&2
  tail -40 "$W/serial.log" >&2
  return 1
}

case "${1:?usage: vm.sh boot|ssh|copy|wait}" in
  boot)
    mkdir -p "$W"
    # KVM: the standard runners expose /dev/kvm, but only to root until a udev rule opens it.
    echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' |
      sudo tee /etc/udev/rules.d/99-kvm4all.rules >/dev/null
    sudo udevadm control --reload-rules && sudo udevadm trigger --name-match=kvm
    sudo apt-get update -qq && sudo apt-get install -y -qq --no-install-recommends qemu-system-x86 qemu-utils cloud-image-utils >/dev/null
    ls -l /dev/kvm
    curl -fsSL --retry 3 -o "$W/arch.qcow2" "$IMAGE_URL"
    curl -fsSL --retry 3 -o "$W/arch.qcow2.SHA256" "$IMAGE_URL.SHA256"
    (cd "$W" && sed 's/ .*cloudimg.qcow2$/  arch.qcow2/' arch.qcow2.SHA256 | sha256sum -c -)
    qemu-img resize "$W/arch.qcow2" +16G >/dev/null
    ssh-keygen -q -t ed25519 -N '' -f "$W/key"
    cat >"$W/user-data" <<EOF
#cloud-config
users:
  - name: arch
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys: [$(cat "$W/key.pub")]
EOF
    echo "instance-id: devid-spike-arch" >"$W/meta-data"
    cloud-localds "$W/seed.img" "$W/user-data" "$W/meta-data"
    qemu-system-x86_64 -enable-kvm -cpu host -smp 3 -m 8192 \
      -drive file="$W/arch.qcow2",if=virtio \
      -drive file="$W/seed.img",if=virtio,format=raw \
      -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:$PORT-:22 \
      -display none -serial file:"$W/serial.log" -daemonize -pidfile "$W/qemu.pid"
    wait_ssh
    "${SSH[@]}" 'cat /etc/os-release | head -3; uname -r'
    ;;
  ssh) shift; "${SSH[@]}" "$@" ;;
  copy) scp -q -P "$PORT" -i "$W/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR "$2" "arch@127.0.0.1:$3" ;;
  wait) sleep 10; wait_ssh ;;
esac
