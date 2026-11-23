#!/bin/bash
# Phase 12 L1 (docs/roadmap/phase-12.md), questions 3 to 9, on a GitHub Ubuntu runner:
# Devid's own daemon (its CA and DNS) under a systemd user unit; nginx and php-fpm from this
# spike's builds; ports 80/443 through nftables; .test through systemd-resolved; CA trust for
# curl, PHP, Chrome and Firefox; the /proc connection check; idle memory; and an uninstall that
# puts everything back. Every check is recorded, none stops the run; the table is the result.
#   BIN=<dir with devid, nginx, php, php-fpm, modules/> PYTHON=<python with selenium> spike/linux/system.sh
set -uo pipefail

BIN="$(cd "${BIN:?set BIN}" && pwd)"
PYTHON="${PYTHON:-python3}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${OUT:-$PWD/system-out}"
HOST=spike.test
TEXT="hello from $HOST"
DEVID_HOME="$HOME/.devid"
CA="$DEVID_HOME/ca/devid-ca.crt"
NSSDB="sql:$HOME/.pki/nssdb"
mkdir -p "$OUT/logs"
TABLE="$OUT/table.md"
printf '| Result | Check |\n|---|---|\n' >"$TABLE"
n=0

# check <name> <command...>: record PASS or FAIL; a failure prints the end of the command's output.
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
# refuse <name> <command...>: the command must fail ("before trust", "after uninstall").
refuse() { local name="$1"; shift; check "$name" invert "$@"; }
invert() { ! "$@"; }
note() { echo "| note | $1 |" >>"$TABLE"; echo "NOTE: $1"; }
section() { echo; echo "== $1"; echo "| | **$1** |" >>"$TABLE"; }
# retry <seconds> <command...>
retry() {
  local end=$((SECONDS + $1))
  shift
  until "$@"; do
    [ "$SECONDS" -lt "$end" ] || return 1
    sleep 0.3
  done
}
serves() { curl -fsS --max-time 5 "$@" | grep -qx "$TEXT"; }
resolves_local() { getent hosts "$1" | grep -q '^127\.0\.0\.1'; }
# leaks <label> <name...>: how many queries for names outside .test reach Devid's DNS port while
# the names are looked up through the system resolver.
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
mem() {
  awk '/^Rss:/ {r = $2} /^Pss:/ {p = $2} END {printf "%.1f MB RSS, %.1f MB PSS", r / 1024, p / 1024}' "/proc/$1/smaps_rollup"
}

section "Runner"
note "$(lsb_release -ds), kernel $(uname -r), $(ldd --version | head -1)"
note "/etc/resolv.conf -> $(readlink -f /etc/resolv.conf); systemd-resolved $(systemctl is-active systemd-resolved)"
note "$(nft --version 2>&1); docker $(systemctl is-active docker 2>&1); $(sudo ufw status 2>&1 | head -1)"
note "$(google-chrome --version 2>&1); $(firefox --version 2>&1 | tail -1) at $(readlink -f "$(command -v firefox)" 2>&1)"

section "The daemon under a systemd user unit (question 7)"
sudo loginctl enable-linger "$USER"
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
check "the user manager runs (lingering on)" retry 20 test -S "$XDG_RUNTIME_DIR/bus"
mkdir -p "$HOME/.config/systemd/user"
cat >"$HOME/.config/systemd/user/devid.service" <<EOF
[Unit]
Description=Devid daemon (Phase 12 L1 spike)

[Service]
ExecStart=$BIN/devid daemon run
Restart=on-failure

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
check "systemctl --user enable --now devid" systemctl --user enable --now devid
check "the daemon answers devid status" retry 20 "$BIN/devid" status
check "the daemon created its CA" retry 10 test -s "$CA"
check "the CA is name-constrained to .test" bash -c "openssl x509 -in '$CA' -noout -text | grep -A3 'Name Constraints' | grep -q 'DNS:.test'"
check "Devid's DNS answers $HOST on 127.0.0.1:53535" bash -c "dig +short -p 53535 @127.0.0.1 $HOST A | grep -qx 127.0.0.1"
check "Devid's DNS refuses other names" bash -c "dig -p 53535 @127.0.0.1 example.org A | grep -q 'status: REFUSED'"
journalctl --user -u devid --no-pager >"$OUT/devid-journal.txt" 2>&1

section "A site certificate from the Devid CA"
# Same fields as ca.rs ensure_site_cert (P-256 key, serverAuth, the host as SAN), made with openssl.
mkdir -p "$OUT/site"
openssl ecparam -name prime256v1 -genkey -noout -out "$OUT/site/key.pem"
openssl req -new -key "$OUT/site/key.pem" -subj "/CN=$HOST" -out "$OUT/site/csr.pem"
printf 'basicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:%s\nauthorityKeyIdentifier=keyid\n' "$HOST" >"$OUT/site/ext.cnf"
check "openssl signs $HOST with the Devid CA key" openssl x509 -req -in "$OUT/site/csr.pem" -CA "$CA" \
  -CAkey "$DEVID_HOME/ca/devid-ca.key" -set_serial "0x$(openssl rand -hex 16)" -days 397 \
  -extfile "$OUT/site/ext.cnf" -out "$OUT/site/cert.pem"

section "nginx and php-fpm from this spike's builds (question 3)"
mkdir -p "$OUT/www" "$OUT/nginx/run" "$OUT/nginx/log" "$OUT/nginx/tmp" "$OUT/fpm"
echo "$TEXT" >"$OUT/www/index.html"
echo '<?php echo "php ", PHP_VERSION, "\n";' >"$OUT/www/info.php"
cat >"$OUT/fpm/php-fpm.conf" <<EOF
[global]
pid = $OUT/fpm/php-fpm.pid
error_log = $OUT/fpm/error.log
daemonize = yes

[www]
listen = $OUT/fpm/php-fpm.sock
pm = ondemand
pm.max_children = 4
pm.process_idle_timeout = 10s
EOF
check "php-fpm starts (pm = ondemand)" "$BIN/php-fpm" -n -y "$OUT/fpm/php-fpm.conf"
cat >"$OUT/nginx/nginx.conf" <<EOF
worker_processes 1;
pid run/nginx.pid;
error_log log/error.log;
events { worker_connections 64; }
http {
  access_log off;
  default_type text/html;
  server {
    listen 127.0.0.1:8088;
    listen 127.0.0.1:8443 ssl;
    server_name $HOST;
    ssl_certificate $OUT/site/cert.pem;
    ssl_certificate_key $OUT/site/key.pem;
    root $OUT/www;
    index index.html;
    location ~ \.php\$ {
      fastcgi_pass unix:$OUT/fpm/php-fpm.sock;
      fastcgi_param SCRIPT_FILENAME \$document_root\$fastcgi_script_name;
      fastcgi_param REQUEST_METHOD \$request_method;
      fastcgi_param QUERY_STRING \$query_string;
    }
  }
}
EOF
check "nginx (static, musl) starts" "$BIN/nginx" -p "$OUT/nginx" -c "$OUT/nginx/nginx.conf"
check "nginx serves on 127.0.0.1:8088" serves -H "Host: $HOST" http://127.0.0.1:8088/
check "nginx passes PHP to php-fpm" bash -c "curl -fsS -H 'Host: $HOST' http://127.0.0.1:8088/info.php | grep -q '^php 8\.4'"

section "Ports 80 and 443 through nftables (question 5)"
sudo mkdir -p /etc/devid
sudo tee /etc/devid/ports.nft >/dev/null <<'EOF'
# Devid: 127.0.0.1 ports 80 and 443 go to the web server's high ports (the pf anchor's twin).
table inet devid
delete table inet devid
table inet devid {
  chain output {
    type nat hook output priority -100; policy accept;
    ip daddr 127.0.0.1 tcp dport 80 redirect to :8088
    ip daddr 127.0.0.1 tcp dport 443 redirect to :8443
  }
}
EOF
NFT="$(command -v nft)"
sudo tee /etc/systemd/system/devid-ports.service >/dev/null <<EOF
[Unit]
Description=Devid: redirect 127.0.0.1 ports 80 and 443 to Devid's web server
After=network-pre.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=$NFT -f /etc/devid/ports.nft
ExecStop=$NFT delete table inet devid

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
check "devid-ports.service enables and starts" sudo systemctl enable --now devid-ports
check "the devid table is loaded" bash -c "sudo nft list table inet devid | grep -q 'redirect to :8443'"
check "127.0.0.1:80 reaches nginx" serves -H "Host: $HOST" http://127.0.0.1/
note "nftables tables with ours: $(sudo nft list tables | tr '\n' ' ')"

section "DNS for .test through systemd-resolved (question 4)"
others=(example.org www.wikipedia.org "nx$RANDOM.example.com")
# A: a global drop-in.
sudo mkdir -p /etc/systemd/resolved.conf.d
printf '[Resolve]\nDNS=127.0.0.1:53535\nDomains=~test\n' | sudo tee /etc/systemd/resolved.conf.d/devid.conf >/dev/null
sudo systemctl restart systemd-resolved
sleep 1
resolvectl status >"$OUT/resolvectl-a.txt" 2>&1
check "A (global drop-in): resolvectl query $HOST" bash -c "resolvectl query $HOST | grep -q 127.0.0.1"
check "A: getent hosts $HOST" resolves_local "$HOST"
check "A: other names still resolve" getent hosts github.com
a=$(leaks a "${others[@]}")
note "A: queries outside .test that reached Devid's DNS: $a"
check "A: no name outside .test reaches Devid's DNS" test "$a" = 0
sudo rm /etc/systemd/resolved.conf.d/devid.conf
sudo systemctl restart systemd-resolved
# B: a link of its own, the way VPN clients add a DNS server for some domains.
sudo ip link add devid0 type dummy && sudo ip link set devid0 up
sudo resolvectl dns devid0 127.0.0.1:53535
sudo resolvectl domain devid0 '~test'
sudo resolvectl default-route devid0 false
sleep 1
resolvectl status >"$OUT/resolvectl-b.txt" 2>&1
check "B (own link): resolvectl query $HOST" bash -c "resolvectl query $HOST | grep -q 127.0.0.1"
check "B: getent hosts $HOST" resolves_local "$HOST"
check "B: other names still resolve" getent hosts github.com
b=$(leaks b "${others[@]}")
note "B: queries outside .test that reached Devid's DNS: $b"
check "B: no name outside .test reaches Devid's DNS" test "$b" = 0
if [ "$a" = 0 ]; then
  sudo ip link del devid0
  printf '[Resolve]\nDNS=127.0.0.1:53535\nDomains=~test\n' | sudo tee /etc/systemd/resolved.conf.d/devid.conf >/dev/null
  sudo systemctl restart systemd-resolved
  note "the rest of the run uses A"
else
  note "the rest of the run uses B"
fi
check "$HOST resolves for the rest of the run" retry 10 resolves_local "$HOST"

section "CA trust (question 6)"
refuse "before trust, curl rejects https://$HOST" serves "https://$HOST/"
sudo cp "$CA" /usr/local/share/ca-certificates/devid-ca.crt
check "update-ca-certificates adds the CA" bash -c "sudo update-ca-certificates 2>&1 | tee /dev/stderr | grep -q '1 added'"
check "curl trusts https://$HOST (443 -> 8443)" serves "https://$HOST/"
check "PHP trusts it with Devid's cacert.pem" bash -c "'$BIN/php' -n -d openssl.cafile='$DEVID_HOME/ca/cacert.pem' -r 'echo file_get_contents(\"https://$HOST/\");' | grep -qx '$TEXT'"
note "PHP's own OpenSSL locations: $("$BIN/php" -n -r 'echo json_encode(openssl_get_cert_locations());' 2>&1)"
for b in chrome firefox; do
  check "$b trusts https://$HOST with only the system store" "$PYTHON" "$HERE/browser.py" "$b" "https://$HOST/" "$TEXT"
done
mkdir -p "$HOME/.pki/nssdb"
[ -f "$HOME/.pki/nssdb/cert9.db" ] || certutil -d "$NSSDB" -N --empty-password
check "certutil adds the CA to ~/.pki/nssdb" certutil -d "$NSSDB" -A -t C,, -n "Devid CA" -i "$CA"
check "chrome trusts it after certutil" "$PYTHON" "$HERE/browser.py" chrome "https://$HOST/" "$TEXT"
sudo mkdir -p /etc/firefox/policies
echo '{"policies": {"Certificates": {"Install": ["/usr/local/share/ca-certificates/devid-ca.crt"]}}}' |
  sudo tee /etc/firefox/policies/policies.json >/dev/null
check "firefox trusts it with policies.json (Certificates.Install)" "$PYTHON" "$HERE/browser.py" firefox "https://$HOST/" "$TEXT"

section "The redirect with Docker and ufw, and across restarts (question 5)"
check "ufw enables" sudo ufw --force enable
check "https://$HOST with ufw on" serves "https://$HOST/"
sudo ufw --force disable >/dev/null
check "devid-ports.service restarts" sudo systemctl restart devid-ports
check "https://$HOST after the restart" serves "https://$HOST/"
sudo systemctl stop devid-ports
refuse "stopping the unit removes the redirect" serves "https://$HOST/"
sudo systemctl start devid-ports
check "starting it brings the redirect back" serves "https://$HOST/"

section "On-demand's connection check through /proc (question 8)"
setsid python3 -m http.server 18999 --bind 127.0.0.1 >/dev/null 2>&1 &
srv=$!
retry 10 curl -fsS -o /dev/null http://127.0.0.1:18999/
refuse "no connection open: established() says no" python3 "$HERE/established.py" "$srv" 18999
python3 -c 'import socket, time; s = socket.create_connection(("127.0.0.1", 18999)); time.sleep(5)' &
holder=$!
sleep 1
check "a held connection: established() says yes" python3 "$HERE/established.py" "$srv" 18999
start=$(date +%s%N)
python3 "$HERE/established.py" "$srv" 18999
note "one established() call took $(( ($(date +%s%N) - start) / 1000000 )) ms (python, $(ls -d /proc/[0-9]* | wc -l) processes)"
wait "$holder"
check "after it closes: established() says no again" retry 10 invert python3 "$HERE/established.py" "$srv" 18999
kill "$srv"

section "Idle memory (question 9)"
sleep 12   # php-fpm's ondemand workers from the PHP request above exit after 10 s
dpid=$(systemctl --user show -p MainPID --value devid)
note "devid daemon: $(mem "$dpid")"
nm=$(cat "$OUT/nginx/run/nginx.pid")
note "nginx master: $(mem "$nm")"
for w in $(pgrep -P "$nm"); do note "nginx worker: $(mem "$w")"; done
fm=$(cat "$OUT/fpm/php-fpm.pid")
note "php-fpm 8.4 master with $(pgrep -P "$fm" | wc -l) workers: $(mem "$fm")"

section "Uninstall puts everything back"
sudo systemctl disable --now devid-ports
sudo rm -f /etc/systemd/system/devid-ports.service /etc/devid/ports.nft
sudo rmdir /etc/devid
sudo systemctl daemon-reload
refuse "no devid table is left" sudo nft list table inet devid
sudo rm -f /etc/systemd/resolved.conf.d/devid.conf
sudo ip link del devid0 2>/dev/null
sudo systemctl restart systemd-resolved
refuse "$HOST no longer resolves" retry 3 resolves_local "$HOST"
sudo rm -f /usr/local/share/ca-certificates/devid-ca.crt
sudo update-ca-certificates --fresh >/dev/null 2>&1
ca_line="$(sed -n 2p "$CA")"
refuse "the CA is out of the system bundle" grep -qF "$ca_line" /etc/ssl/certs/ca-certificates.crt
certutil -d "$NSSDB" -D -n "Devid CA"
refuse "the CA is out of ~/.pki/nssdb" certutil -d "$NSSDB" -L -n "Devid CA"
sudo rm -f /etc/firefox/policies/policies.json
"$BIN/nginx" -p "$OUT/nginx" -c "$OUT/nginx/nginx.conf" -s stop
kill "$(cat "$OUT/fpm/php-fpm.pid")"
systemctl --user disable --now devid
refuse "the user unit no longer runs" systemctl --user is-active devid
sudo loginctl disable-linger "$USER"

echo
cat "$TABLE"
! grep -q '| FAIL |' "$TABLE"
