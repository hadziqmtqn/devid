#!/bin/bash
# Phase 12 L1 (docs/roadmap/phase-12.md), question 3: nginx for Linux, fully static with musl,
# OpenSSL and zlib from static-php-cli (its default Linux target) and PCRE2 from source.
#   spike/linux/build-nginx.sh   -> out/ (nginx, BUILDINFO)
# The configure flags follow devid-binaries' scripts/build-nginx.sh (macOS).
set -euo pipefail

ROOT="$(pwd)"
WORK="${WORK:-$ROOT/work}"
OUT="${OUT:-$ROOT/out}"
SPC_VERSION=2.8.5
SPC_SHA256=523ba4279c54c7a377156c0dd3a36adf92ee64b01e9a7f5e9e2ec084b8e458e5
NGINX_VER=1.30.7   # runtime-latest on 2026-11-23
PCRE2_VER=10.49
BR="$WORK/buildroot"
mkdir -p "$WORK/nginx" "$OUT"
cd "$WORK"
if [ ! -x spc ]; then
  curl -fsSL -o spc.tar.gz "https://github.com/crazywhalecc/static-php-cli/releases/download/$SPC_VERSION/spc-linux-x86_64.tar.gz"
  echo "$SPC_SHA256  spc.tar.gz" | sha256sum -c -
  tar xzf spc.tar.gz spc && rm spc.tar.gz
fi
unset SPC_TARGET   # musl, spc's default on Linux
echo "::group::spc libs"
./spc doctor --auto-fix
./spc download openssl,zlib --retry=3
./spc build:libs openssl,zlib
echo "::endgroup::"

CC="$(command -v x86_64-linux-musl-gcc || echo /usr/local/musl/bin/x86_64-linux-musl-gcc)"
[ -x "$CC" ] || { echo "FAIL: no musl gcc after spc doctor" >&2; exit 1; }

cd "$WORK/nginx"
curl -fsSLO "https://nginx.org/download/nginx-$NGINX_VER.tar.gz"
curl -fsSLO "https://github.com/PCRE2Project/pcre2/releases/download/pcre2-$PCRE2_VER/pcre2-$PCRE2_VER.tar.gz"
tar xzf "nginx-$NGINX_VER.tar.gz" && tar xzf "pcre2-$PCRE2_VER.tar.gz"
cd "nginx-$NGINX_VER"
echo "::group::nginx configure and make"
./configure --with-cc="$CC" \
  --prefix=. --sbin-path=nginx --conf-path=conf/nginx.conf \
  --pid-path=run/nginx.pid --lock-path=run/nginx.lock \
  --error-log-path=log/error.log --http-log-path=log/access.log \
  --http-client-body-temp-path=tmp/body --http-proxy-temp-path=tmp/proxy \
  --http-fastcgi-temp-path=tmp/fastcgi --without-http_uwsgi_module --without-http_scgi_module \
  --with-http_ssl_module --with-http_v2_module --with-http_realip_module --with-http_gzip_static_module \
  --with-pcre="../pcre2-$PCRE2_VER" --with-pcre-jit \
  --with-cc-opt="-I$BR/include -Os" \
  --with-ld-opt="-static -L$BR/lib -Wl,--start-group -lssl -lcrypto -lz -Wl,--end-group" ||
  { tail -40 objs/autoconf.err; ls "$BR/lib"; exit 1; }
# GNU ld reads static archives in order, unlike Apple's: OpenSSL needs zlib after it, hence the group.
make -j"$(nproc)"
echo "::endgroup::"
cp objs/nginx "$OUT/nginx"
strip "$OUT/nginx"
cd "$OUT"
{
  echo "nginx $NGINX_VER, pcre2 $PCRE2_VER, musl ($CC)"
  file nginx
  ldd nginx 2>&1 || true
  ./nginx -V 2>&1 | grep -i 'built with'
} > BUILDINFO-nginx
cat BUILDINFO-nginx
