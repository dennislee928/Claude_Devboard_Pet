#!/usr/bin/env bash
# setup.sh — install DevPet on macOS / Linux from an unpacked release (or from
# a source checkout with --build). Installs both editions, merges the Claude
# Code hooks, and starts the pet silently in the background.
#
#   ./setup.sh                     install from the files next to this script
#   ./setup.sh --build             cargo build --release first
#   ./setup.sh --edition firmware  autostart petd-lite (board is the brain)
#   ./setup.sh --no-autostart      don't start at login
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${DEVPET_PREFIX:-$HOME/.local/share/devpet}"
BIN="$PREFIX/bin"
EDITION=standalone
BUILD=0
AUTOSTART=1

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build) BUILD=1 ;;
        --edition) EDITION="${2:?}"; shift ;;
        --no-autostart) AUTOSTART=0 ;;
        -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done
[[ "$EDITION" == standalone || "$EDITION" == firmware ]] || { echo "--edition must be standalone or firmware" >&2; exit 2; }

echo "== DevPet setup ($EDITION edition) =="

SRC="$HERE"
if [[ $BUILD -eq 1 ]]; then
    ROOT="$(cd "$HERE/.." && pwd)"
    (cd "$ROOT/pc" && cargo run --release -q -p asset-gen && cargo build --release)
    SRC="$ROOT/pc/target/release"
fi

# stop a running pet so its binary can be replaced
pkill -x petd 2>/dev/null || true
pkill -x petd-lite 2>/dev/null || true
sleep 1

mkdir -p "$BIN"
for f in petd petd-lite pet-hook; do
    if [[ -f "$SRC/$f" ]]; then
        install -m 755 "$SRC/$f" "$BIN/$f"
    else
        echo "warning: $f not found in $SRC" >&2
    fi
done
echo "binaries -> $BIN"

# ---- Claude Code hooks ----------------------------------------------------
SETTINGS="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/settings.json"
mkdir -p "$(dirname "$SETTINGS")"
HOOK="$BIN/pet-hook" python3 - "$SETTINGS" <<'PY'
import json, os, sys
path = sys.argv[1]
hook = os.environ["HOOK"]
cmd = f'"{hook}"'
try:
    with open(path) as f:
        data = json.load(f)
except (OSError, json.JSONDecodeError):
    data = {}

def entry(matcher=False):
    e = {"hooks": [{"type": "command", "command": cmd}]}
    return {"matcher": "*", **e} if matcher else e

wanted = {
    "SessionStart": entry(), "UserPromptSubmit": entry(),
    "PreToolUse": entry(True), "PostToolUse": entry(True),
    "Notification": entry(), "SubagentStop": entry(), "Stop": entry(),
}
hooks = data.setdefault("hooks", {})
added = 0
for name, e in wanted.items():
    lst = hooks.setdefault(name, [])
    if any("pet-hook" in json.dumps(x) for x in lst):
        continue
    lst.append(e)
    added += 1
with open(path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
print(f"hooks: {added} added, {len(wanted) - added} already present -> {path}")
PY

# ---- start silently at login ---------------------------------------------
EXE="$BIN/petd"
[[ "$EDITION" == firmware ]] && EXE="$BIN/petd-lite"
if [[ $AUTOSTART -eq 1 ]]; then
    "$EXE" --install-autostart --display both
else
    echo "autostart: skipped (run '$EXE --install-autostart' later)"
fi

# ---- run it now, detached and silent -------------------------------------
"$EXE" --daemon --display both
echo "DevPet is running in the background. Logs: ${XDG_CONFIG_HOME:-$HOME/.config}/devpet/petd.log (macOS: ~/Library/Application Support/devpet/petd.log)"
echo "Right-click the pet for the menu; enable the status panel to see live Claude Code sessions."
