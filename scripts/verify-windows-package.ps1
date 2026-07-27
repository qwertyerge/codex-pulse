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
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    throw "$Description did not exit within $TimeoutSeconds seconds"
  }

  if ($process.ExitCode -ne 0) {
    throw "$Description exited with $($process.ExitCode)"
  }
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
    ((Test-Path $ApplicationPath) -or (Test-Path $UninstallerPath)) -and
    (Get-Date) -lt $deadline
  ) {
    Start-Sleep -Milliseconds 250
  }

  if ((Test-Path $ApplicationPath) -or (Test-Path $UninstallerPath)) {
    throw "NSIS uninstall did not remove the installed application"
  }
}

if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
  throw "LOCALAPPDATA is required to verify the current-user installation"
}

$setups = @(Get-ChildItem -Path $BundleDirectory -Filter "*-setup.exe" -File)
if ($setups.Count -ne 1) {
  throw "Expected exactly one NSIS setup executable, found $($setups.Count)"
}

$installDirectory = Join-Path $env:LOCALAPPDATA "Codex Pulse"
$app = Join-Path $installDirectory "CodexPulse.exe"
$uninstaller = Join-Path $installDirectory "uninstall.exe"

try {
  Invoke-BoundedProcess `
    -FilePath $setups[0].FullName `
    -ArgumentList "/S" `
    -TimeoutSeconds 120 `
    -Description "NSIS setup"

  if (-not (Test-Path $app) -or -not (Test-Path $uninstaller)) {
    throw "NSIS did not install CodexPulse.exe and uninstall.exe"
  }

  $image = [System.IO.File]::ReadAllBytes($app)
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
    -FilePath $app `
    -ArgumentList "__hook" `
    -TimeoutSeconds 15 `
    -Description "Installed __hook helper"

  Invoke-BoundedProcess `
    -FilePath $uninstaller `
    -ArgumentList "/S" `
    -TimeoutSeconds 120 `
    -Description "NSIS uninstaller"

  Wait-ForInstalledFilesRemoval `
    -ApplicationPath $app `
    -UninstallerPath $uninstaller
}
finally {
  if (Test-Path $uninstaller) {
    try {
      Invoke-BoundedProcess `
        -FilePath $uninstaller `
        -ArgumentList "/S" `
        -TimeoutSeconds 120 `
        -Description "NSIS cleanup uninstaller"
      Wait-ForInstalledFilesRemoval `
        -ApplicationPath $app `
        -UninstallerPath $uninstaller
    }
    catch {
      Write-Warning "Best-effort cleanup failed: $($_.Exception.Message)"
    }
  }
}
