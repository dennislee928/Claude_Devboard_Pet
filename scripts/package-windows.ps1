# package-windows.ps1 — build DevPet for Windows and produce:
#   dist\DevPet-<ver>-win64.zip      portable package (unzip + run setup.ps1)
#   dist\DevPet-Setup-<ver>.exe      self-installing exe (Rust stub + payload)
#   dist\DevPet-<ver>.msi            per-user MSI (WiX v5)
#   dist\DevPet-selfsigned.cer       the publisher certificate, for inspection
#
# Usage:
#   .\scripts\package-windows.ps1 [-Version 0.1.0] [-SkipFirmware] [-Sign]
#
# -Sign creates (or reuses) a self-signed code-signing certificate and signs
# every produced binary and installer. Publisher information ("DevPet Project")
# is compiled into the exes by pc\petd\build.rs and declared again in the MSI.
#
# Prereqs: Rust (cargo); for the MSI the WiX v5 CLI (`dotnet tool install
# --global wix`); for firmware images Python + platformio.

param(
    [string]$Version = '0.1.0',
    [switch]$SkipFirmware,
    [switch]$Sign,
    [switch]$SkipMsi
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist "DevPet-$Version"
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }
New-Item -ItemType Directory -Force $dist | Out-Null

Write-Host "== DevPet Windows package v$Version =="

# ---- certificate ---------------------------------------------------------
$cert = $null
if ($Sign) {
    $cert = & (Join-Path $PSScriptRoot 'make-selfsigned-cert.ps1') `
        -Export (Join-Path $dist 'DevPet-selfsigned.cer')
}
function Invoke-Sign([string]$Path) {
    if (-not $cert) { return }
    # A self-signed certificate is not chained to a trusted root, so Windows
    # reports the *verification* status as UnknownError even though the
    # signature itself was written correctly. Only a missing SignerCertificate
    # means signing actually failed.
    $r = Set-AuthenticodeSignature -FilePath $Path -Certificate $cert `
        -HashAlgorithm SHA256 -TimestampServer 'http://timestamp.digicert.com' `
        -ErrorAction SilentlyContinue
    if (-not $r -or -not $r.SignerCertificate) {
        # no network for the timestamp server? sign without one
        $r = Set-AuthenticodeSignature -FilePath $Path -Certificate $cert -HashAlgorithm SHA256
    }
    $leaf = Split-Path $Path -Leaf
    if (-not $r -or -not $r.SignerCertificate) {
        throw "failed to sign $leaf"
    }
    $stamped = if ($r.TimeStamperCertificate) { 'timestamped' } else { 'no timestamp' }
    Write-Host ("  signed {0} as {1} ({2}; verification: {3} - expected for a self-signed root)" -f `
        $leaf, $r.SignerCertificate.Subject, $stamped, $r.Status)
}

# ---- 1. assets + release binaries ---------------------------------------
Push-Location (Join-Path $root 'pc')
& $cargo run --release -q -p asset-gen
if ($LASTEXITCODE -ne 0) { Pop-Location; throw 'asset-gen failed' }
& $cargo build --release -p petd -p pet-hook
if ($LASTEXITCODE -ne 0) { Pop-Location; throw 'cargo build failed' }
Pop-Location

$rel = Join-Path $root 'pc\target\release'
foreach ($exe in 'petd.exe', 'petd-lite.exe', 'pet-hook.exe') {
    Invoke-Sign (Join-Path $rel $exe)
}

# ---- 2. firmware images -------------------------------------------------
if (-not $SkipFirmware) {
    Push-Location (Join-Path $root 'firmware')
    python -m platformio run
    $fwOk = ($LASTEXITCODE -eq 0)
    Pop-Location
    if (-not $fwOk) { Write-Warning 'firmware build failed; continuing without images' }
}

# ---- 3. stage -----------------------------------------------------------
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force -Confirm:$false }
New-Item -ItemType Directory -Force $stage | Out-Null
foreach ($exe in 'petd.exe', 'petd-lite.exe', 'pet-hook.exe') {
    Copy-Item (Join-Path $rel $exe) $stage
}
Copy-Item (Join-Path $PSScriptRoot 'setup.ps1') $stage
Copy-Item (Join-Path $PSScriptRoot 'uninstall.ps1') $stage
Copy-Item (Join-Path $root 'README.md') $stage
Copy-Item (Join-Path $root 'preview.png') $stage -ErrorAction SilentlyContinue
Copy-Item (Join-Path $root 'hooks\settings.snippet.json') $stage
New-Item -ItemType Directory -Force (Join-Path $stage 'hooks') | Out-Null
Copy-Item (Join-Path $root 'hooks\settings.snippet.json') (Join-Path $stage 'hooks')

$fwOut = Join-Path $root 'firmware\.pio\build\esp32doit-devkit-v1'
if (Test-Path (Join-Path $fwOut 'firmware.bin')) {
    $fwStage = Join-Path $stage 'firmware'
    New-Item -ItemType Directory -Force $fwStage | Out-Null
    foreach ($img in 'firmware.bin', 'bootloader.bin', 'partitions.bin') {
        Copy-Item (Join-Path $fwOut $img) $fwStage
    }
    $bootApp0 = Join-Path $env:USERPROFILE '.platformio\packages\framework-arduinoespressif32\tools\partitions\boot_app0.bin'
    if (Test-Path $bootApp0) { Copy-Item $bootApp0 $fwStage } else { Write-Warning "boot_app0.bin not found at $bootApp0" }
    Copy-Item (Join-Path $PSScriptRoot 'flash-firmware.ps1') $fwStage
} else {
    Write-Warning 'firmware images missing from the package'
}

# ---- 4. portable zip ----------------------------------------------------
$zip = Join-Path $dist "DevPet-$Version-win64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "zip: $zip"

# ---- 5. self-installing exe (Rust stub with the zip embedded) -----------
$setupExe = Join-Path $dist "DevPet-Setup-$Version.exe"
if (Test-Path $setupExe) { Remove-Item $setupExe -Force }
$env:PAYLOAD_ZIP = $zip
Push-Location (Join-Path $root 'pc')
& $cargo build --release -p devpet-setup
$buildOk = ($LASTEXITCODE -eq 0)
Pop-Location
Remove-Item Env:\PAYLOAD_ZIP
if ($buildOk) {
    Copy-Item (Join-Path $rel 'devpet-setup.exe') $setupExe -Force
    Invoke-Sign $setupExe
    Write-Host "installer: $setupExe"
} else {
    Write-Warning 'installer stub build failed; the portable zip is still valid.'
}

# ---- 6. MSI -------------------------------------------------------------
if (-not $SkipMsi) {
    $wix = Get-Command wix -ErrorAction SilentlyContinue
    if (-not $wix) {
        Write-Warning 'WiX v5 CLI not found (dotnet tool install --global wix); skipping the MSI.'
    } else {
        $msi = Join-Path $dist "DevPet-$Version.msi"
        if (Test-Path $msi) { Remove-Item $msi -Force }
        & wix build (Join-Path $root 'packaging\wix\DevPet.wxs') `
            -d Version=$Version -d StageDir=$stage -arch x64 -o $msi
        if ($LASTEXITCODE -eq 0) {
            Invoke-Sign $msi
            Write-Host "msi: $msi"
        } else {
            Write-Warning 'wix build failed; the zip and setup exe are still valid.'
        }
    }
}

Write-Host '== done =='
Get-ChildItem $dist -File | Select-Object Name, @{n='MB';e={[math]::Round($_.Length/1MB,2)}}
