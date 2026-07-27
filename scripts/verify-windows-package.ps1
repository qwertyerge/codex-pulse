param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$BundleDirectory
)

$ErrorActionPreference = "Stop"

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
    $terminationTimeoutSeconds = 10
    $terminationError = $null

    try {
      $process.Kill($true)
    }
    catch {
      $terminationError = $_.Exception.Message
    }

    $becameQuiescent = $process.WaitForExit($terminationTimeoutSeconds * 1000)
    if ($null -ne $terminationError -or -not $becameQuiescent) {
      $detail = if ($null -ne $terminationError) {
        ": $terminationError"
      }
      else {
        ""
      }
      throw "$Description exceeded $TimeoutSeconds seconds and its process tree could not be made quiescent within $terminationTimeoutSeconds seconds$detail"
    }

    throw "$Description did not exit within $TimeoutSeconds seconds; its process tree was terminated"
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

  if (Test-InstalledArtifactsPresent) {
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
