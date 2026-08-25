# setup.ps1 — DevPet installer payload. Installs binaries + firmware images to
# %LOCALAPPDATA%\devpet, merges Claude Code hooks, creates shortcuts, starts
# the daemon. Safe to re-run (upgrades in place).

$ErrorActionPreference = 'Stop'
$src = $PSScriptRoot
$root = Join-Path $env:LOCALAPPDATA 'devpet'
$bin = Join-Path $root 'bin'

Write-Host '== DevPet setup =='

# 1. stop a running daemon so the exe can be replaced
foreach ($p in 'petd', 'petd-lite') {
    try { Stop-Process -Name $p -Force -Confirm:$false -ErrorAction Stop } catch {}
}
Start-Sleep -Seconds 2

# 2. binaries + firmware images
New-Item -ItemType Directory -Force $bin | Out-Null
foreach ($exe in 'petd.exe', 'petd-lite.exe', 'pet-hook.exe') {
    $from = Join-Path $src $exe
    if (Test-Path $from) { Copy-Item $from $bin -Force } else { Write-Warning "$exe missing from the package" }
}
if (Test-Path (Join-Path $src 'firmware')) {
    Copy-Item (Join-Path $src 'firmware') $root -Recurse -Force
}
Write-Host "installed binaries -> $bin"

# 3. Claude Code hooks (conservative merge)
$settings = Join-Path $env:USERPROFILE '.claude\settings.json'
$hookCmd = '"' + (Join-Path $bin 'pet-hook.exe') + '"'
function New-HookEntry([bool]$withMatcher) {
    $e = [ordered]@{ hooks = @([ordered]@{ type = 'command'; command = $hookCmd }) }
    if ($withMatcher) { $e = [ordered]@{ matcher = '*'; hooks = $e.hooks } }
    return $e
}
$hooksObj = [ordered]@{
    SessionStart     = @(New-HookEntry $false)
    UserPromptSubmit = @(New-HookEntry $false)
    PreToolUse       = @(New-HookEntry $true)
    PostToolUse      = @(New-HookEntry $true)
    Notification     = @(New-HookEntry $false)
    SubagentStop     = @(New-HookEntry $false)
    Stop             = @(New-HookEntry $false)
}
if (Test-Path $settings) {
    $raw = Get-Content $settings -Raw
    if ($raw -match 'pet-hook\.exe') {
        Write-Host 'hooks: already present, skipping'
    } else {
        $json = $raw | ConvertFrom-Json
        if ($json.PSObject.Properties['hooks']) {
            Write-Warning "hooks: $settings already defines hooks - merge hooks\settings.snippet.json manually."
        } else {
            $json | Add-Member -NotePropertyName hooks -NotePropertyValue $hooksObj
            $json | ConvertTo-Json -Depth 16 | Set-Content -Encoding utf8 $settings
            Write-Host "hooks: merged into $settings"
        }
    }
} else {
    New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
    @{ hooks = $hooksObj } | ConvertTo-Json -Depth 16 | Set-Content -Encoding utf8 $settings
    Write-Host "hooks: created $settings"
}

# 4. Start Menu + autostart shortcuts
$ws = New-Object -ComObject WScript.Shell
$startMenu = [Environment]::GetFolderPath('Programs')
$lnk = $ws.CreateShortcut((Join-Path $startMenu 'DevPet.lnk'))
$lnk.TargetPath = Join-Path $bin 'petd.exe'
$lnk.Arguments = '--display both'
$lnk.Description = 'DevPet desk-pet daemon (standalone edition)'
$lnk.Save()
$lnkLite = $ws.CreateShortcut((Join-Path $startMenu 'DevPet (firmware edition).lnk'))
$lnkLite.TargetPath = Join-Path $bin 'petd-lite.exe'
$lnkLite.Arguments = '--display both'
$lnkLite.Description = 'Bridge to a DevPet dev board that runs the pet itself'
$lnkLite.Save()
$auto = $ws.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Startup')) 'DevPet.lnk'))
$auto.TargetPath = Join-Path $bin 'petd.exe'
$auto.Arguments = '--display both'
$auto.Save()
Write-Host 'shortcuts: Start Menu + Startup (runs at login)'

# 5. launch silently in the background (petd.exe is a GUI-subsystem binary,
#    so this never opens a console window)
Start-Process (Join-Path $bin 'petd.exe') -ArgumentList '--display','both' -WindowStyle Hidden
Write-Host 'DevPet is running. Flash the board with firmware\flash-firmware.ps1 if you have not yet.'
