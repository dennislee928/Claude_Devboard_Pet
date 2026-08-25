# relocate.ps1 ??sync this project folder into D:\Git\Claude_Devboard_Pet, then remove the local copy.
#
# Usage:
#   .\relocate.ps1                 # copy -> verify -> delete source (this folder)
#   .\relocate.ps1 -KeepSource     # copy only, keep this folder
#   .\relocate.ps1 -NoElevate      # never trigger a UAC prompt, fail instead
#
# D:\Git is writable only by Administrators on this machine. If access is denied,
# the script relaunches itself elevated (UAC credential prompt). Build artifacts
# (Rust target/, PlatformIO .pio/) are excluded from the copy.

param(
    [switch]$KeepSource,
    [switch]$NoElevate
)

$ErrorActionPreference = 'Stop'
$Source = $PSScriptRoot
$Dest   = 'D:\Git\Claude_Devboard_Pet'

if (-not $Source -or -not (Test-Path $Source)) { Write-Error 'Cannot resolve script folder.'; exit 1 }
if ($Source -ieq $Dest) { Write-Host 'Already running from the destination; nothing to do.'; exit 0 }

# Probe write access to the destination.
$probeOk = $true
try {
    if (-not (Test-Path $Dest)) { New-Item -ItemType Directory -Path $Dest -ErrorAction Stop | Out-Null }
    $probe = Join-Path $Dest ('.wprobe_' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType File -Path $probe -ErrorAction Stop | Out-Null
    Remove-Item $probe -Force
} catch {
    $probeOk = $false
}

if (-not $probeOk) {
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
               ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin -or $NoElevate) {
        Write-Error "No write access to $Dest. Ask an admin to run:  icacls `"$Dest`" /grant `"${env:USERDOMAIN}\${env:USERNAME}:(OI)(CI)M`""
        exit 1
    }
    Write-Host "Write access to $Dest denied - requesting elevation (UAC)..."
    $args = @('-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`"")
    if ($KeepSource) { $args += '-KeepSource' }
    $args += '-NoElevate'
    try {
        $p = Start-Process powershell -Verb RunAs -ArgumentList $args -Wait -PassThru
        exit $p.ExitCode
    } catch {
        Write-Error "Elevation was cancelled or unavailable. Run this script from an admin shell, or have an admin run: icacls `"$Dest`" /grant `"${env:USERDOMAIN}\${env:USERNAME}:(OI)(CI)M`""
        exit 1
    }
}

Write-Host "Syncing $Source -> $Dest ..."
# /MIR mirror, exclude build artifacts and VCS-less junk. Robocopy exit codes 0-7 = success.
robocopy $Source $Dest /MIR /R:2 /W:2 /NFL /NDL /NJH /XD target .pio | Out-Null
if ($LASTEXITCODE -ge 8) { Write-Error "robocopy failed with code $LASTEXITCODE"; exit 1 }

# Verify: compare file counts (excluding the excluded dirs).
$exclude = '\\pc\\target(\\|$)|\\firmware\\\.pio(\\|$)'
$srcCount = (Get-ChildItem $Source -Recurse -File | Where-Object { $_.FullName -notmatch $exclude }).Count
$dstCount = (Get-ChildItem $Dest   -Recurse -File | Where-Object { $_.FullName -notmatch $exclude }).Count
if ($dstCount -lt $srcCount) { Write-Error "Verification failed: $srcCount source files vs $dstCount copied."; exit 1 }
Write-Host "Copied OK ($dstCount files)."

if (-not $KeepSource) {
    # Delete the source copy (self-deletion of a running script is fine once loaded).
    Set-Location (Split-Path $Source -Parent)
    Remove-Item $Source -Recurse -Force -Confirm:$false
    Write-Host "Removed local copy $Source. Project now lives at $Dest."
} else {
    Write-Host 'KeepSource set - local copy retained.'
}


