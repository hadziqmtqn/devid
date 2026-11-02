#!/bin/sh
# Devid installer: fetches the devid binary and hands over to `devid setup` (D-014).
# This script never uses sudo; system changes happen only inside `devid setup`, which lists them first.
#
#   sh install.sh                       latest release
#   DEVID_VERSION=v0.1.0 sh install.sh  specific release
#   DEVID_NO_SETUP=1 sh install.sh      install the binary only
set -eu

REPO="hadziqmtqn/devid"
BIN_DIR="${DEVID_HOME:-$HOME/.devid}/bin"
VERSION="${DEVID_VERSION:-latest}"

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

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

say "downloading $asset ($VERSION)"
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  # Private repo: release assets need an authenticated download.
  tag=""
  [ "$VERSION" = latest ] || tag="$VERSION"
  gh release download $tag -R "$REPO" -p "$asset" -p checksums.txt -D "$tmp" \
    || die "no release asset '$asset' found for $VERSION"
else
  # Public repo: plain HTTPS download.
  if [ "$VERSION" = latest ]; then base="https://github.com/$REPO/releases/latest/download"
  else base="https://github.com/$REPO/releases/download/$VERSION"; fi
  curl -fsSL "$base/$asset" -o "$tmp/$asset" || die "download failed (private repo? install and log in to gh)"
  curl -fsSL "$base/checksums.txt" -o "$tmp/checksums.txt" || die "checksums.txt missing"
fi

expected=$(awk -v f="$asset" '$2 == f || $2 == "*"f {print $1}' "$tmp/checksums.txt")
[ -n "$expected" ] || die "no checksum listed for $asset"
actual=$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')
[ "$expected" = "$actual" ] || die "checksum mismatch for $asset"

mkdir -p "$BIN_DIR"
chmod +x "$tmp/$asset"
mv -f "$tmp/$asset" "$BIN_DIR/devid"
say "installed $BIN_DIR/devid"

if [ -n "${DEVID_NO_SETUP:-}" ]; then
  say "skipping setup; run '$BIN_DIR/devid setup' when ready"
else
  exec "$BIN_DIR/devid" setup
fi
