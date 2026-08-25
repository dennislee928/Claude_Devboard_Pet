#!/usr/bin/env bash
# uninstall.sh — remove DevPet from macOS / Linux.
#   ./uninstall.sh            keep growth + config
#   ./uninstall.sh --purge    remove them too
set -euo pipefail
PURGE=0
[[ "${1:-}" == "--purge" ]] && PURGE=1

PREFIX="${DEVPET_PREFIX:-$HOME/.local/share/devpet}"
"$PREFIX/bin/petd" --uninstall-autostart 2>/dev/null || true
pkill -x petd 2>/dev/null || true
pkill -x petd-lite 2>/dev/null || true

rm -rf "$PREFIX"
rm -f "$HOME/Library/LaunchAgents/dev.devpet.petd.plist" 2>/dev/null || true
rm -f "$HOME/.config/systemd/user/devpet.service" 2>/dev/null || true
rm -rf /Applications/DevPet.app 2>/dev/null || true

SETTINGS="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/settings.json"
if [[ -f "$SETTINGS" ]]; then
    python3 - "$SETTINGS" <<'PY'
import json, sys
path = sys.argv[1]
try:
    data = json.load(open(path))
except Exception:
    sys.exit(0)
hooks = data.get("hooks", {})
for name in list(hooks):
    kept = [e for e in hooks[name] if "pet-hook" not in json.dumps(e)]
    if kept:
        hooks[name] = kept
    else:
        del hooks[name]
if not hooks:
    data.pop("hooks", None)
json.dump(data, open(path, "w"), indent=2)
open(path, "a").write("\n")
print(f"hooks: pet-hook entries removed from {path}")
PY
fi

if [[ $PURGE -eq 1 ]]; then
    rm -rf "$HOME/Library/Application Support/devpet" "${XDG_CONFIG_HOME:-$HOME/.config}/devpet"
    echo "growth + config removed"
else
    echo "growth + config kept (use --purge to remove)"
fi
echo "DevPet uninstalled."
