# package.ps1 — build the whole DevPet project and produce distributables:
#   dist\DevPet-<ver>-win64.zip     portable package (unzip + run setup.ps1)
#   dist\DevPet-Setup-<ver>.exe     self-installing exe (built with Windows' IExpress)
#
# Usage: .\scripts\package.ps1 [-Version 0.1.0] [-SkipFirmware]
# Prereqs: Rust (cargo), and for firmware images Python + platformio.

param(
    [string]$Version = '0.1.0',
    [switch]$SkipFirmware
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist "DevPet-$Version"
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

Write-Host "== DevPet package v$Version =="

# 1. regenerate assets + release binaries
Push-Location (Join-Path $root 'pc')
& $cargo run --release -q -p asset-gen
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error 'asset-gen failed'; exit 1 }
& $cargo build --release -p petd -p pet-hook
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error 'cargo build failed'; exit 1 }
Pop-Location

# 2. firmware images
if (-not $SkipFirmware) {
    Push-Location (Join-Path $root 'firmware')
    python -m platformio run
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error 'firmware build failed'; exit 1 }
    Pop-Location
}

# 3. stage
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force -Confirm:$false }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root 'pc\target\release\petd.exe') $stage
Copy-Item (Join-Path $root 'pc\target\release\pet-hook.exe') $stage
Copy-Item (Join-Path $PSScriptRoot 'setup.ps1') $stage
Copy-Item (Join-Path $PSScriptRoot 'uninstall.ps1') $stage
Copy-Item (Join-Path $root 'README.md') $stage
Copy-Item (Join-Path $root 'preview.png') $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $stage 'hooks') | Out-Null
Copy-Item (Join-Path $root 'hooks\settings.snippet.json') (Join-Path $stage 'hooks')

$fwOut = Join-Path $root 'firmware\.pio\build\esp32doit-devkit-v1'
if (Test-Path (Join-Path $fwOut 'firmware.bin')) {
    $fwStage = Join-Path $stage 'firmware'
    New-Item -ItemType Directory -Force $fwStage | Out-Null
    Copy-Item (Join-Path $fwOut 'firmware.bin') $fwStage
    Copy-Item (Join-Path $fwOut 'bootloader.bin') $fwStage
    Copy-Item (Join-Path $fwOut 'partitions.bin') $fwStage
    $bootApp0 = Join-Path $env:USERPROFILE '.platformio\packages\framework-arduinoespressif32\tools\partitions\boot_app0.bin'
    if (Test-Path $bootApp0) { Copy-Item $bootApp0 $fwStage } else { Write-Warning "boot_app0.bin not found at $bootApp0" }
    Copy-Item (Join-Path $PSScriptRoot 'flash-firmware.ps1') $fwStage
} else {
    Write-Warning 'firmware images missing from stage (built with -SkipFirmware?)'
}

# 4. portable zip
$zip = Join-Path $dist "DevPet-$Version-win64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "zip: $zip"

# 5. self-installing exe: Rust stub with the payload zip embedded
$setupExe = Join-Path $dist "DevPet-Setup-$Version.exe"
if (Test-Path $setupExe) { Remove-Item $setupExe -Force }
$env:PAYLOAD_ZIP = $zip
Push-Location (Join-Path $root 'pc')
& $cargo build --release -p devpet-setup
$buildOk = ($LASTEXITCODE -eq 0)
Pop-Location
Remove-Item Env:\PAYLOAD_ZIP
if ($buildOk) {
    Copy-Item (Join-Path $root 'pc\target\release\devpet-setup.exe') $setupExe -Force
    Write-Host "installer: $setupExe"
} else {
    Write-Warning 'installer stub build failed; the portable zip is still valid.'
}

Write-Host '== done =='
Get-ChildItem $dist -File | Select-Object Name, @{n='MB';e={[math]::Round($_.Length/1MB,2)}}
