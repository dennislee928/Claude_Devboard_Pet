#!/usr/bin/env bash
# package-macos.sh — build DevPet's PC side into a .app + .dmg (and the firmware
# images). MUST run on macOS (uses hdiutil); provided for future cross-platform
# packaging — the current daemon targets Windows (%APPDATA%, COM ports), so
# expect minor porting work (config paths, /dev/cu.* serial names) first.
set -euo pipefail
VER="${1:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
APP="$DIST/DevPet.app"

command -v cargo >/dev/null || { echo "cargo required"; exit 1; }
[[ "$(uname)" == "Darwin" ]] || { echo "this script must run on macOS"; exit 1; }

(cd "$ROOT/pc" && cargo run --release -q -p asset-gen && cargo build --release -p petd -p pet-hook)

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/pc/target/release/petd" "$APP/Contents/MacOS/"
cp "$ROOT/pc/target/release/pet-hook" "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>DevPet</string>
  <key>CFBundleIdentifier</key><string>dev.devpet.petd</string>
  <key>CFBundleVersion</key><string>$VER</string>
  <key>CFBundleExecutable</key><string>petd</string>
  <key>LSUIElement</key><true/>
</dict></plist>
EOF

DMG="$DIST/DevPet-$VER.dmg"
rm -f "$DMG"
hdiutil create -volname "DevPet $VER" -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "dmg: $DMG"
