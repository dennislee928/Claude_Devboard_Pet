#!/bin/bash
# Install DevPet.command — double-click this to install the DevPet .pkg sitting
# next to it.
#
# Why this exists: Apple only lets `productsign` use a Developer ID Installer
# certificate issued by Apple, so a self-signed .pkg cannot be signed and
# Gatekeeper refuses to open it on a double-click ("no usable signature").
# The `installer` command-line tool is not subject to that check, so this
# script clears the download quarantine flag and installs through it. The
# binaries inside the package *are* signed — see DevPet-selfsigned.cer.
set -euo pipefail
cd "$(dirname "$0")"

PKG=$(ls DevPet-*.pkg 2>/dev/null | head -1 || true)
if [[ -z "$PKG" ]]; then
    echo "No DevPet-*.pkg found next to this script." >&2
    read -r -p "Press Return to close." _
    exit 1
fi

echo "== Installing $PKG =="
xattr -d com.apple.quarantine "$PKG" 2>/dev/null || true
sudo installer -pkg "$PKG" -target /
echo
echo "Done. DevPet is running in the background and starts at every login."
echo "Right-click the pet for its menu, or open the Claude Code status panel from there."
read -r -p "Press Return to close." _
