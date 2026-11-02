#!/bin/bash
# Spike: HTTPS for *.test through a Devid CA, and two PHP versions side by side.
# Generates everything into $STATE (gitignored); trusts nothing system-wide and needs no sudo.
#   blog.test -> PHP 8.5 (spike/laravel)    api.test -> PHP 8.4 (spike/laravel83)
set -euo pipefail

MAIN=/Users/meowpush/Projects/devid/spike          # gitignored caches and test apps live here
STATE="${STATE:-$MAIN/runtime/multi}"
OPENSSL="$MAIN/php/buildroot/bin/openssl"           # OpenSSL 3.6 from the PHP build
PORT=8443

mkdir -p "$STATE"/{ca,certs,etc/conf.d,run,log}
cd "$STATE"

# Devid root CA (Devid generates this itself in Phase 2; openssl stands in for the spike).
if [ ! -f ca/devid-ca.crt ]; then
  "$OPENSSL" req -x509 -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout ca/devid-ca.key -out ca/devid-ca.crt -days 3650 \
    -subj "/CN=Devid Spike Local CA" \
    -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign"
fi

# One leaf certificate per site, signed by the CA; 825 days is the macOS maximum for TLS certs.
for site in blog api; do
  "$OPENSSL" req -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -keyout "certs/$site.test.key" -out "certs/$site.test.csr" -subj "/CN=$site.test"
  "$OPENSSL" x509 -req -in "certs/$site.test.csr" -CA ca/devid-ca.crt -CAkey ca/devid-ca.key \
    -CAcreateserial -out "certs/$site.test.crt" -days 825 \
    -extfile <(printf 'subjectAltName=DNS:%s.test\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' "$site")
done

# D-008: PHP trusts the system bundle plus the Devid CA.
cat /etc/ssl/cert.pem ca/devid-ca.crt > ca/cacert.pem

cat > etc/php.ini <<EOF
memory_limit = 512M
display_errors = On
opcache.enable = 1
opcache.jit = off
openssl.cafile = $STATE/ca/cacert.pem
curl.cainfo = $STATE/ca/cacert.pem
EOF

for v in 8.5 8.4; do
  n=${v/./}
  cat > "etc/php$n-fpm.conf" <<EOF
[global]
pid = $STATE/run/php$n-fpm.pid
error_log = $STATE/log/php$n-fpm.log
daemonize = no

[devid-$n]
listen = $STATE/run/php$n.sock
pm = ondemand
pm.max_children = 4
pm.process_idle_timeout = 10s
catch_workers_output = yes
EOF
done

cat > etc/Caddyfile <<EOF
{
	admin off
	auto_https off
	storage file_system $STATE/run/caddy-data
}

https://blog.test:$PORT {
	tls $STATE/certs/blog.test.crt $STATE/certs/blog.test.key
	root * $MAIN/laravel/public
	php_fastcgi unix/$STATE/run/php85.sock
	file_server
}

https://api.test:$PORT {
	tls $STATE/certs/api.test.crt $STATE/certs/api.test.key
	root * $MAIN/laravel83/public
	php_fastcgi unix/$STATE/run/php84.sock
	file_server
}
EOF

"$MAIN/runtime/caddy/caddy" validate --config etc/Caddyfile --adapter caddyfile 2>&1 | tail -1
echo "state: $STATE"
