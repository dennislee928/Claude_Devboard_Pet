# uninstall.ps1 — remove DevPet from this machine (keeps growth/config unless -Purge).
param([switch]$Purge)

$ErrorActionPreference = 'SilentlyContinue'
foreach ($p in 'petd', 'petd-lite') {
    try { Stop-Process -Name $p -Force -Confirm:$false -ErrorAction Stop } catch {}
}
# drop the autostart entry the app (or the MSI) may have written
reg delete 'HKCU\Software\Microsoft\Windows\CurrentVersion\Run' /v DevPet /f 2>$null | Out-Null
Start-Sleep -Seconds 1

Remove-Item (Join-Path $env:LOCALAPPDATA 'devpet\bin') -Recurse -Force -Confirm:$false
Remove-Item (Join-Path $env:LOCALAPPDATA 'devpet\firmware') -Recurse -Force -Confirm:$false
if ($Purge) { Remove-Item (Join-Path $env:APPDATA 'devpet') -Recurse -Force -Confirm:$false }
Remove-Item (Join-Path ([Environment]::GetFolderPath('Programs')) 'DevPet.lnk') -Force -Confirm:$false
Remove-Item (Join-Path ([Environment]::GetFolderPath('Programs')) 'DevPet (firmware edition).lnk') -Force -Confirm:$false
Remove-Item (Join-Path ([Environment]::GetFolderPath('Startup')) 'DevPet.lnk') -Force -Confirm:$false

# strip pet-hook entries from Claude Code settings
$settings = Join-Path $env:USERPROFILE '.claude\settings.json'
if (Test-Path $settings) {
    $json = Get-Content $settings -Raw | ConvertFrom-Json
    if ($json.PSObject.Properties['hooks']) {
        $names = @($json.hooks.PSObject.Properties.Name)
        foreach ($n in $names) {
            $kept = @($json.hooks.$n | Where-Object { ($_ | ConvertTo-Json -Depth 8) -notmatch 'pet-hook\.exe' })
            if ($kept.Count -gt 0) { $json.hooks.$n = $kept } else { $json.hooks.PSObject.Properties.Remove($n) }
        }
        if (@($json.hooks.PSObject.Properties).Count -eq 0) { $json.PSObject.Properties.Remove('hooks') }
        $json | ConvertTo-Json -Depth 16 | Set-Content -Encoding utf8 $settings
    }
}
Write-Host 'DevPet uninstalled.' -ForegroundColor Green
if (-not $Purge) { Write-Host 'Growth/config kept in %APPDATA%\devpet (use -Purge to remove).' }
