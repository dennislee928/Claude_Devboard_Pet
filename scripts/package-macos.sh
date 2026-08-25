#!/usr/bin/env bash
# package-macos.sh — build DevPet for macOS and produce:
#   dist/DevPet-<ver>.dmg          drag-to-Applications disk image
#   dist/DevPet-<ver>.pkg          installer (binaries + hooks + autostart)
#   dist/DevPet-<ver>-macos.tar.gz portable archive (both editions)
#
#   ./package-macos.sh [VERSION] [--sign] [--skip-firmware]
#
# --sign creates a self-signed code-signing identity (see
# make-selfsigned-cert.sh) and signs the app, the binaries and the installer,
# so the bundle has a real publisher identity instead of being unsigned.
set -euo pipefail

VER="0.1.0"
SIGN=0
SKIP_FW=0
for a in "$@"; do
    case "$a" in
        --sign) SIGN=1 ;;
        --skip-firmware) SKIP_FW=1 ;;
        -*) echo "unknown option: $a" >&2; exit 2 ;;
        *) VER="$a" ;;
    esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
STAGE="$DIST/macos-stage"
APP="$DIST/DevPet.app"
BUNDLE_ID="dev.devpet.petd"
PUBLISHER="DevPet Project"

command -v cargo >/dev/null || { echo "cargo required"; exit 1; }
[[ "$(uname)" == "Darwin" ]] || { echo "this script must run on macOS"; exit 1; }

echo "== DevPet macOS package v$VER =="
(cd "$ROOT/pc" && cargo run --release -q -p asset-gen && cargo build --release -p petd -p pet-hook)

if [[ $SKIP_FW -eq 0 ]] && command -v pio >/dev/null 2>&1; then
    (cd "$ROOT/firmware" && pio run) || echo "warning: firmware build failed, continuing"
fi

REL="$ROOT/pc/target/release"
rm -rf "$APP" "$STAGE"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# ---- .app bundle: LSUIElement keeps it out of the Dock and the app switcher,
# which is what makes the pet feel like a background process --------------
cp "$REL/petd" "$REL/petd-lite" "$REL/pet-hook" "$APP/Contents/MacOS/"
cp "$ROOT/preview.png" "$APP/Contents/Resources/" 2>/dev/null || true
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>DevPet</string>
  <key>CFBundleDisplayName</key><string>DevPet</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleVersion</key><string>$VER</string>
  <key>CFBundleShortVersionString</key><string>$VER</string>
  <key>CFBundleExecutable</key><string>petd</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHumanReadableCopyright</key><string>Copyright (c) 2026 $PUBLISHER. MIT licensed.</string>
  <key>LSUIElement</key><true/>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
EOF

IDENTITY=""
if [[ $SIGN -eq 1 ]]; then
    IDENTITY="$("$ROOT/scripts/make-selfsigned-cert.sh")"
    echo "signing as: $IDENTITY (self-signed)"
    for f in "$APP/Contents/MacOS/"*; do
        codesign --force --timestamp=none --options runtime -s "$IDENTITY" "$f"
    done
    codesign --force --timestamp=none --options runtime -s "$IDENTITY" "$APP"
    codesign --verify --deep --strict --verbose=1 "$APP"
fi

# ---- portable tarball ----------------------------------------------------
mkdir -p "$STAGE/DevPet-$VER"
cp "$REL/petd" "$REL/petd-lite" "$REL/pet-hook" "$STAGE/DevPet-$VER/"
cp "$ROOT/scripts/setup.sh" "$ROOT/scripts/uninstall.sh" "$STAGE/DevPet-$VER/"
cp "$ROOT/README.md" "$STAGE/DevPet-$VER/"
mkdir -p "$STAGE/DevPet-$VER/hooks"
cp "$ROOT/hooks/settings.snippet.posix.json" "$STAGE/DevPet-$VER/hooks/"
FW_OUT="$ROOT/firmware/.pio/build/esp32doit-devkit-v1"
if [[ -f "$FW_OUT/firmware.bin" ]]; then
    mkdir -p "$STAGE/DevPet-$VER/firmware"
    cp "$FW_OUT/firmware.bin" "$FW_OUT/bootloader.bin" "$FW_OUT/partitions.bin" "$STAGE/DevPet-$VER/firmware/"
    cp "$ROOT/scripts/flash-firmware.sh" "$STAGE/DevPet-$VER/firmware/" 2>/dev/null || true
fi
tar -czf "$DIST/DevPet-$VER-macos.tar.gz" -C "$STAGE" "DevPet-$VER"
echo "tarball: $DIST/DevPet-$VER-macos.tar.gz"

# ---- .dmg ---------------------------------------------------------------
DMG="$DIST/DevPet-$VER.dmg"
DMG_SRC="$STAGE/dmg"
rm -f "$DMG"
mkdir -p "$DMG_SRC"
cp -R "$APP" "$DMG_SRC/"
ln -s /Applications "$DMG_SRC/Applications"
cp "$ROOT/README.md" "$DMG_SRC/"
hdiutil create -volname "DevPet $VER" -srcfolder "$DMG_SRC" -ov -quiet -format UDZO "$DMG"
[[ -n "$IDENTITY" ]] && codesign --force -s "$IDENTITY" "$DMG"
echo "dmg: $DMG"

# ---- .pkg ---------------------------------------------------------------
PKGROOT="$STAGE/pkgroot"
mkdir -p "$PKGROOT/Applications" "$PKGROOT/usr/local/bin"
cp -R "$APP" "$PKGROOT/Applications/"
for f in petd petd-lite pet-hook; do
    cp "$REL/$f" "$PKGROOT/usr/local/bin/$f"
    chmod 755 "$PKGROOT/usr/local/bin/$f"
done

SCRIPTS="$STAGE/pkgscripts"
mkdir -p "$SCRIPTS"
cat > "$SCRIPTS/postinstall" <<'POST'
#!/bin/bash
# Merge the Claude Code hooks for the installing user and start the pet.
set -u
USER_HOME=$(dscl . -read "/Users/$USER" NFSHomeDirectory 2>/dev/null | awk '{print $2}')
[ -z "${USER_HOME:-}" ] && USER_HOME="$HOME"
SETTINGS="$USER_HOME/.claude/settings.json"
mkdir -p "$(dirname "$SETTINGS")"
HOOK=/usr/local/bin/pet-hook /usr/bin/python3 - "$SETTINGS" <<'PY'
import json, os, sys
path = sys.argv[1]
cmd = f'"{os.environ["HOOK"]}"'
try:
    data = json.load(open(path))
except Exception:
    data = {}
def entry(m=False):
    e = {"hooks": [{"type": "command", "command": cmd}]}
    return {"matcher": "*", **e} if m else e
wanted = {"SessionStart": entry(), "UserPromptSubmit": entry(), "PreToolUse": entry(True),
          "PostToolUse": entry(True), "Notification": entry(), "SubagentStop": entry(), "Stop": entry()}
hooks = data.setdefault("hooks", {})
for name, e in wanted.items():
    lst = hooks.setdefault(name, [])
    if not any("pet-hook" in json.dumps(x) for x in lst):
        lst.append(e)
json.dump(data, open(path, "w"), indent=2)
open(path, "a").write("\n")
PY
sudo -u "$USER" /usr/local/bin/petd --install-autostart --display both || true
sudo -u "$USER" /usr/local/bin/petd --daemon --display both || true
exit 0
POST
chmod +x "$SCRIPTS/postinstall"

PKG_RAW="$STAGE/DevPet-component.pkg"
PKG="$DIST/DevPet-$VER.pkg"
rm -f "$PKG"
pkgbuild --root "$PKGROOT" --scripts "$SCRIPTS" \
    --identifier "$BUNDLE_ID.pkg" --version "$VER" --install-location / "$PKG_RAW" >/dev/null

cat > "$STAGE/distribution.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
  <title>DevPet $VER</title>
  <organization>$BUNDLE_ID</organization>
  <options customize="never" require-scripts="false" hostArchitectures="arm64,x86_64"/>
  <domains enable_localSystem="true"/>
  <pkg-ref id="$BUNDLE_ID.pkg"/>
  <choices-outline><line choice="default"/></choices-outline>
  <choice id="default"><pkg-ref id="$BUNDLE_ID.pkg"/></choice>
  <pkg-ref id="$BUNDLE_ID.pkg" version="$VER" onConclusion="none">DevPet-component.pkg</pkg-ref>
</installer-gui-script>
EOF
productbuild --distribution "$STAGE/distribution.xml" --package-path "$STAGE" "$PKG" >/dev/null
if [[ -n "$IDENTITY" ]]; then
    # productsign normally wants a "Developer ID Installer" certificate; a
    # self-signed code-signing one may be rejected, which is not fatal.
    if productsign --sign "$IDENTITY" "$PKG" "$PKG.signed" 2>/dev/null; then
        mv "$PKG.signed" "$PKG"
        echo "pkg signed with the self-signed identity"
    else
        echo "note: productsign rejected the self-signed identity; the .pkg payload is still signed"
    fi
fi
echo "pkg: $PKG"

rm -rf "$STAGE"
ls -lh "$DIST" | sed -n '2,20p'
