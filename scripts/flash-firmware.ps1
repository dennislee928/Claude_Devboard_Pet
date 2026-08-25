# flash-firmware.ps1 — write the prebuilt DevPet firmware images to an ESP32.
# Usage: .\flash-firmware.ps1 [-Port COM9]
# Requires Python 3; installs esptool via pip if missing. Image files are
# expected next to this script (bootloader.bin, partitions.bin, boot_app0.bin,
# firmware.bin — standard ESP32 Arduino offsets).

param([string]$Port = '')

$ErrorActionPreference = 'Stop'
$dir = $PSScriptRoot

python -c "import esptool" 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Host 'installing esptool...'
    pip install --quiet esptool
}

$args = @('-m', 'esptool', '--chip', 'esp32', '--baud', '460800')
if ($Port) { $args += @('--port', $Port) }
$args += @(
    'write_flash',
    '0x1000',  (Join-Path $dir 'bootloader.bin'),
    '0x8000',  (Join-Path $dir 'partitions.bin'),
    '0xe000',  (Join-Path $dir 'boot_app0.bin'),
    '0x10000', (Join-Path $dir 'firmware.bin')
)
python @args
if ($LASTEXITCODE -ne 0) { Write-Error 'flash failed'; exit 1 }
Write-Host 'firmware flashed OK'
