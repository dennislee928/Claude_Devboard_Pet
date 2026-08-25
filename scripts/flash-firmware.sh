#!/usr/bin/env bash
# flash-firmware.sh — flash the DevPet firmware images onto an ESP32 dev board
# from macOS / Linux. Needs esptool (pip install esptool).
#   ./flash-firmware.sh [/dev/cu.usbserial-XXXX]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PORT="${1:-}"
if [[ -z "$PORT" ]]; then
    PORT=$(ls /dev/cu.usbserial* /dev/cu.wchusbserial* /dev/ttyUSB* /dev/ttyACM* 2>/dev/null | head -1 || true)
fi
[[ -n "$PORT" ]] || { echo "no serial port found; pass one explicitly" >&2; exit 1; }
command -v esptool.py >/dev/null || command -v esptool >/dev/null || { echo "esptool required: pip install esptool" >&2; exit 1; }
ESPTOOL=$(command -v esptool.py || command -v esptool)

echo "flashing $PORT ..."
"$ESPTOOL" --chip esp32 --port "$PORT" --baud 460800 write_flash -z \
    0x1000 "$HERE/bootloader.bin" \
    0x8000 "$HERE/partitions.bin" \
    0x10000 "$HERE/firmware.bin"
echo "done — the pet should appear on the screen."
