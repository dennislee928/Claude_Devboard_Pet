# DevPet — 智能桌寵小屏幕 (Smart Desk-Pet Status Display)

A pixel-art desk pet that mirrors your AI coding assistant's work state in real
time. **Clawd** the crab 🦀 (or **Beemo**, a BMO-style handheld) lives either on
an ESP32-driven mini screen on your desk, in a tiny always-on-top window on
your monitor, or both — and it **grows** as you work.

- **13 states**: idle, coding, thinking, searching, testing, building,
  debugging, error, success, waiting, notify, celebrating, sleep
- **2 characters** + an egg form, all generated from one pixel-art source
- **Growth**: XP from real usage; Lv1 Egg → Lv2 Baby → Lv3 Junior (tie /
  cartridge) → Lv4 Senior (glasses / headphones) → Lv5 Legend (crown 👑),
  with the pet physically rendered bigger at higher levels
- **Dual display**: `--display board`, `--display desktop`, or `both`

```
Claude Code ──hooks──▶ pet-hook.exe ──HTTP──▶ petd.exe ──USB serial──▶ ESP32 + ST7789
                                                  │
                                                  └──▶ desktop pet window (egui)
```

## Hardware

| Part | Detail |
|---|---|
| Board | ESP-WROOM-32 DevKit V1 (this repo's board: ESP32-D0WD-V3, 4MB flash, CH9102 USB on **COM9**) |
| Screen | ST7789 240x240 1.3"/1.54" IPS, SPI |

Wiring (DevKit V1 → screen):

| Screen pin | ESP32 pin |
|---|---|
| VCC | 3V3 |
| GND | GND |
| SCL / SCK | GPIO18 |
| SDA / MOSI | GPIO23 |
| RES | GPIO4 |
| DC | GPIO2 |
| CS | GPIO5 |
| BLK / BL | 3V3 (always on) |

> If your ST7789 module has **no CS pin**, add `-DTFT_SPI_MODE3=1` to
> `firmware/platformio.ini` build_flags and tie nothing to GPIO5.

## Build & install

Prereqs: Python 3 + `pip install platformio`, Rust (`rustup`), and for
firmware upload the board on COM9 (auto-detected otherwise).

```powershell
# 1. Generate sprites (writes firmware/src/sprites_gen.h + pc/petd assets)
cd pc; cargo run -p asset-gen

# 2. Firmware: build + flash (from firmware/)
cd ..\firmware
python -m platformio run -t upload

# 3. PC side: build release + install to %LOCALAPPDATA%\devpet\bin
cd ..
.\install.ps1

# 4. Claude Code hooks: merge hooks\settings.snippet.json into
#    %USERPROFILE%\.claude\settings.json  (then restart Claude Code)

# 5. Run the daemon
& "$env:LOCALAPPDATA\devpet\bin\petd.exe" --display both
```

## Usage

```
petd [--display board|desktop|both] [--port COM9] [--char clawd|beemo] [--reset-growth]
```

- Config + growth persist in `%APPDATA%\devpet\` (`config.json`, `pet_state.json`).
- Desktop pet interactions (aim at the pet's pixels — the transparent area
  around it is click-through):
  - **Click** — pet it: hearts + a little celebration (+1 XP, 60s cooldown)
  - **Double-click** — nap / wake toggle
  - **Drag** — move it anywhere
  - **Right-click** — info menu: level & current state, XP progress, character
    switch, 🍪 Feed (+5 XP, 10 min cooldown), 💤 Nap, **Wander when idle**
    (the pet slowly strolls across the screen and bounces off the edges),
    XP rules, board status, Minimize (restore from the taskbar), quit.
    Dismiss with **Close**, **Esc**, or by clicking the pet.
- **Note**: at Lv1 the pet is an egg for *both* characters — switching
  clawd/beemo has no visible effect until it hatches at Lv2 (100 XP).
- The board caches character + level in NVS, so it shows your grown pet even
  when the daemon isn't running (and dozes off after 8 min of silence).
- Manual state test (daemon running):
  `curl -X POST http://127.0.0.1:8127/event -d "{\"s\":\"celebrating\"}"`

### Growth rules

| Action | XP |
|---|---|
| Prompt submitted | 5 |
| Tool call | 1 |
| Active minute | 1 |
| Recovering from an error | 3 |
| Petting (click, max 1/min) | 1 |
| Feeding (menu, max 1/10min) | 5 |

Levels at 0 / 100 / 400 / 1200 / 3000 XP.

## Packaging (scripts\)

```powershell
.\scripts\package.ps1 [-Version 0.1.0] [-SkipFirmware]
```

Produces in `dist\`:

- `DevPet-<ver>-win64.zip` — portable: unzip anywhere, run `setup.ps1`
- `DevPet-Setup-<ver>.exe` — one-click installer (built with Windows IExpress):
  installs to `%LOCALAPPDATA%\devpet`, merges Claude Code hooks, adds Start
  Menu + login-autostart shortcuts, starts the daemon

Both include the prebuilt firmware images and `flash-firmware.ps1` (esptool,
standard ESP32 offsets), so end users don't need PlatformIO. Remove with
`scripts\uninstall.ps1` (`-Purge` also deletes growth/config). `package-macos.sh`
sketches the future .app/.dmg path — the daemon needs minor porting first and
must be built on a Mac.

## State detection

Claude Code hooks → `pet-hook.exe` → `petd`:
`UserPromptSubmit`→thinking · `PreToolUse` Edit/Write→coding (→debugging after
a recent error), Bash→testing/building/searching by command, Grep/Read/Web→
searching, Task→thinking · `PostToolUse` error→error (sticky 5s) · `Stop`→
success (10s)→idle · `Notification`→notify→waiting · 3 min idle→sleep.

## Project layout

```
firmware/        PlatformIO (Arduino + TFT_eSPI), 8-bpp 240x240 sprite double buffer
pc/asset-gen     pixel-art source of truth → sprites_gen.h + PNGs (run after art edits)
pc/petd          daemon: HTTP hook server + state machine + growth + serial + egui pet
pc/pet-hook      1ms-startup hook forwarder (stdin JSON → HTTP)
hooks/           Claude Code hooks snippet
relocate.ps1     sync this folder to D:\Git\Claude_Devboard_Pet (needs admin once)
install.ps1      release build + install exes to %LOCALAPPDATA%\devpet\bin
preview.png      contact sheet of all sprites (regenerated by asset-gen)
```

## Memory notes (ESP32)

A full-screen 16-bpp sprite would need 115KB of the WROOM-32's ~320KB usable
DRAM; this firmware uses an **8-bpp (RGB332) sprite = 57.6KB** instead, and
40x40 frames scaled up 4-6x at render time, so all 70 unique frames fit in
~109KB of flash (PROGMEM) — no SPIFFS needed. Sprite allocation is checked at
boot; failure shows an on-screen error instead of a null-pointer crash.
