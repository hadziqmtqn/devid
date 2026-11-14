#!/bin/bash
# Builds Devid.app (Phase 7 G4): the desktop app as a plain macOS bundle, ad-hoc signed (D-010),
# plus the zip the release publishes. The same script runs on the owner's Mac and in release.yml.
# Usage: scripts/bundle-app.sh [out-dir]   (default: dist)
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=${1:-dist}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TARGET=${CARGO_TARGET_DIR:-target}

# custom-protocol: Tauri's production mode (the bundle supplies the icon; no dev-only behaviour).
cargo build --release -p devid-gui --features custom-protocol

mkdir -p "$OUT"
APP="$OUT/Devid.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/devid-gui" "$APP/Contents/MacOS/devid-gui"
cp crates/gui/icons/icon.icns "$APP/Contents/Resources/icon.icns"
# LSUIElement: the app starts in the menu bar only (no Dock icon even for a moment, e.g. at login)
# and becomes a regular app while its window is open (D-030).
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>CFBundleDisplayName</key><string>Devid</string>
    <key>CFBundleExecutable</key><string>devid-gui</string>
    <key>CFBundleIconFile</key><string>icon</string>
    <key>CFBundleIdentifier</key><string>dev.devid.app</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleName</key><string>Devid</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
plutil -lint "$APP/Contents/Info.plist" >/dev/null
codesign --force --sign - "$APP"
codesign --verify --strict "$APP"
ZIP="Devid-darwin-$(uname -m).zip"   # built natively: x86_64 or arm64
rm -f "$OUT/$ZIP"
(cd "$OUT" && ditto -c -k --keepParent Devid.app "$ZIP")
echo "built $APP ($VERSION) and $OUT/$ZIP"
