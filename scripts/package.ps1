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

# 5. self-installing exe via IExpress (ships payload.zip + install.cmd)
$ix = Join-Path $dist 'iexpress'
if (Test-Path $ix) { Remove-Item $ix -Recurse -Force -Confirm:$false }
New-Item -ItemType Directory -Force $ix | Out-Null
Copy-Item $zip (Join-Path $ix 'DevPet-payload.zip')
@'
@echo off
echo Installing DevPet...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$d = Join-Path $env:TEMP ('devpet_' + [guid]::NewGuid().ToString('N')); Expand-Archive -LiteralPath (Join-Path '%~dp0' 'DevPet-payload.zip') -DestinationPath $d -Force; & (Join-Path $d 'setup.ps1'); Remove-Item $d -Recurse -Force -ErrorAction SilentlyContinue"
if errorlevel 1 ( echo Install failed. & pause & exit /b 1 )
echo Done. DevPet is running.
timeout /t 5 >nul
'@ | Set-Content -Encoding ascii (Join-Path $ix 'install.cmd')

$setupExe = Join-Path $dist "DevPet-Setup-$Version.exe"
if (Test-Path $setupExe) { Remove-Item $setupExe -Force }
@"
[Version]
Class=IEXPRESS
SEDVersion=3
[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=1
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=0
RebootMode=N
TargetName=$setupExe
FriendlyName=DevPet Setup $Version
AppLaunched=cmd /c install.cmd
PostInstallCmd=<None>
AdminQuietInstCmd=
UserQuietInstCmd=
SourceFiles=SourceFiles
[SourceFiles]
SourceFiles0=$ix\
[SourceFiles0]
%FILE0%=
%FILE1%=
[Strings]
FILE0="DevPet-payload.zip"
FILE1="install.cmd"
"@ | Set-Content -Encoding ascii (Join-Path $ix 'devpet.sed')

& "$env:WINDIR\System32\iexpress.exe" /N /Q /M (Join-Path $ix 'devpet.sed') | Out-Null
# iexpress launches async in some environments; wait for the output file
for ($i = 0; $i -lt 60 -and -not (Test-Path $setupExe); $i++) { Start-Sleep -Milliseconds 500 }
if (Test-Path $setupExe) {
    Write-Host "installer: $setupExe"
} else {
    Write-Warning 'IExpress did not produce the installer exe; the portable zip is still valid.'
}

Write-Host '== done =='
Get-ChildItem $dist -File | Select-Object Name, @{n='MB';e={[math]::Round($_.Length/1MB,2)}}
