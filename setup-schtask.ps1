<#
.SYNOPSIS
    Sets up a Windows Scheduled Task to run Immich-Album-Sync.ps1 at every PC boot.
.DESCRIPTION
    This script creates (or updates) a scheduled task named 'ImmichAlbumSync' that runs the sync script at every system startup.
.NOTES
    Run this script as Administrator.
#>

$taskName = "ImmichAlbumSync"
$scriptPath = Join-Path $PSScriptRoot 'Immich-Album-Sync.ps1'

if (-not (Test-Path $scriptPath)) {
    Write-Error "Could not find Immich-Album-Sync.ps1 at $scriptPath"
    exit 1
}

$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`""
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest

try {
    Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Force
    Write-Host "Scheduled task '$taskName' created/updated to run at startup."
}
catch {
    Write-Error "Failed to register scheduled task: $_"
    exit 1
}
