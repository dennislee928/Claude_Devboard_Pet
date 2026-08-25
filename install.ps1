# install.ps1 — build the PC binaries in release mode and install them to
# %LOCALAPPDATA%\devpet\bin (a stable path that survives project relocation,
# safe to reference from Claude Code hook settings).

$ErrorActionPreference = 'Stop'
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

Push-Location (Join-Path $PSScriptRoot 'pc')
& $cargo run --release -q -p asset-gen
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error 'asset-gen failed'; exit 1 }
& $cargo build --release -p petd -p pet-hook
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error 'cargo build failed'; exit 1 }
Pop-Location

$bin = Join-Path $env:LOCALAPPDATA 'devpet\bin'
New-Item -ItemType Directory -Force $bin | Out-Null
# a running daemon locks the exe — stop it before overwriting
foreach ($p in 'petd', 'petd-lite') {
    try { Stop-Process -Name $p -Force -Confirm:$false -ErrorAction Stop } catch {}
}
Start-Sleep -Seconds 2
foreach ($exe in 'petd.exe', 'petd-lite.exe', 'pet-hook.exe') {
    Copy-Item (Join-Path $PSScriptRoot "pc\target\release\$exe") $bin -Force
}

Write-Host "Installed to $bin"
Write-Host '  petd.exe       - standalone edition: the whole pet runs on this PC'
Write-Host '  petd-lite.exe  - firmware edition: the dev board is the brain, this only bridges'
Write-Host '  pet-hook.exe   - referenced by Claude Code hooks (see hooks\settings.snippet.json)'
Write-Host ''
Write-Host 'Run silently at login:  petd.exe --install-autostart --display both'
