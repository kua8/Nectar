<#
Shared helpers for measuring Nectar's memory footprint.

Dot-source from other scripts:
    . "$PSScriptRoot\nectar-mem.ps1"

Why a process tree: a Tauri app is nectar.exe plus the WebView2 processes it
owns -- one browser process, one GPU process, one renderer per site, network
and storage utility processes, and a crashpad handler. Task Manager's number
for "nectar" is the sum of their private working sets, so that is what these
functions report, next to commit (private bytes) and total working set.

Parent PIDs come back from CIM as UInt32 and PowerShell hashtables compare
strictly, so every key is cast to Int32. Skipping that cast silently reports
"no children" and understates an app like this by an order of magnitude.
#>

$script:NectarExeName = 'nectar'

function Format-MemMB {
  param([double]$Bytes)
  '{0,8:N1} MB' -f ($Bytes / 1MB)
}

function Get-NectarProcessTree {
  <#
    Returns every process belonging to the app: the root process plus all
    descendants (the WebView2 browser process and, under it, the GPU,
    renderer, utility and crashpad processes).
  #>
  param([string]$ProcessName = $script:NectarExeName)

  $roots = @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue)
  if ($roots.Count -eq 0) { return $null }

  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, CommandLine -ErrorAction SilentlyContinue

  $children = @{}
  foreach ($p in $all) {
    if ($null -eq $p.ParentProcessId) { continue }
    $parent = [int]$p.ParentProcessId
    if (-not $children.ContainsKey($parent)) { $children[$parent] = New-Object System.Collections.Generic.List[object] }
    $children[$parent].Add($p)
  }

  $queue = New-Object System.Collections.Generic.Queue[int]
  $seen = New-Object System.Collections.Generic.HashSet[int]
  foreach ($r in $roots) { [void]$queue.Enqueue([int]$r.Id); [void]$seen.Add([int]$r.Id) }

  $pids = New-Object System.Collections.Generic.List[int]
  while ($queue.Count -gt 0) {
    $cur = $queue.Dequeue()
    $pids.Add($cur)
    if ($children.ContainsKey($cur)) {
      foreach ($child in $children[$cur]) {
        $cid = [int]$child.ProcessId
        if ($seen.Add($cid)) { $queue.Enqueue($cid) }
      }
    }
  }

  $meta = @{}
  foreach ($p in $all) { $meta[[int]$p.ProcessId] = $p }

  # WorkingSetPrivate is what Task Manager shows as "Memory"; Win32_Process
  # does not expose it, so it comes from the perf counter class.
  $privateWS = @{}
  try {
    foreach ($row in (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue)) {
      if ($row.IDProcess) { $privateWS[[int]$row.IDProcess] = [int64]$row.WorkingSetPrivate }
    }
  } catch { }

  function Get-Role([string]$commandLine, [string]$name) {
    if ($name -eq $script:NectarExeName) { return 'host' }
    if (-not $commandLine) { return 'unknown' }
    if ($commandLine -match '--type=renderer')  { return 'renderer' }
    if ($commandLine -match '--type=gpu-process') { return 'gpu' }
    if ($commandLine -match '--type=crashpad-handler') { return 'crashpad' }
    if ($commandLine -match '--type=utility') {
      if ($commandLine -match '--utility-sub-type=([^ ]+)') { return "utility:$($Matches[1])" }
      return 'utility'
    }
    if ($commandLine -match '--embedded-browser-webview') { return 'browser' }
    return 'other'
  }

  $rows = @()
  foreach ($p in $pids) {
    $np = Get-Process -Id $p -ErrorAction SilentlyContinue
    if (-not $np) { continue }
    $cmd = if ($meta.ContainsKey($p)) { $meta[$p].CommandLine } else { $null }
    $rows += [pscustomobject]@{
      Pid          = $p
      Role         = Get-Role $cmd $np.ProcessName
      PrivateWS    = if ($privateWS.ContainsKey($p)) { $privateWS[$p] } else { [int64]$np.PrivateMemorySize64 }
      WorkingSet   = [int64]$np.WorkingSet64
      PrivateBytes = [int64]$np.PrivateMemorySize64
      Threads      = $np.Threads.Count
    }
  }

  if ($rows.Count -eq 0) { return $null }

  return [pscustomobject]@{
    Rows         = $rows
    Count        = $rows.Count
    Renderers    = @($rows | Where-Object { $_.Role -eq 'renderer' }).Count
    PrivateWS    = ($rows | Measure-Object PrivateWS -Sum).Sum
    WorkingSet   = ($rows | Measure-Object WorkingSet -Sum).Sum
    PrivateBytes = ($rows | Measure-Object PrivateBytes -Sum).Sum
    Threads      = ($rows | Measure-Object Threads -Sum).Sum
  }
}

function Show-NectarMemory {
  param(
    [Parameter(Mandatory)] $Sample,
    [switch]$Detail
  )

  Write-Host ("  processes       : {0}  (renderers: {1})" -f $Sample.Count, $Sample.Renderers)
  Write-Host ("  private WS      : {0}   <- Task Manager 'Memory'" -f (Format-MemMB $Sample.PrivateWS)) -ForegroundColor Green
  Write-Host ("  commit          : {0}" -f (Format-MemMB $Sample.PrivateBytes))
  Write-Host ("  working set     : {0}" -f (Format-MemMB $Sample.WorkingSet))
  Write-Host ("  threads         : {0}" -f $Sample.Threads)

  if ($Detail) {
    Write-Host ""
    $Sample.Rows | Sort-Object PrivateWS -Descending |
      Format-Table @{n='PID';e={$_.Pid}}, Role,
                   @{n='PrivateWS';e={Format-MemMB $_.PrivateWS}},
                   @{n='WorkingSet';e={Format-MemMB $_.WorkingSet}},
                   @{n='Commit';e={Format-MemMB $_.PrivateBytes}}, Threads |
      Out-String -Width 120 | Write-Host
  }
}

function Restore-NativeTaskbar {
  # Nectar hides Shell_TrayWnd while running. This is the fallback for when a
  # run ends without going through the app's own restore path.
  try {
    if (-not ('NectarMem.Native' -as [type])) {
      Add-Type -Namespace NectarMem -Name Native -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll", CharSet = System.Runtime.InteropServices.CharSet.Ansi)]
public static extern System.IntPtr FindWindowA(string cls, string win);
[System.Runtime.InteropServices.DllImport("user32.dll")]
public static extern bool ShowWindow(System.IntPtr hWnd, int nCmdShow);
'@
    }
    foreach ($cls in @('Shell_TrayWnd', 'Shell_SecondaryTrayWnd')) {
      $h = [NectarMem.Native]::FindWindowA($cls, $null)
      if ($h -ne [IntPtr]::Zero) { [void][NectarMem.Native]::ShowWindow($h, 5) }
    }
  } catch { }
}

function Stop-Nectar {
  <#
    Shuts Nectar down the way the user would: a close request, which the app
    handles by unregistering its app bars, restoring the taskbar and exiting.
  #>
  param([int]$TimeoutSeconds = 15)

  $proc = Get-Process -Name $script:NectarExeName -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $proc) { return $true }

  $proc.CloseMainWindow() | Out-Null
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  while ((Get-Date) -lt $deadline) {
    if (-not (Get-Process -Name $script:NectarExeName -ErrorAction SilentlyContinue)) { return $true }
    Start-Sleep -Milliseconds 300
  }

  Write-Host "  close request was ignored; stopping the process tree" -ForegroundColor Yellow
  Get-Process -Name $script:NectarExeName -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 700
  Restore-NativeTaskbar
  return $false
}
