#!/bin/bash
# Devid on Linux end to end (Phase 12 L3, D-057; Arch, D-059), for a GitHub Ubuntu runner or an Arch
# VM set up like a desktop (spike/arch/prepare.sh): `devid setup` with the real root step (sudo,
# systemd-resolved, nftables, the CA store), Chrome's database and the systemd user unit; a Firefox
# profile's own database; a linked site over https://<site>.test through ports 80/443, for curl and
# headless Chrome or Chromium; then `devid uninstall --purge`, after which every file setup touched,
# /etc/resolv.conf and systemd-resolved's state must be as before. On Arch, setup turns
# systemd-resolved on and uninstall turns it off again.
# It changes the system, so it is for throwaway runners and VMs only, never a workstation.
# Usage: scripts/linux-e2e.sh <path to devid>
set -euo pipefail
DEVID="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
SITE=e2e
T="$(mktemp -d)"
step() { printf '\n== %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

# The files setup may change. Their hashes (or absence) must match again after uninstall.
FILES=(
  "$HOME/.bashrc" "$HOME/.profile" "$HOME/.zshenv" "$HOME/.zshrc" "$HOME/.config/fish/conf.d/devid.fish"
  "$HOME/.config/systemd/user/devid.service"
  /etc/systemd/resolved.conf.d/devid.conf /etc/devid-ports.nft /etc/systemd/system/devid-ports.service
  /etc/devid-resolved.state /etc/resolv.conf.devid-backup
  /usr/local/share/ca-certificates/devid-ca.crt /etc/ca-certificates/trust-source/anchors/devid-ca.crt
  /etc/ssl/certs/ca-certificates.crt
)
snapshot() {
  for f in "${FILES[@]}"; do
    if [ -e "$f" ]; then sha256sum "$f"; else echo "absent $f"; fi
  done
  # /etc/resolv.conf as a link or as content, and whether systemd-resolved and the sockets that
  # start it are enabled and running.
  if [ -L /etc/resolv.conf ]; then echo "resolv.conf -> $(readlink /etc/resolv.conf)"; else echo "resolv.conf $(sha256sum </etc/resolv.conf)"; fi
  for unit in systemd-resolved.service systemd-resolved-varlink.socket systemd-resolved-monitor.socket; do
    echo "$unit $(systemctl is-enabled "$unit" 2>&1 || true) $(systemctl is-active "$unit" 2>&1 || true)"
  done
  # The browsers' databases: each certificate, prefixed with its database, without certutil's
  # header. Setup may create Chrome's itself (Chrome would on its first run), and uninstall leaves it
  # empty, so an empty database prints nothing, as a missing one does.
  for db in "$HOME/.pki/nssdb" "$FIREFOX"; do
    if [ -f "$db/cert9.db" ]; then
      certutil -d "sql:$db" -L | awk -v db="$db" 'NF && !/^Certificate Nickname/ && !/SSL,S\/MIME,JAR\/XPI/ {print db ": " $0}'
    fi
  done
}
# A Firefox profile with its own certificate database, where each system's Firefox keeps it. Ubuntu's
# snap reads nothing else, so setup must add the CA there and uninstall take it out.
if command -v pacman >/dev/null; then
  FIREFOX="$HOME/.mozilla/firefox/e2e.default"
else
  FIREFOX="$HOME/snap/firefox/common/.mozilla/firefox/e2e.default"
fi
CHROME="$(command -v google-chrome || command -v chromium || true)"

finish() {
  status=$?
  if [ "$status" -ne 0 ]; then
    echo "--- daemon.log" && tail -50 "$HOME/.devid/log/daemon.log" 2>/dev/null || true
    for f in "$HOME"/.devid/log/*.log; do [ -f "$f" ] && echo "--- $f" && tail -20 "$f"; done
    systemctl --user status devid.service --no-pager 2>&1 | tail -20 || true
  fi
  rm -rf "$T"
}
trap finish EXIT

step "The user's systemd"
# A desktop or SSH login starts the user's systemd; a runner has no login session, so lingering does.
sudo loginctl enable-linger "$USER"
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/bus" ] && break; sleep 0.2; done
systemctl --user is-system-running || true
mkdir -p "$FIREFOX"
certutil -N -d "sql:$FIREFOX" --empty-password
snapshot > "$T/before.txt"

step "devid setup"
"$DEVID" setup
D="$HOME/.devid/bin/devid"
[ -x "$D" ] || fail "setup did not install $D"
systemctl --user is-active --quiet devid.service || fail "the user unit does not run"
grep -qx "export PATH=\"$HOME/.devid/bin:\$PATH\" # added by devid" "$HOME/.bashrc" || fail "no PATH line in ~/.bashrc"
"$D" helper status
certutil -d "sql:$FIREFOX" -L -n "Devid Local CA" >/dev/null || fail "setup left the CA out of the Firefox profile"

step "A site at https://$SITE.test"
mkdir -p "$T/$SITE/public"
echo '<?php echo "e2e ", PHP_VERSION, " ", PHP_OS;' > "$T/$SITE/public/index.php"
"$D" link "$SITE" --path "$T/$SITE"
getent hosts "$SITE.test" | grep -q '^127\.0\.0\.1 ' || fail "$SITE.test does not resolve through systemd-resolved"
getent hosts example.org >/dev/null || fail "other names stopped resolving"
# Devid reloads nginx with SIGHUP and replies at once. Until the old worker quits it still accepts
# connections with the previous config, which does not know the site ("unrecognized name"), so one
# good answer is not enough: wait for three in a row, as smoke.sh's settle does.
good=0
for _ in $(seq 100); do
  if curl -fs -o /dev/null "https://$SITE.test/"; then good=$((good + 1)); else good=0; fi
  [ "$good" -ge 3 ] && break
  sleep 0.1
done
page="$(curl -fsS "https://$SITE.test/")" || fail "curl cannot reach https://$SITE.test/"
echo "curl: $page"
grep -q '^e2e 8\.[0-9.]* Linux$' <<<"$page" || fail "unexpected page: $page"
curl -fsS -o /dev/null "http://$SITE.test/" || fail "port 80 does not reach the web server"

# A fresh profile, as a new user's browser would have; Chrome still reads ~/.pki/nssdb.
[ -n "$CHROME" ] || fail "no Chrome or Chromium to open the site with"
dom="$("$CHROME" --headless=new --disable-gpu --user-data-dir="$T/chrome" --dump-dom "https://$SITE.test/" 2>/dev/null)"
echo "$(basename "$CHROME"): $(tr -d '\n' <<<"$dom" | cut -c1-200)"
grep -q 'e2e 8\.' <<<"$dom" || fail "headless $(basename "$CHROME") did not load https://$SITE.test/ (a certificate error?)"

step "devid doctor"
# A new terminal would have ~/.devid/bin on PATH; this shell started before setup.
PATH="$HOME/.devid/bin:$PATH" "$D" doctor

step "devid uninstall --purge"
"$D" uninstall --purge
snapshot > "$T/after.txt"
diff "$T/before.txt" "$T/after.txt" || fail "uninstall left changed files (see the diff above)"
if sudo nft list table inet devid >/dev/null 2>&1; then fail "the devid nftables table is still loaded"; fi
if systemctl --user cat devid.service >/dev/null 2>&1; then fail "the user unit is still known"; fi
if systemctl cat devid-ports.service >/dev/null 2>&1; then fail "devid-ports.service is still known"; fi
[ ! -e "$HOME/.devid" ] || fail "~/.devid is still there"
if getent hosts "$SITE.test" >/dev/null; then fail "$SITE.test still resolves"; fi
echo
echo "linux-e2e: all checks passed"
