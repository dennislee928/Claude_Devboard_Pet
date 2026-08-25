#!/usr/bin/env bash
# package-linux.sh — portable Linux tarball with both editions.
#   ./package-linux.sh [VERSION]
set -euo pipefail
VER="${1:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
STAGE="$DIST/DevPet-$VER"

command -v cargo >/dev/null || { echo "cargo required"; exit 1; }
(cd "$ROOT/pc" && cargo run --release -q -p asset-gen && cargo build --release -p petd -p pet-hook)

rm -rf "$STAGE"
mkdir -p "$STAGE/hooks"
cp "$ROOT/pc/target/release/petd" "$ROOT/pc/target/release/petd-lite" "$ROOT/pc/target/release/pet-hook" "$STAGE/"
cp "$ROOT/scripts/setup.sh" "$ROOT/scripts/uninstall.sh" "$ROOT/README.md" "$STAGE/"
cp "$ROOT/hooks/settings.snippet.posix.json" "$STAGE/hooks/"
mkdir -p "$DIST"
tar -czf "$DIST/DevPet-$VER-linux-x86_64.tar.gz" -C "$DIST" "DevPet-$VER"
rm -rf "$STAGE"
echo "tarball: $DIST/DevPet-$VER-linux-x86_64.tar.gz"
