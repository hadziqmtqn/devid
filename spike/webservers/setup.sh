#!/bin/bash
# Spike: serve the real erdbpro-saas app through Caddy, nginx and Apache with the same
# php-fpm 8.4 pool and a Devid-CA certificate, to compare the three adapters (D-005).
#   Caddy :9443   nginx :9444   Apache (/usr/sbin/httpd) :9445
# Needs spike/multi/setup.sh state (CA + php-fpm 8.4 config). No sudo; nothing system-wide.
set -euo pipefail

MAIN=/Users/meowpush/Projects/devid/spike
MULTI="$MAIN/runtime/multi"
STATE="${STATE:-$MAIN/webservers/run}"
APP=/Users/meowpush/Projects/erdbpro-saas
SITE=erdbpro-saas.test
SOCK="$MULTI/run/php84.sock"
OPENSSL="$MAIN/php/buildroot/bin/openssl"
NGINX_SRC=$(ls -d "$MAIN"/webservers/nginx-build/nginx-*/ | head -1)

mkdir -p "$STATE"/{certs,caddy,nginx/{conf,run,log,tmp},apache/{run,log}}
cd "$STATE"

"$OPENSSL" req -new -nodes -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout "certs/$SITE.key" -out "certs/$SITE.csr" -subj "/CN=$SITE" 2>/dev/null
"$OPENSSL" x509 -req -in "certs/$SITE.csr" -CA "$MULTI/ca/devid-ca.crt" -CAkey "$MULTI/ca/devid-ca.key" \
  -CAcreateserial -out "certs/$SITE.crt" -days 825 2>/dev/null \
  -extfile <(printf 'subjectAltName=DNS:%s\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' "$SITE")
chmod 600 certs/*.key
CRT="$STATE/certs/$SITE.crt"; KEY="$STATE/certs/$SITE.key"

cat > caddy/Caddyfile <<EOF
{
	admin off
	auto_https off
	storage file_system $STATE/caddy/data
}

https://$SITE:9443 {
	tls $CRT $KEY
	root * $APP/public
	encode
	php_fastcgi unix/$SOCK
	file_server
}
EOF

# nginx: the standard Laravel server block.
cp "$NGINX_SRC/conf/mime.types" "$NGINX_SRC/conf/fastcgi_params" nginx/conf/
cat > nginx/conf/nginx.conf <<EOF
worker_processes 1;
error_log log/error.log warn;
events { worker_connections 256; }
http {
    include mime.types;
    default_type application/octet-stream;
    access_log off;
    sendfile on;
    gzip on;
    server {
        listen 9444 ssl;
        http2 on;
        server_name $SITE;
        ssl_certificate $CRT;
        ssl_certificate_key $KEY;
        root $APP/public;
        index index.php;
        charset utf-8;
        client_max_body_size 64m;
        location / { try_files \$uri \$uri/ /index.php?\$query_string; }
        location ~ \.php\$ {
            fastcgi_pass unix:$SOCK;
            fastcgi_param SCRIPT_FILENAME \$realpath_root\$fastcgi_script_name;
            include fastcgi_params;
            # Laravel/Inertia responses (cookies, Vite preload Link headers) exceed nginx's 4k default.
            fastcgi_buffer_size 32k;
            fastcgi_buffers 16 16k;
            fastcgi_hide_header X-Powered-By;
            add_header X-Devid-Server nginx;
        }
        location ~ /\.(?!well-known).* { deny all; }
    }
}
EOF

# Apache: the macOS system httpd, run as the user with its own config (never /etc/apache2).
cat > apache/httpd.conf <<EOF
ServerRoot "/usr"
Listen 9445
ServerName $SITE
PidFile "$STATE/apache/run/httpd.pid"
DefaultRuntimeDir "$STATE/apache/run"
Mutex file:$STATE/apache/run default
ErrorLog "$STATE/apache/log/error.log"
LogLevel warn
LoadModule mpm_event_module libexec/apache2/mod_mpm_event.so
LoadModule authz_core_module libexec/apache2/mod_authz_core.so
LoadModule unixd_module libexec/apache2/mod_unixd.so
LoadModule mime_module libexec/apache2/mod_mime.so
LoadModule dir_module libexec/apache2/mod_dir.so
LoadModule rewrite_module libexec/apache2/mod_rewrite.so
LoadModule setenvif_module libexec/apache2/mod_setenvif.so
LoadModule headers_module libexec/apache2/mod_headers.so
LoadModule deflate_module libexec/apache2/mod_deflate.so
LoadModule filter_module libexec/apache2/mod_filter.so
LoadModule proxy_module libexec/apache2/mod_proxy.so
LoadModule proxy_fcgi_module libexec/apache2/mod_proxy_fcgi.so
LoadModule socache_shmcb_module libexec/apache2/mod_socache_shmcb.so
LoadModule ssl_module libexec/apache2/mod_ssl.so
LoadModule http2_module libexec/apache2/mod_http2.so
TypesConfig /private/etc/apache2/mime.types
StartServers 1
ServerLimit 1
ThreadsPerChild 25
MaxRequestWorkers 25
<VirtualHost *:9445>
    Protocols h2 http/1.1
    SSLEngine on
    SSLCertificateFile "$CRT"
    SSLCertificateKeyFile "$KEY"
    DocumentRoot "$APP/public"
    DirectoryIndex index.php
    Header set X-Devid-Server apache
    <Directory "$APP/public">
        AllowOverride All
        Require all granted
    </Directory>
    <FilesMatch "\.php\$">
        SetHandler "proxy:unix:$SOCK|fcgi://localhost"
    </FilesMatch>
</VirtualHost>
EOF

"$MAIN/runtime/caddy/caddy" validate --config caddy/Caddyfile --adapter caddyfile 2>&1 | tail -1
[ -x "$MAIN/webservers/out/nginx/nginx" ] && "$MAIN/webservers/out/nginx/nginx" -p "$STATE/nginx" -t 2>&1 | tail -1
/usr/sbin/httpd -f "$STATE/apache/httpd.conf" -t 2>&1 | tail -1
echo "state: $STATE"
