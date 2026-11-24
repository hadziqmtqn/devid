#!/bin/bash
# Phase 13 A1 (docs/roadmap/phase-13.md), on an Arch VM prepared like a desktop install
# (prepare.sh: NetworkManager, systemd-resolved off). Devid 0.19.0 as released, then each part
# Arch does differently from Ubuntu:
#   1 DNS: option 2 (turn systemd-resolved on) and option 1 (NetworkManager's dnsmasq plugin),
#     each undone afterwards and compared with the state before
#   2 the CA through p11-kit (trust anchor), for curl
#   3 the browsers (fresh Chromium and Firefox profiles) with the CA only in the system store
#   4 ports 80/443 through 0.19's nftables table and unit
#   5 imagick's fonts with one type.xml for both layouts
#   and a PATH file for fish (A2)
# Every check is recorded, none stops the run; the table is the result.
#   bash spike/system.sh    (as the user arch, with passwordless sudo, ~/devid = devid 0.19.0)
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$HOME/a1-out"
HOST=hello.test
TEXT="hello from $HOST"
DEVID="$HOME/devid"
D="$HOME/.devid/bin/devid"
CA="$HOME/.devid/ca/devid-ca.crt"
mkdir -p "$OUT/logs"
TABLE="$OUT/table.md"
printf '| Result | Check |\n|---|---|\n' >"$TABLE"
n=0

check() {
  local name="$1" log
  shift
  n=$((n + 1))
  log="$OUT/logs/$(printf %02d "$n").log"
  echo "\$ $*" >"$log"
  if "$@" >>"$log" 2>&1; then
    echo "| PASS | $name |" >>"$TABLE"; echo "PASS: $name"
  else
    echo "| FAIL | $name |" >>"$TABLE"; echo "FAIL: $name (log $log)"; tail -15 "$log" | sed 's/^/    /'
  fi
}
refuse() { local name="$1"; shift; check "$name" invert "$@"; }
invert() { ! "$@"; }
note() { echo "| note | $1 |" >>"$TABLE"; echo "NOTE: $1"; }
section() { echo; echo "== $1"; echo "| | **$1** |" >>"$TABLE"; }
retry() {
  local end=$((SECONDS + $1))
  shift
  until "$@"; do
    [ "$SECONDS" -lt "$end" ] || return 1
    sleep 0.5
  done
}
serves() { curl -fsS --max-time 5 "$@" | grep -qx "$TEXT"; }
resolves_local() { getent hosts "$1" | grep -q '^127\.0\.0\.1'; }
# leaks <label> <name...>: queries for names outside .test that reach Devid's DNS port.
leaks() {
  local cap="$OUT/dns-$1.txt" pid
  shift
  sudo timeout 8 tcpdump -i lo -nn -l udp dst port 53535 >"$cap" 2>/dev/null &
  pid=$!
  sleep 2
  for name in "$@"; do getent hosts "$name" >/dev/null; done
  wait "$pid"
  grep '? ' "$cap" | grep -vc '\.test\. ' || true
}
state() { echo "resolv.conf $(sha256sum </etc/resolv.conf | cut -c1-16) $(test -L /etc/resolv.conf && echo link || echo file); resolved $(systemctl is-enabled systemd-resolved 2>&1)/$(systemctl is-active systemd-resolved 2>&1); NetworkManager $(systemctl is-active NetworkManager)"; }
browser() { timeout 90 python3 "$HERE/browser.py" "$1" "https://$HOST/" "$TEXT"; }

section "The VM"
. /etc/os-release
note "$PRETTY_NAME, kernel $(uname -r), $(ldd --version | head -1)"
note "before: $(state)"
note "/etc/resolv.conf: $(grep -v '^#' /etc/resolv.conf | tr '\n' ' ')"
note "libnssckbi.so is $(readlink -f /usr/lib/libnssckbi.so)"
note "tools: $(for c in update-ca-certificates update-ca-trust trust nft certutil fish; do printf '%s=%s ' "$c" "$(command -v "$c" || echo missing)"; done)"
note "the user's login shell: $(getent passwd "$USER" | cut -d: -f7)"
BEFORE="$(state)"
# NetworkManager rewrites /etc/resolv.conf a moment after it restarts.
unchanged() { [ "$(state)" = "$BEFORE" ]; }
BUNDLE="$(sha256sum </etc/ssl/certs/ca-certificates.crt)"

section "Devid 0.19.0 as released, on stock Arch"
check "devid setup --dry-run lists its steps" "$DEVID" setup --dry-run
"$DEVID" setup --no-php --no-server >"$OUT/setup.txt" 2>&1
note "devid setup ends with: $(tail -2 "$OUT/setup.txt" | tr '\n' ' ')"
check "the daemon runs as the systemd user unit" retry 20 systemctl --user is-active --quiet devid.service
check "PHP 8.5 installs from runtime-latest" "$D" php install 8.5
check "nginx installs" "$D" server install nginx
mkdir -p "$HOME/sites/hello/public"
echo "<?php echo \"$TEXT\";" >"$HOME/sites/hello/public/index.php"
check "devid link" "$D" link hello --path "$HOME/sites/hello"
check "nginx serves it on 8443 (curl -k)" retry 30 serves -k --resolve "$HOST:8443:127.0.0.1" "https://$HOST:8443/"

section "Ports 80/443: 0.19's nftables table and unit (A1.4)"
printf '%s\n' '# Devid: local-only redirect of the standard web ports to the web server'"'"'s high ports' \
  'table inet devid' 'delete table inet devid' 'table inet devid {' '  chain output {' \
  '    type nat hook output priority -100; policy accept;' '    ip daddr 127.0.0.1 tcp dport 80 redirect to :8088' \
  '    ip daddr 127.0.0.1 tcp dport 443 redirect to :8443' '  }' '}' | sudo tee /etc/devid-ports.nft >/dev/null
printf '%s\n' '[Unit]' "Description=Devid: send 127.0.0.1 ports 80 and 443 to Devid's web server" \
  'After=network-pre.target nftables.service' '' '[Service]' 'Type=oneshot' 'RemainAfterExit=yes' \
  'ExecStart=/usr/sbin/nft -f /etc/devid-ports.nft' 'ExecStop=/usr/sbin/nft delete table inet devid' '' \
  '[Install]' 'WantedBy=multi-user.target' | sudo tee /etc/systemd/system/devid-ports.service >/dev/null
check "/usr/sbin/nft exists (0.19's path)" test -x /usr/sbin/nft
sudo systemctl daemon-reload
check "devid-ports.service enables and starts" sudo systemctl enable --now devid-ports.service
check "the table is loaded" sudo nft list table inet devid
check "127.0.0.1:443 reaches nginx (curl -k)" retry 10 serves -k --resolve "$HOST:443:127.0.0.1" "https://$HOST/"
note "nftables tables: $(sudo nft list tables | tr '\n' ' '); iptables: $(pacman -Q iptables iptables-nft 2>/dev/null | tr '\n' ' ')"

section "DNS, option 2: turn systemd-resolved on (A1.1)"
sudo cp -a /etc/resolv.conf /etc/resolv.conf.devid-backup
sudo mkdir -p /etc/systemd/resolved.conf.d
printf '[Resolve]\nDNS=127.0.0.1:53535\nDomains=~test\n' | sudo tee /etc/systemd/resolved.conf.d/devid.conf >/dev/null
check "systemd-resolved enables and starts" sudo systemctl enable --now systemd-resolved.service
sudo ln -sfn ../run/systemd/resolve/stub-resolv.conf /etc/resolv.conf
check "NetworkManager restarts" sudo systemctl restart NetworkManager.service
check "$HOST resolves to 127.0.0.1" retry 30 resolves_local "$HOST"
check "other names still resolve" retry 30 getent hosts archlinux.org
note "resolvectl dns: $(resolvectl dns 2>&1 | tr '\n' ' '); domains: $(resolvectl domain 2>&1 | tr '\n' ' ')"
note "queries outside .test that reached Devid: $(leaks option2 archlinux.org example.org wikipedia.org)"
note "during: $(state)"

section "The CA through p11-kit (A1.2), and the browsers (A1.3)"
refuse "curl rejects the site before trust" serves "https://$HOST/"
refuse "Chromium rejects it before trust" browser chromium
refuse "Firefox rejects it before trust" browser firefox
check "trust anchor --store adds the CA" sudo trust anchor --store "$CA"
note "files it wrote: $(sudo find /etc/ca-certificates/trust-source -type f -newer "$OUT/setup.txt" | tr '\n' ' ')"
check "the system bundle holds it" grep -q "$(sed -n 2p "$CA")" /etc/ssl/certs/ca-certificates.crt
check "curl trusts https://$HOST (80/443, systemd-resolved)" retry 10 serves "https://$HOST/"
check "Chromium (fresh profile) trusts it from the system store" browser chromium
check "Firefox (fresh profile) trusts it from the system store" browser firefox
note "~/.pki/nssdb: $(test -e "$HOME/.pki/nssdb" && echo 'exists (Chromium made it)' || echo absent)"
check "Devid's PHP reaches it (cacert.pem)" bash -c "'$HOME/.devid/bin/php' -r 'echo file_get_contents(\"https://$HOST/\");' | grep -qx '$TEXT'"
check "trust anchor --remove takes it out" sudo trust anchor --remove "$CA"
refuse "curl rejects it again" serves "https://$HOST/"
refuse "Chromium rejects it again" browser chromium
refuse "Firefox rejects it again" browser firefox
check "the system bundle is as before" test "$(sha256sum </etc/ssl/certs/ca-certificates.crt)" = "$BUNDLE"

section "DNS option 2 undone"
check "systemd-resolved disables and stops" sudo systemctl disable --now systemd-resolved.service
sudo rm -f /etc/systemd/resolved.conf.d/devid.conf
sudo rmdir /etc/systemd/resolved.conf.d 2>/dev/null
sudo mv -f /etc/resolv.conf.devid-backup /etc/resolv.conf
check "NetworkManager restarts" sudo systemctl restart NetworkManager.service
check "other names resolve" retry 30 getent hosts archlinux.org
refuse "$HOST no longer resolves" resolves_local "$HOST"
note "after: $(state)"
check "resolv.conf and both services are as before" retry 15 unchanged

section "DNS, option 1: NetworkManager's dnsmasq plugin (A1.1)"
printf '[main]\ndns=dnsmasq\n' | sudo tee /etc/NetworkManager/conf.d/devid.conf >/dev/null
sudo mkdir -p /etc/NetworkManager/dnsmasq.d
echo 'server=/test/127.0.0.1#53535' | sudo tee /etc/NetworkManager/dnsmasq.d/devid.conf >/dev/null
check "NetworkManager restarts with dnsmasq" sudo systemctl restart NetworkManager.service
check "$HOST resolves to 127.0.0.1" retry 30 resolves_local "$HOST"
check "other names still resolve" retry 30 getent hosts archlinux.org
note "/etc/resolv.conf: $(grep -v '^#' /etc/resolv.conf | tr '\n' ' ')"
note "queries outside .test that reached Devid: $(leaks option1 archlinux.org example.org wikipedia.org)"
sudo rm -f /etc/NetworkManager/conf.d/devid.conf /etc/NetworkManager/dnsmasq.d/devid.conf
check "NetworkManager restarts without it" sudo systemctl restart NetworkManager.service
check "other names resolve" retry 30 getent hosts archlinux.org
refuse "$HOST no longer resolves" resolves_local "$HOST"
check "resolv.conf and both services are as before" retry 15 unchanged
note "after: $(state)"

section "Ports undone"
check "devid-ports.service stops and disables" sudo systemctl disable --now devid-ports.service
refuse "no devid table is left" sudo nft list table inet devid
sudo rm -f /etc/systemd/system/devid-ports.service /etc/devid-ports.nft
sudo systemctl daemon-reload

section "imagick's fonts (A1.5)"
check "imagick installs from the catalog" "$D" php ext install 8.5 imagick
ETC="$(dirname "$(find "$HOME/.devid" -name type.xml -path '*imagick*' | head -1)")"
note "imagick's etc: $ETC; Arch's fonts: $(ls /usr/share/fonts/TTF | grep -E '^(DejaVuSans|LiberationSans-Regular)\.ttf$' | tr '\n' ' ')"
# PHP itself, with Devid's ini: the php8.5 shim sets MAGICK_CONFIGURE_PATH to imagick's own etc.
PHP85="$OUT/php85"
printf '#!/bin/bash\nPHPRC=%s PHP_INI_SCAN_DIR=%s/conf.d exec %s "$@"\n' \
  "$HOME/.devid/php/8.5/etc" "$HOME/.devid/php/8.5/etc" "$HOME/.devid/php/8.5/bin/php" >"$PHP85"
chmod +x "$PHP85"
check "PHP 8.5 loads imagick" bash -c "'$PHP85' -m | grep -qx imagick"
refuse "the shipped type.xml (Debian paths) cannot draw text on Arch" bash "$HERE/fonts.sh" draw "$ETC" "$PHP85"
check "a type.xml for both layouts builds" bash "$HERE/fonts.sh" both "$ETC" "$OUT/fonts"
check "it draws text on Arch" bash "$HERE/fonts.sh" draw "$OUT/fonts" "$PHP85"

section "fish: a PATH file in conf.d (A2)"
mkdir -p "$HOME/.config/fish/conf.d"
echo "fish_add_path --global --move --path $HOME/.devid/bin # added by devid" >"$HOME/.config/fish/conf.d/devid.fish"
check "a new fish puts ~/.devid/bin first, even when PATH has it last" \
  test "$(env PATH="/usr/local/bin:/usr/bin:$HOME/.devid/bin" fish -c 'echo $PATH[1]')" = "$HOME/.devid/bin"
check "fish -c finds Devid's php" test "$(env PATH=/usr/local/bin:/usr/bin fish -c 'command -v php')" = "$HOME/.devid/bin/php"
refuse "no universal variable names Devid" grep -q devid "$HOME/.config/fish/fish_variables"
rm -f "$HOME/.config/fish/conf.d/devid.fish"
check "without the file, fish does not see Devid's php" test "$(env PATH=/usr/local/bin:/usr/bin fish -c 'command -v php' || true)" != "$HOME/.devid/bin/php"

echo
cat "$TABLE"
