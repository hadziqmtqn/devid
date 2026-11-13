#!/bin/sh
# Devid installer: fetches the devid binary and hands over to `devid setup` (D-014).
# This script never uses sudo; system changes happen only inside `devid setup`, which lists them first.
#
#   curl -fsSL https://github.com/hadziqmtqn/devid-binaries/releases/download/devid-latest/install.sh | sh
#   DEVID_VERSION=0.1.0 sh install.sh   specific release
#   DEVID_NO_SETUP=1 sh install.sh      install the binary only
#   ... | sh -s -- --php 8.3 --server caddy   pass options to `devid setup`
set -eu

# Binaries live in the devid-binaries repo (D-016); the source is in hadziqmtqn/devid (D-043).
REPO="hadziqmtqn/devid-binaries"
BIN_DIR="${DEVID_HOME:-$HOME/.devid}/bin"
VERSION="${DEVID_VERSION:-latest}"
BASE="${DEVID_RELEASE_BASE:-https://github.com/$REPO/releases/download}"

say() { printf 'devid: %s\n' "$*"; }
die() { printf 'devid: error: %s\n' "$*" >&2; exit 1; }

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin) os=darwin ;;
  *) die "unsupported OS '$os'; v1 supports macOS only" ;;
esac
case "$arch" in
  x86_64 | arm64) ;;
  *) die "unsupported CPU '$arch'" ;;
esac
asset="devid-$os-$arch"
case "$VERSION" in
  latest) tag="devid-latest" ;;
  *) tag="devid-v${VERSION#v}" ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

say "downloading $asset ($tag)"
curl -fsSL --proto '=https,file' -o "$tmp/$asset" "$BASE/$tag/$asset" \
  || die "no $asset in release $tag (arm64 builds come later, see D-010)"
curl -fsSL --proto '=https,file' -o "$tmp/checksums.txt" "$BASE/$tag/checksums.txt" || die "checksums.txt missing"

expected=$(awk -v f="$asset" '$2 == f || $2 == "*"f {print $1}' "$tmp/checksums.txt")
[ -n "$expected" ] || die "no checksum listed for $asset"
actual=$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')
[ "$expected" = "$actual" ] || die "checksum mismatch for $asset"

mkdir -p "$BIN_DIR"
chmod +x "$tmp/$asset"
mv -f "$tmp/$asset" "$BIN_DIR/devid"
say "installed $BIN_DIR/devid ($("$BIN_DIR/devid" --version))"

if [ -n "${DEVID_NO_SETUP:-}" ]; then
  say "skipping setup; run '$BIN_DIR/devid setup' when ready"
else
  exec "$BIN_DIR/devid" setup "$@"
fi
