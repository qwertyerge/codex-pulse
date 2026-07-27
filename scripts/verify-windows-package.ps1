param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$BundleDirectory
)

$ErrorActionPreference = "Stop"
$script:cleanupMutationAllowed = $true

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
    $script:cleanupMutationAllowed = $false
    $terminationTimeoutSeconds = 10
    $killError = $null
    $rootWaitError = $null
    $rootExitObserved = $false

    try {
      $process.Kill($true)
    }
    catch {
      $killError = $_.Exception.Message
    }

    try {
      $rootExitObserved = $process.WaitForExit(
        $terminationTimeoutSeconds * 1000
      )
    }
    catch {
      $rootWaitError = $_.Exception.Message
    }

    $diagnostics = [System.Collections.Generic.List[string]]::new()
    if ($null -ne $killError) {
      [void]$diagnostics.Add("Kill(true) failed: $killError")
    }
    if ($null -ne $rootWaitError) {
      [void]$diagnostics.Add("Root process wait failed: $rootWaitError")
    }
    elseif ($rootExitObserved) {
      [void]$diagnostics.Add(
        "Root process exit was observed; descendant state was not verified"
      )
    }
    else {
      [void]$diagnostics.Add(
        "Root process exit was not observed within $terminationTimeoutSeconds seconds"
      )
    }

    $diagnosticText = $diagnostics -join ". "
    throw "$Description did not exit within $TimeoutSeconds seconds; cleanup mutation is disabled and timeout residue is left for ephemeral hosted runner disposal. $diagnosticText"
  }

  if ($process.ExitCode -ne 0) {
    throw "$Description exited with $($process.ExitCode)"
  }
}

function Read-ProcessDiagnostic {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    return "<not created>"
  }

  $content = Get-Content -LiteralPath $Path -Raw
  if ([string]::IsNullOrWhiteSpace($content)) {
    return "<empty>"
  }
  return $content.Trim()
}

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class CodexPulseWindowProbe
{
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetClientRect(IntPtr window, out Rect rect);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool IsZoomed(IntPtr window);

    [DllImport("user32.dll")]
    public static extern uint GetDpiForWindow(IntPtr window);

    [DllImport("user32.dll")]
    public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
"@

function Get-ApplicationWindowDiagnostic {
  param(
    [Parameter(Mandatory = $true)]
    [IntPtr]$Handle
  )

  $previousDpiContext = [CodexPulseWindowProbe]::SetThreadDpiAwarenessContext(
    [IntPtr]::new(-4)
  )
  try {
    $rect = [CodexPulseWindowProbe+Rect]::new()
    if (-not [CodexPulseWindowProbe]::GetClientRect($Handle, [ref]$rect)) {
      $win32Error = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
      throw "Failed to read the application client area; Win32 error $win32Error"
    }

    $dpi = [CodexPulseWindowProbe]::GetDpiForWindow($Handle)
    if ($dpi -eq 0) {
      $dpi = 96
    }
  }
  finally {
    if ($previousDpiContext -ne [IntPtr]::Zero) {
      [void][CodexPulseWindowProbe]::SetThreadDpiAwarenessContext(
        $previousDpiContext
      )
    }
  }
  $physicalWidth = $rect.Right - $rect.Left
  $physicalHeight = $rect.Bottom - $rect.Top

  return [pscustomobject]@{
    Maximized = [CodexPulseWindowProbe]::IsZoomed($Handle)
    Dpi = $dpi
    LogicalClientWidth = [Math]::Round(
      ($physicalWidth * 96.0) / $dpi,
      2
    )
    LogicalClientHeight = [Math]::Round(
      ($physicalHeight * 96.0) / $dpi,
      2
    )
  }
}

function Assert-InstalledShortcuts {
  $shortcutPaths = @(
    (
      Join-Path `
        ([Environment]::GetFolderPath(
          [Environment+SpecialFolder]::DesktopDirectory
        )) `
        "Codex Pulse.lnk"
    ),
    (
      Join-Path `
        ([Environment]::GetFolderPath(
          [Environment+SpecialFolder]::Programs
        )) `
        "Codex Pulse.lnk"
    )
  )
  $shell = New-Object -ComObject WScript.Shell

  foreach ($path in $shortcutPaths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
      throw "Expected installed shortcut at $path"
    }

    $shortcut = $shell.CreateShortcut($path)
    $target = [System.IO.Path]::GetFullPath($shortcut.TargetPath)
    if (
      -not [string]::Equals(
        $target,
        $script:app,
        [System.StringComparison]::OrdinalIgnoreCase
      )
    ) {
      throw "Installed shortcut $path targets $target instead of $script:app"
    }
    if (-not [string]::IsNullOrWhiteSpace($shortcut.Arguments)) {
      throw "Installed shortcut $path unexpectedly supplies arguments: $($shortcut.Arguments)"
    }
  }
}

function Invoke-ApplicationStartupSmoke {
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [int]$WindowTimeoutSeconds = 15,

    [int]$SurvivalSeconds = 3
  )

  $diagnosticDirectory = Join-Path `
    $env:TEMP `
    ("codex-pulse-startup-smoke-" + [guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Path $diagnosticDirectory | Out-Null
  $stdout = Join-Path $diagnosticDirectory "stdout.log"
  $stderr = Join-Path $diagnosticDirectory "stderr.log"
  $process = $null

  try {
    $process = Start-Process `
      -FilePath $FilePath `
      -PassThru `
      -RedirectStandardOutput $stdout `
      -RedirectStandardError $stderr

    $windowDeadline = (Get-Date).AddSeconds($WindowTimeoutSeconds)
    $windowObserved = $false
    while ((Get-Date) -lt $windowDeadline) {
      if ($process.HasExited) {
        $process.WaitForExit()
        $diagnostic = Read-ProcessDiagnostic -Path $stderr
        throw "Installed application exited before startup completed with code $($process.ExitCode). stderr: $diagnostic"
      }

      $process.Refresh()
      if ($process.MainWindowHandle -ne [IntPtr]::Zero) {
        $windowObserved = $true
        break
      }
      Start-Sleep -Milliseconds 250
    }

    if (-not $windowObserved) {
      $diagnostic = Read-ProcessDiagnostic -Path $stderr
      throw "Installed application did not create a main window within $WindowTimeoutSeconds seconds. stderr: $diagnostic"
    }

    $survivalDeadline = (Get-Date).AddSeconds($SurvivalSeconds)
    while ((Get-Date) -lt $survivalDeadline) {
      if ($process.HasExited) {
        $process.WaitForExit()
        $diagnostic = Read-ProcessDiagnostic -Path $stderr
        throw "Installed application exited during the $SurvivalSeconds-second survival window with code $($process.ExitCode). stderr: $diagnostic"
      }
      Start-Sleep -Milliseconds 250
    }

    $process.Refresh()
    $windowDiagnostic = Get-ApplicationWindowDiagnostic `
      -Handle $process.MainWindowHandle
    $windowSummary = (
      "client={0}x{1} logical pixels, dpi={2}" -f
      $windowDiagnostic.LogicalClientWidth,
      $windowDiagnostic.LogicalClientHeight,
      $windowDiagnostic.Dpi
    )
    if ($windowDiagnostic.Maximized) {
      throw "Installed application window is maximized; expected a compact utility window ($windowSummary)"
    }
    if ($windowDiagnostic.LogicalClientWidth -gt 480.0) {
      throw "Installed application client width exceeds 480 logical pixels ($windowSummary)"
    }
    if (
      [Math]::Abs($windowDiagnostic.LogicalClientWidth - 360.0) -gt 1.0 -or
      [Math]::Abs($windowDiagnostic.LogicalClientHeight - 420.0) -gt 1.0
    ) {
      throw "Installed application did not start at the expected 360x420 logical client size ($windowSummary)"
    }
    Write-Host "Installed application startup: $windowSummary"

    $terminalProcessNames = @(
      "cmd.exe",
      "OpenConsole.exe",
      "powershell.exe",
      "pwsh.exe",
      "WindowsTerminal.exe"
    )
    $terminalChildren = @(
      Get-CimInstance Win32_Process |
        Where-Object {
          $_.ParentProcessId -eq $process.Id -and
          $_.Name -in $terminalProcessNames
        } |
        Select-Object -ExpandProperty Name
    )
    if ($terminalChildren.Count -ne 0) {
      throw "Installed application spawned terminal processes: $($terminalChildren -join ', ')"
    }
  }
  finally {
    if ($null -ne $process -and -not $process.HasExited) {
      $taskkill = Join-Path $env:SystemRoot "System32\taskkill.exe"
      & $taskkill /PID $process.Id /T /F | Out-Null
      $taskkillExitCode = $LASTEXITCODE
      $process.Refresh()
      if ($taskkillExitCode -ne 0 -and -not $process.HasExited) {
        $script:cleanupMutationAllowed = $false
        throw "Failed to stop the startup-smoke process tree for PID $($process.Id); taskkill exited with $taskkillExitCode"
      }

      if (-not $process.WaitForExit(10000)) {
        $script:cleanupMutationAllowed = $false
        throw "Startup-smoke process tree for PID $($process.Id) did not exit within 10 seconds"
      }
    }
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

  if (-not $script:cleanupMutationAllowed) {
    Write-Warning "Cleanup mutation was skipped after a process timeout; residue is left for ephemeral hosted runner disposal"
    if (Test-InstalledArtifactsPresent) {
      Write-Warning "CodexPulse.exe or uninstall.exe remains after the timeout"
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

  if (-not $script:cleanupMutationAllowed) {
    [void]$cleanupIssues.Add(
      "Guarded fallback removal was skipped after the cleanup uninstaller timed out; residue is left for ephemeral hosted runner disposal"
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

  Assert-InstalledShortcuts

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

  Invoke-ApplicationStartupSmoke -FilePath $script:app

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
