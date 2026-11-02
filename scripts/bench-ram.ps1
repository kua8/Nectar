<#
.SYNOPSIS
  Launches Nectar, lets it settle, samples its memory, then quits it cleanly.

.DESCRIPTION
  Nectar hides the Windows taskbar and registers app bars, so it must be shut
  down through its own exit path. This script sends the normal close request
  (which restores the taskbar) and falls back to restoring the taskbar itself
  if the process has to be killed.

  Use -KeepRunning to inspect the app afterwards, or -Exe to compare builds.

.PARAMETER Exe
  Path to nectar.exe. Default: src-tauri\target\release\nectar.exe

.PARAMETER WarmupSeconds
  Seconds to wait after launch before sampling. Default: 20

.PARAMETER Samples
  Number of samples to average. Default: 5

.PARAMETER IntervalSeconds
  Seconds between samples. Default: 3

.PARAMETER Detail
  Print the per-process breakdown of the last sample.

.PARAMETER KeepRunning
  Leave Nectar running afterwards instead of quitting it.

.EXAMPLE
  .\scripts\bench-ram.ps1 -Detail
  .\scripts\bench-ram.ps1 -Samples 10 -IntervalSeconds 2 -KeepRunning
#>
[CmdletBinding()]
param(
  [string]$Exe = 'src-tauri\target\release\nectar.exe',
  [int]$WarmupSeconds = 20,
  [int]$Samples = 5,
  [double]$IntervalSeconds = 3,
  [switch]$Detail,
  [switch]$KeepRunning
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\nectar-mem.ps1"

$exePath = (Resolve-Path $Exe).Path

if (Get-Process -Name 'nectar' -ErrorAction SilentlyContinue) {
  Write-Host "Nectar is already running; stopping it first." -ForegroundColor Yellow
  [void](Stop-Nectar)
}

Write-Host "Launching $exePath" -ForegroundColor Cyan
$launched = Start-Process -FilePath $exePath -PassThru
Write-Host "Waiting ${WarmupSeconds}s for startup to settle..." -ForegroundColor DarkGray
Start-Sleep -Seconds $WarmupSeconds

if ($launched.HasExited) {
  Write-Host "nectar exited during startup (exit code $($launched.ExitCode))." -ForegroundColor Red
  Restore-NativeTaskbar
  return
}

# Note: not `$samples` -- PowerShell variable names are case-insensitive, so it
# would be the same variable as the [int]$Samples parameter.
$collected = @()
for ($i = 0; $i -lt $Samples; $i++) {
  $sample = Get-NectarProcessTree
  if ($null -eq $sample) { Write-Host "nectar exited while sampling." -ForegroundColor Red; break }
  $collected += $sample
  if ($i -lt $Samples - 1) { Start-Sleep -Seconds $IntervalSeconds }
}

if ($collected.Count -eq 0) {
  if (-not $KeepRunning) { Restore-NativeTaskbar }
  return
}

$last = $collected[-1]
$ws = $collected | Measure-Object PrivateWS -Average -Minimum -Maximum
$commit = $collected | Measure-Object PrivateBytes -Average -Minimum -Maximum

Write-Host ""
Write-Host "=== Nectar memory report ===" -ForegroundColor Cyan
Show-NectarMemory -Sample $last -Detail:$Detail
Write-Host ""
Write-Host ("  private WS  avg : {0}  [min {1} / max {2}]" -f (Format-MemMB $ws.Average), (Format-MemMB $ws.Minimum), (Format-MemMB $ws.Maximum)) -ForegroundColor Green
Write-Host ("  commit      avg : {0}  [min {1} / max {2}]" -f (Format-MemMB $commit.Average), (Format-MemMB $commit.Minimum), (Format-MemMB $commit.Maximum))

if (-not $KeepRunning) {
  Write-Host ""
  Write-Host "Shutting down through the app's close path..." -ForegroundColor DarkGray
  [void](Stop-Nectar)
}
