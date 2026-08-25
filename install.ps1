# install.ps1 — build the PC binaries in release mode and install them to
# %LOCALAPPDATA%\devpet\bin (a stable path that survives project relocation,
# safe to reference from Claude Code hook settings).

$ErrorActionPreference = 'Stop'
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

Push-Location (Join-Path $PSScriptRoot 'pc')
& $cargo build --release -p petd -p pet-hook
if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error 'cargo build failed'; exit 1 }
Pop-Location

$bin = Join-Path $env:LOCALAPPDATA 'devpet\bin'
New-Item -ItemType Directory -Force $bin | Out-Null
# a running daemon locks the exe — stop it before overwriting
try { Stop-Process -Name petd -Force -Confirm:$false -ErrorAction Stop; Start-Sleep -Seconds 2 } catch {}
Copy-Item (Join-Path $PSScriptRoot 'pc\target\release\petd.exe') $bin -Force
Copy-Item (Join-Path $PSScriptRoot 'pc\target\release\pet-hook.exe') $bin -Force

Write-Host "Installed to $bin"
Write-Host '  petd.exe      - the daemon (run at login, e.g. shell:startup shortcut)'
Write-Host '  pet-hook.exe  - referenced by Claude Code hooks (see hooks\settings.snippet.json)'
