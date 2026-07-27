param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$BundleDirectory
)

$ErrorActionPreference = "Stop"
$script:cleanupProcessTreeQuiescent = $true

function ConvertTo-ProcessCreationIdentityTicks {
  param(
    [Parameter(Mandatory = $true)]
    [datetime]$CreationTime
  )

  $ticks = $CreationTime.ToUniversalTime().Ticks
  return $ticks - ($ticks % 10)
}

function Get-CimProcessIdentityKey {
  param(
    [Parameter(Mandatory = $true)]
    [object]$ProcessRecord
  )

  if ($null -eq $ProcessRecord.CreationDate) {
    throw "Process $($ProcessRecord.ProcessId) has no creation timestamp"
  }

  $creationTicks = ConvertTo-ProcessCreationIdentityTicks `
    -CreationTime ([datetime]$ProcessRecord.CreationDate)
  return ("{0}:{1}" -f ([int]$ProcessRecord.ProcessId), $creationTicks)
}

function Get-ProcessIdentityKey {
  param(
    [Parameter(Mandatory = $true)]
    [System.Diagnostics.Process]$Process
  )

  $null = $Process.Handle
  $creationTicks = ConvertTo-ProcessCreationIdentityTicks `
    -CreationTime ($Process.StartTime)
  return ("{0}:{1}" -f $Process.Id, $creationTicks)
}

function Get-WindowsProcessSnapshot {
  return @(
    Get-CimInstance `
      -ClassName Win32_Process `
      -Property "ProcessId", "ParentProcessId", "CreationDate" `
      -OperationTimeoutSec 1 `
      -ErrorAction Stop
  )
}

function Get-LiveDescendantProcessRecords {
  param(
    [Parameter(Mandatory = $true)]
    [int]$RootProcessId,

    [Parameter(Mandatory = $true)]
    [System.Collections.Generic.HashSet[int]]$KnownProcessIds
  )

  $snapshot = @(Get-WindowsProcessSnapshot)
  $records = [System.Collections.Generic.List[object]]::new()
  $seenProcessIds = [System.Collections.Generic.HashSet[int]]::new()
  $queuedProcessIds = [System.Collections.Generic.HashSet[int]]::new()
  $pendingProcessIds = [System.Collections.Generic.Queue[int]]::new()

  foreach ($knownProcessId in $KnownProcessIds) {
    if ($queuedProcessIds.Add($knownProcessId)) {
      $pendingProcessIds.Enqueue($knownProcessId)
    }
  }

  foreach ($record in $snapshot) {
    $processId = [int]$record.ProcessId
    if (
      $processId -ne $RootProcessId -and
      $KnownProcessIds.Contains($processId) -and
      $seenProcessIds.Add($processId)
    ) {
      $records.Add($record)
    }
  }

  while ($pendingProcessIds.Count -gt 0) {
    $parentProcessId = $pendingProcessIds.Dequeue()
    foreach ($record in $snapshot) {
      $processId = [int]$record.ProcessId
      if (
        [int]$record.ParentProcessId -eq $parentProcessId -and
        $processId -ne $RootProcessId -and
        $seenProcessIds.Add($processId)
      ) {
        [void]$KnownProcessIds.Add($processId)
        $records.Add($record)
        if ($queuedProcessIds.Add($processId)) {
          $pendingProcessIds.Enqueue($processId)
        }
      }
    }
  }

  return $records.ToArray()
}

function Add-RetainedProcessHandle {
  param(
    [Parameter(Mandatory = $true)]
    [object]$ProcessRecord,

    [Parameter(Mandatory = $true)]
    [System.Collections.Generic.Dictionary[string,System.Diagnostics.Process]]$RetainedProcesses,

    [Parameter(Mandatory = $true)]
    [System.Collections.Generic.HashSet[int]]$KnownProcessIds
  )

  $processId = [int]$ProcessRecord.ProcessId
  $expectedIdentity = Get-CimProcessIdentityKey -ProcessRecord $ProcessRecord
  [void]$KnownProcessIds.Add($processId)
  if ($RetainedProcesses.ContainsKey($expectedIdentity)) {
    return
  }

  $candidate = $null
  try {
    $candidate = [System.Diagnostics.Process]::GetProcessById($processId)
    $actualIdentity = Get-ProcessIdentityKey -Process $candidate
    if ($actualIdentity -ne $expectedIdentity) {
      throw "PID $processId was reused while acquiring its process handle"
    }

    $RetainedProcesses.Add($expectedIdentity, $candidate)
    $candidate = $null
    return
  }
  catch {
    $acquisitionError = $_.Exception.Message
    if ($null -ne $candidate) {
      $candidate.Dispose()
    }

    $currentRecords = @(
      Get-CimInstance `
        -ClassName Win32_Process `
        -Filter "ProcessId = $processId" `
        -Property "ProcessId", "ParentProcessId", "CreationDate" `
        -OperationTimeoutSec 1 `
        -ErrorAction Stop
    )
    if ($currentRecords.Count -eq 0) {
      return
    }
    if ($currentRecords.Count -ne 1) {
      throw "Could not prove process $processId identity after handle acquisition failed"
    }

    $currentIdentity = Get-CimProcessIdentityKey `
      -ProcessRecord $currentRecords[0]
    if ($currentIdentity -ne $expectedIdentity) {
      return
    }

    throw "Could not retain process $expectedIdentity handle: $acquisitionError"
  }
}

function Wait-RetainedProcessHandles {
  param(
    [Parameter(Mandatory = $true)]
    [System.Collections.Generic.Dictionary[string,System.Diagnostics.Process]]$RetainedProcesses,

    [Parameter(Mandatory = $true)]
    [datetime]$Deadline
  )

  $liveProcesses = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
  foreach ($process in $RetainedProcesses.Values) {
    try {
      if (-not $process.WaitForExit(0)) {
        $liveProcesses.Add($process)
      }
    }
    catch {
      throw "Could not inspect retained process $($process.Id): $($_.Exception.Message)"
    }
  }

  if ($liveProcesses.Count -eq 0) {
    return $true
  }

  $remainingMilliseconds = [int][Math]::Floor(
    ($Deadline - [datetime]::UtcNow).TotalMilliseconds
  )
  if ($remainingMilliseconds -le 0) {
    return $false
  }

  $waitSliceMilliseconds = [int][Math]::Floor(
    $remainingMilliseconds / $liveProcesses.Count
  )
  $waitSliceMilliseconds = [Math]::Max(
    1,
    [Math]::Min(100, $waitSliceMilliseconds)
  )
  foreach ($process in $liveProcesses) {
    $remainingMilliseconds = [int][Math]::Floor(
      ($Deadline - [datetime]::UtcNow).TotalMilliseconds
    )
    if ($remainingMilliseconds -le 0) {
      return $false
    }

    $waitMilliseconds = [Math]::Min(
      $waitSliceMilliseconds,
      $remainingMilliseconds
    )
    try {
      [void]$process.WaitForExit($waitMilliseconds)
    }
    catch {
      throw "Could not wait for retained process $($process.Id): $($_.Exception.Message)"
    }
  }

  foreach ($process in $RetainedProcesses.Values) {
    if (-not $process.WaitForExit(0)) {
      return $false
    }
  }
  return $true
}

function Assert-TimedOutProcessTreeQuiescent {
  param(
    [Parameter(Mandatory = $true)]
    [System.Diagnostics.Process]$RootProcess,

    [int]$TerminationTimeoutSeconds = 10
  )

  $knownProcessIds = [System.Collections.Generic.HashSet[int]]::new()
  $retainedProcesses = [System.Collections.Generic.Dictionary[string,System.Diagnostics.Process]]::new()
  $killAttempted = $false
  $killError = $null

  try {
    [void]$knownProcessIds.Add($RootProcess.Id)
    $rootIdentity = Get-ProcessIdentityKey -Process $RootProcess
    $retainedProcesses.Add($rootIdentity, $RootProcess)

    $captureDeadline = [datetime]::UtcNow.AddSeconds($TerminationTimeoutSeconds)
    $initialRecords = @(
      Get-LiveDescendantProcessRecords `
        -RootProcessId $RootProcess.Id `
        -KnownProcessIds $knownProcessIds
    )
    if ([datetime]::UtcNow -ge $captureDeadline) {
      throw "Initial descendant handle capture exceeded $TerminationTimeoutSeconds seconds"
    }
    foreach ($record in $initialRecords) {
      if ([datetime]::UtcNow -ge $captureDeadline) {
        throw "Initial descendant handle capture exceeded $TerminationTimeoutSeconds seconds"
      }
      Add-RetainedProcessHandle `
        -ProcessRecord $record `
        -RetainedProcesses $retainedProcesses `
        -KnownProcessIds $knownProcessIds
      if ([datetime]::UtcNow -gt $captureDeadline) {
        throw "Initial descendant handle capture exceeded $TerminationTimeoutSeconds seconds"
      }
    }

    $deadline = [datetime]::UtcNow.AddSeconds($TerminationTimeoutSeconds)
    $killAttempted = $true
    try {
      $RootProcess.Kill($true)
    }
    catch {
      $killError = $_.Exception.Message
    }

    while ([datetime]::UtcNow -lt $deadline) {
      $records = @(
        Get-LiveDescendantProcessRecords `
          -RootProcessId $RootProcess.Id `
          -KnownProcessIds $knownProcessIds
      )
      if ([datetime]::UtcNow -ge $deadline) {
        throw "Termination deadline expired during descendant ancestry scan"
      }
      foreach ($record in $records) {
        if ([datetime]::UtcNow -ge $deadline) {
          throw "Termination deadline expired while retaining descendant handles"
        }
        Add-RetainedProcessHandle `
          -ProcessRecord $record `
          -RetainedProcesses $retainedProcesses `
          -KnownProcessIds $knownProcessIds
        if ([datetime]::UtcNow -gt $deadline) {
          throw "Termination deadline expired while retaining descendant handles"
        }
      }

      $allRetainedProcessesExited = Wait-RetainedProcessHandles `
        -RetainedProcesses $retainedProcesses `
        -Deadline $deadline
      if ($allRetainedProcessesExited) {
        $finalRecords = @(
          Get-LiveDescendantProcessRecords `
            -RootProcessId $RootProcess.Id `
            -KnownProcessIds $knownProcessIds
        )
        if ([datetime]::UtcNow -ge $deadline) {
          throw "Termination deadline expired during the final ancestry scan"
        }
        foreach ($record in $finalRecords) {
          if ([datetime]::UtcNow -ge $deadline) {
            throw "Termination deadline expired during the final ancestry scan"
          }
          Add-RetainedProcessHandle `
            -ProcessRecord $record `
            -RetainedProcesses $retainedProcesses `
            -KnownProcessIds $knownProcessIds
          if ([datetime]::UtcNow -gt $deadline) {
            throw "Termination deadline expired during the final ancestry scan"
          }
        }

        if (
          [datetime]::UtcNow -le $deadline -and
          $finalRecords.Count -eq 0 -and
          (Wait-RetainedProcessHandles `
            -RetainedProcesses $retainedProcesses `
            -Deadline $deadline)
        ) {
          return
        }
      }
    }

    throw "Process tree did not become quiescent within $TerminationTimeoutSeconds seconds"
  }
  catch {
    $proofError = $_.Exception.Message
    if (-not $killAttempted) {
      try {
        $RootProcess.Kill($true)
      }
      catch {
        $proofError = "$proofError Best-effort Kill(true) failed: $($_.Exception.Message)"
      }
    }
    elseif ($null -ne $killError) {
      $proofError = "$proofError Kill(true) failed: $killError"
    }
    throw $proofError
  }
  finally {
    foreach ($process in $retainedProcesses.Values) {
      try {
        $process.Dispose()
      }
      catch {
        # Disposal must not replace the timeout or quiescence result.
      }
    }
  }
}

function Invoke-BoundedProcess {
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [string[]]$ArgumentList = @(),

    [Parameter(Mandatory = $true)]
    [int]$TimeoutSeconds,

    [Parameter(Mandatory = $true)]
    [string]$Description
  )

  $process = Start-Process `
    -FilePath $FilePath `
    -ArgumentList $ArgumentList `
    -PassThru

  if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    $script:cleanupProcessTreeQuiescent = $false
    try {
      Assert-TimedOutProcessTreeQuiescent -RootProcess $process
      $script:cleanupProcessTreeQuiescent = $true
    }
    catch {
      throw "$Description did not exit within $TimeoutSeconds seconds; termination was not quiescent: $($_.Exception.Message)"
    }

    throw "$Description did not exit within $TimeoutSeconds seconds; its process tree was confirmed quiescent after the termination attempt"
  }

  if ($process.ExitCode -ne 0) {
    throw "$Description exited with $($process.ExitCode)"
  }
}

function Test-InstalledArtifactsPresent {
  return (
    (Test-Path -LiteralPath $script:app) -or
    (Test-Path -LiteralPath $script:uninstaller)
  )
}

function Wait-ForInstalledFilesRemoval {
  param(
    [Parameter(Mandatory = $true)]
    [string]$ApplicationPath,

    [Parameter(Mandatory = $true)]
    [string]$UninstallerPath,

    [int]$TimeoutSeconds = 15
  )

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  while (
    (
      (Test-Path -LiteralPath $ApplicationPath) -or
      (Test-Path -LiteralPath $UninstallerPath)
    ) -and
    (Get-Date) -lt $deadline
  ) {
    Start-Sleep -Milliseconds 250
  }

  if (
    (Test-Path -LiteralPath $ApplicationPath) -or
    (Test-Path -LiteralPath $UninstallerPath)
  ) {
    throw "NSIS uninstall did not remove the installed application"
  }
}

function Remove-ValidatedInstallDirectory {
  if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
    throw "Refusing fallback cleanup because LOCALAPPDATA is unavailable"
  }

  $localAppData = [System.IO.Path]::GetFullPath($env:LOCALAPPDATA)
  $expectedInstallDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $localAppData "Codex Pulse")
  )
  if (
    -not [string]::Equals(
      $script:installDirectory,
      $expectedInstallDirectory,
      [System.StringComparison]::OrdinalIgnoreCase
    )
  ) {
    throw "Refusing fallback cleanup outside LOCALAPPDATA\Codex Pulse"
  }

  if (-not (Test-Path -LiteralPath $script:installDirectory)) {
    return
  }

  $installDirectoryItem = Get-Item -LiteralPath $script:installDirectory -Force
  if (-not $installDirectoryItem.PSIsContainer) {
    throw "Refusing fallback cleanup because the install path is not a directory"
  }
  $isReparsePoint = (
    $installDirectoryItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint
  ) -ne 0
  if ($isReparsePoint) {
    throw "Refusing fallback cleanup through a reparse point"
  }

  $resolvedLocalAppData = (
    Resolve-Path -LiteralPath $localAppData
  ).ProviderPath
  $resolvedInstallDirectory = (
    Resolve-Path -LiteralPath $script:installDirectory
  ).ProviderPath
  $resolvedExpectedInstallDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $resolvedLocalAppData "Codex Pulse")
  )
  if (
    -not [string]::Equals(
      $resolvedInstallDirectory,
      $resolvedExpectedInstallDirectory,
      [System.StringComparison]::OrdinalIgnoreCase
    )
  ) {
    throw "Refusing fallback cleanup because the resolved install path is unexpected"
  }

  Remove-Item -LiteralPath $resolvedInstallDirectory -Recurse -Force
}

function Invoke-InstallationCleanup {
  $cleanupIssues = [System.Collections.Generic.List[string]]::new()

  if (-not $script:cleanupProcessTreeQuiescent) {
    [void]$cleanupIssues.Add(
      "Cleanup mutation was skipped because timed-out process tree quiescence was not proven"
    )
    if (Test-InstalledArtifactsPresent) {
      [void]$cleanupIssues.Add(
        "Cleanup left CodexPulse.exe or uninstall.exe installed"
      )
    }
    foreach ($issue in $cleanupIssues) {
      Write-Warning "Best-effort cleanup failed: $issue"
    }
    return
  }

  if (Test-Path -LiteralPath $script:uninstaller -PathType Leaf) {
    try {
      Invoke-BoundedProcess `
        -FilePath $script:uninstaller `
        -ArgumentList "/S" `
        -TimeoutSeconds 120 `
        -Description "NSIS cleanup uninstaller"
      Wait-ForInstalledFilesRemoval `
        -ApplicationPath $script:app `
        -UninstallerPath $script:uninstaller
    }
    catch {
      [void]$cleanupIssues.Add(
        "Silent uninstall failed: $($_.Exception.Message)"
      )
    }
  }

  if (-not $script:cleanupProcessTreeQuiescent) {
    [void]$cleanupIssues.Add(
      "Guarded fallback removal was skipped because cleanup process tree quiescence was not proven"
    )
  }
  elseif (Test-InstalledArtifactsPresent) {
    try {
      Remove-ValidatedInstallDirectory
    }
    catch {
      [void]$cleanupIssues.Add(
        "Guarded fallback removal failed: $($_.Exception.Message)"
      )
    }
  }

  if (Test-InstalledArtifactsPresent) {
    [void]$cleanupIssues.Add(
      "Cleanup left CodexPulse.exe or uninstall.exe installed"
    )
  }

  foreach ($issue in $cleanupIssues) {
    Write-Warning "Best-effort cleanup failed: $issue"
  }
}

if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
  throw "LOCALAPPDATA is required to verify the current-user installation"
}
if (-not [System.IO.Path]::IsPathFullyQualified($env:LOCALAPPDATA)) {
  throw "LOCALAPPDATA must be an absolute path"
}

$setups = @(Get-ChildItem -Path $BundleDirectory -Filter "*-setup.exe" -File)
if ($setups.Count -ne 1) {
  throw "Expected exactly one NSIS setup executable, found $($setups.Count)"
}

$script:installDirectory = [System.IO.Path]::GetFullPath(
  (Join-Path $env:LOCALAPPDATA "Codex Pulse")
)
$script:app = Join-Path $script:installDirectory "CodexPulse.exe"
$script:uninstaller = Join-Path $script:installDirectory "uninstall.exe"

try {
  Invoke-BoundedProcess `
    -FilePath $setups[0].FullName `
    -ArgumentList "/S" `
    -TimeoutSeconds 120 `
    -Description "NSIS setup"

  if (
    -not (Test-Path -LiteralPath $script:app) -or
    -not (Test-Path -LiteralPath $script:uninstaller)
  ) {
    throw "NSIS did not install CodexPulse.exe and uninstall.exe"
  }

  $image = [System.IO.File]::ReadAllBytes($script:app)
  if ($image.Length -lt 0x40) {
    throw "Installed CodexPulse.exe is too small to contain a PE header"
  }

  $peOffset = [BitConverter]::ToInt32($image, 0x3c)
  if ($peOffset -lt 0 -or $peOffset + 94 -gt $image.Length) {
    throw "Installed CodexPulse.exe has an invalid PE header offset"
  }

  $signature = [System.Text.Encoding]::ASCII.GetString($image, $peOffset, 4)
  if ($signature -ne "PE`0`0") {
    throw "Installed CodexPulse.exe does not have a valid PE signature"
  }

  $subsystem = [BitConverter]::ToUInt16($image, $peOffset + 24 + 68)
  if ($subsystem -ne 2) {
    throw "Expected Windows GUI subsystem 2, found $subsystem"
  }

  Invoke-BoundedProcess `
    -FilePath $script:app `
    -ArgumentList "__hook" `
    -TimeoutSeconds 15 `
    -Description "Installed __hook helper"

  Invoke-BoundedProcess `
    -FilePath $script:uninstaller `
    -ArgumentList "/S" `
    -TimeoutSeconds 120 `
    -Description "NSIS uninstaller"

  Wait-ForInstalledFilesRemoval `
    -ApplicationPath $script:app `
    -UninstallerPath $script:uninstaller
}
finally {
  try {
    Invoke-InstallationCleanup
  }
  catch {
    Write-Warning "Best-effort cleanup failed unexpectedly: $($_.Exception.Message)"
  }
}
