#!/bin/bash
# Spike: build nginx for macOS with its dependencies linked statically (risk 4).
# Reuses the OpenSSL 3.6 and zlib static libs from the PHP build cache; PCRE2 is built by nginx itself.
set -euo pipefail

NGINX_VER="${NGINX_VER:-1.30.6}"   # stable branch
PCRE2_VER="${PCRE2_VER:-10.49}"
MAIN=/Users/meowpush/Projects/devid/spike
BR="$MAIN/php/buildroot"           # static libssl.a, libcrypto.a, libz.a + headers
WORK="$MAIN/webservers/nginx-build"
OUT="$MAIN/webservers/out/nginx"

# Same Homebrew isolation as spike/php/build.sh.
unset PKG_CONFIG_PATH
export SDKROOT="$(xcrun --show-sdk-path)"

mkdir -p "$WORK" "$OUT"
cd "$WORK"
[ -f "nginx-$NGINX_VER.tar.gz" ] || curl -fsSLO "https://nginx.org/download/nginx-$NGINX_VER.tar.gz"
[ -f "pcre2-$PCRE2_VER.tar.gz" ] || curl -fsSLO "https://github.com/PCRE2Project/pcre2/releases/download/pcre2-$PCRE2_VER/pcre2-$PCRE2_VER.tar.gz"
rm -rf "nginx-$NGINX_VER" "pcre2-$PCRE2_VER"
tar xzf "nginx-$NGINX_VER.tar.gz"
tar xzf "pcre2-$PCRE2_VER.tar.gz"

cd "nginx-$NGINX_VER"
start=$(date +%s)
# Runtime paths are relative to -p <prefix>, so Devid can run it from any folder.
./configure \
  --prefix=. --sbin-path=nginx --conf-path=conf/nginx.conf \
  --pid-path=run/nginx.pid --lock-path=run/nginx.lock \
  --error-log-path=log/error.log --http-log-path=log/access.log \
  --http-client-body-temp-path=tmp/body --http-proxy-temp-path=tmp/proxy \
  --http-fastcgi-temp-path=tmp/fastcgi --without-http_uwsgi_module --without-http_scgi_module \
  --with-http_ssl_module --with-http_v2_module --with-http_realip_module --with-http_gzip_static_module \
  --with-pcre="../pcre2-$PCRE2_VER" --with-pcre-jit \
  --with-cc-opt="-I$BR/include -Os" \
  --with-ld-opt="-L$BR/lib -lz" > ../configure.log
# nginx falls back to a hard-coded -L/usr/local/lib (Homebrew) when -lssl is not found; refuse that.
grep -q 'using system OpenSSL library' ../configure.log && ! grep -q 'OpenSSL library in /usr/local' ../configure.log
make -j4 > ../make.log 2>&1
cp objs/nginx "$OUT/nginx.new" && mv -f "$OUT/nginx.new" "$OUT/nginx"
strip "$OUT/nginx"

echo "== build took $(( $(date +%s) - start ))s"
ls -lh "$OUT/nginx"
"$OUT/nginx" -V 2>&1 | head -3
echo "== linked libs (must be /usr/lib or /System only)"
otool -L "$OUT/nginx"
! otool -L "$OUT/nginx" | grep -q /usr/local || { echo "FAIL: links Homebrew libraries" >&2; exit 1; }
