[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string] $ReleaseDir,

  [switch] $LaunchUi,

  [ValidateRange(1, 60)]
  [int] $LaunchSeconds = 5
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Smoke {
  param([Parameter(Mandatory = $true)][string] $Message)
  Write-Host "[gateway-ui-smoke] $Message"
}

function Resolve-SmokeReleaseDir {
  param([Parameter(Mandatory = $true)][string] $Path)
  $resolved = Resolve-Path -LiteralPath $Path -ErrorAction Stop
  $item = Get-Item -LiteralPath $resolved.Path
  if (-not $item.PSIsContainer) {
    throw "ReleaseDir must be a directory: $Path"
  }
  return $item.FullName
}

function Get-RelativeUnixPath {
  param(
    [Parameter(Mandatory = $true)][string] $BasePath,
    [Parameter(Mandatory = $true)][string] $Path
  )

  $baseFull = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
  $pathFull = [System.IO.Path]::GetFullPath($Path)
  $baseUri = New-Object System.Uri($baseFull)
  $pathUri = New-Object System.Uri($pathFull)
  return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Assert-SafePackageRelativePath {
  param([Parameter(Mandatory = $true)][string] $Path)

  if ([string]::IsNullOrWhiteSpace($Path) -or
    $Path.Contains("\") -or
    $Path.StartsWith("/") -or
    $Path.Contains(":") -or
    (($Path -split "/") -contains "..")) {
    throw "Package manifest contains an unsafe relative path: $Path"
  }
  return $Path
}

function Read-ChecksumIndex {
  param([Parameter(Mandatory = $true)][string] $Path)

  $entries = [System.Collections.Generic.Dictionary[string, string]]::new(
    [System.StringComparer]::Ordinal
  )
  $lineNumber = 0
  foreach ($line in (Get-Content -LiteralPath $Path -Encoding ASCII)) {
    $lineNumber++
    if ([string]::IsNullOrWhiteSpace($line)) {
      continue
    }
    if ($line -notmatch "^(?<digest>[0-9a-fA-F]{64})  (?<path>[^\r\n]+)$") {
      throw "Invalid checksums.sha256 line $lineNumber (expected digest + two spaces + relative path)"
    }
    $digest = $Matches["digest"].ToLowerInvariant()
    $relative = Assert-SafePackageRelativePath -Path $Matches["path"]
    if ($entries.ContainsKey($relative)) {
      throw "Duplicate checksums.sha256 path: $relative"
    }
    $entries.Add($relative, $digest)
  }
  if ($entries.Count -eq 0) {
    throw "checksums.sha256 contains no entries"
  }
  return $entries
}

function Get-ObjectPropertyValue {
  param(
    [Parameter(Mandatory = $true)] $Object,
    [Parameter(Mandatory = $true)][string] $Name
  )

  $property = $Object.PSObject.Properties[$Name]
  if ($null -eq $property) {
    return $null
  }
  return $property.Value
}

function Assert-PackagedReleaseIntegrity {
  param([Parameter(Mandatory = $true)][string] $ReleaseRoot)

  $manifestPath = Join-Path $ReleaseRoot "manifest.json"
  $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $checksumsRelative = [string](Get-ObjectPropertyValue -Object $manifest -Name "checksums")
  if ($checksumsRelative -ne "checksums.sha256") {
    throw "Packaged manifest must point to checksums.sha256 exactly"
  }
  $checksumsPath = Join-Path $ReleaseRoot (Assert-SafePackageRelativePath -Path $checksumsRelative)
  $checksumEntries = Read-ChecksumIndex -Path $checksumsPath

  $actualFiles = [System.Collections.Generic.Dictionary[string, string]]::new(
    [System.StringComparer]::Ordinal
  )
  foreach ($file in @(Get-ChildItem -LiteralPath $ReleaseRoot -Recurse -File -Force)) {
    $relative = Get-RelativeUnixPath -BasePath $ReleaseRoot -Path $file.FullName
    if ($relative -eq "checksums.sha256") {
      continue
    }
    Assert-SafePackageRelativePath -Path $relative | Out-Null
    $actualFiles[$relative] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  }
  if ($actualFiles.Count -ne $checksumEntries.Count) {
    throw "checksums.sha256 entry count does not match packaged files"
  }
  foreach ($relative in $checksumEntries.Keys) {
    if (-not $actualFiles.ContainsKey($relative)) {
      throw "checksums.sha256 references a missing packaged file: $relative"
    }
    if ($actualFiles[$relative] -ne $checksumEntries[$relative]) {
      throw "Packaged checksum mismatch: $relative"
    }
  }

  foreach ($record in @(Get-ObjectPropertyValue -Object $manifest -Name "files")) {
    $relative = Assert-SafePackageRelativePath -Path ([string](Get-ObjectPropertyValue -Object $record -Name "path"))
    if (-not $actualFiles.ContainsKey($relative)) {
      throw "Manifest references a missing packaged file: $relative"
    }
    $payloadPath = Join-Path $ReleaseRoot $relative
    $item = Get-Item -LiteralPath $payloadPath
    $recordBytes = [int64](Get-ObjectPropertyValue -Object $record -Name "bytes")
    $recordSha = [string](Get-ObjectPropertyValue -Object $record -Name "sha256")
    if ($item.Length -ne $recordBytes -or $actualFiles[$relative] -ne $recordSha.ToLowerInvariant()) {
      throw "Manifest artifact record does not match payload: $relative"
    }
    if ($checksumEntries[$relative] -ne $actualFiles[$relative]) {
      throw "Manifest artifact is not represented by the exact checksum entry: $relative"
    }
  }

  if (-not $checksumEntries.ContainsKey("manifest.json")) {
    throw "checksums.sha256 must contain an exact manifest.json entry"
  }
  $manifestListedPaths = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::Ordinal
  )
  foreach ($record in @(Get-ObjectPropertyValue -Object $manifest -Name "files")) {
    $manifestListedPaths.Add([string](Get-ObjectPropertyValue -Object $record -Name "path")) | Out-Null
  }
  foreach ($relative in $checksumEntries.Keys) {
    if ($relative -ne "manifest.json" -and -not $manifestListedPaths.Contains($relative)) {
      throw "checksums.sha256 contains a payload not listed in manifest.files: $relative"
    }
  }

  return [pscustomobject]@{
    manifest = $manifest
    checksumEntries = $checksumEntries
  }
}

function Test-PathOwnedByRelease {
  param(
    [Parameter(Mandatory = $true)] $Process,
    [Parameter(Mandatory = $true)][string] $ReleaseRoot,
    [Parameter(Mandatory = $true)][string] $ExpectedGatewayPath
  )

  $executablePath = [string] $Process.ExecutablePath
  $commandLine = [string] $Process.CommandLine
  if ([string]::IsNullOrWhiteSpace($executablePath) -or
    [string]::IsNullOrWhiteSpace($commandLine)) {
    return $false
  }

  try {
    $normalizedExecutablePath = [System.IO.Path]::GetFullPath($executablePath)
    $normalizedReleaseRoot = [System.IO.Path]::GetFullPath($ReleaseRoot).TrimEnd("\", "/")
    $normalizedExpectedGatewayPath = [System.IO.Path]::GetFullPath($ExpectedGatewayPath)
  } catch {
    return $false
  }

  $releasePrefix = $normalizedReleaseRoot + [System.IO.Path]::DirectorySeparatorChar
  if (-not $normalizedExpectedGatewayPath.StartsWith($releasePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    return $false
  }
  if (-not [string]::Equals(
      $normalizedExecutablePath,
      $normalizedExpectedGatewayPath,
      [System.StringComparison]::OrdinalIgnoreCase
    )) {
    return $false
  }

  return $commandLine.IndexOf($normalizedExpectedGatewayPath, [System.StringComparison]::OrdinalIgnoreCase) -ge 0
}

function Get-ReleaseOwnedGatewayProcesses {
  param(
    [Parameter(Mandatory = $true)][string] $ReleaseRoot,
    [Parameter(Mandatory = $true)][string] $ExpectedGatewayPath
  )

  $ownershipContext = [pscustomobject]@{
    ReleaseRoot = $ReleaseRoot
    ExpectedGatewayPath = $ExpectedGatewayPath
  }
  $cimProcesses = @(Get-CimInstance -ClassName Win32_Process -Filter "Name = 'gateway.exe'" -ErrorAction SilentlyContinue)
  foreach ($candidate in $cimProcesses) {
    if (Test-PathOwnedByRelease `
        -Process $candidate `
        -ReleaseRoot $ownershipContext.ReleaseRoot `
        -ExpectedGatewayPath $ownershipContext.ExpectedGatewayPath) {
      $candidate
    }
  }
}

function Assert-ReleaseArtifact {
  param(
    [Parameter(Mandatory = $true)] $Record,
    [Parameter(Mandatory = $true)][string] $ReleaseRoot,
    [Parameter(Mandatory = $true)] $ChecksumEntries
  )

  $artifactPath = Join-Path $ReleaseRoot $Record.path
  if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
    throw "Missing release artifact: $($Record.path)"
  }

  $item = Get-Item -LiteralPath $artifactPath
  $hash = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $bytesMatch = $item.Length -eq [int64] $Record.bytes
  $shaMatch = $hash -eq [string] $Record.sha256
  Write-Smoke "$($Record.name)|bytesMatch=$bytesMatch|shaMatch=$shaMatch|bytes=$($item.Length)|sha256=$hash"

  if (-not $bytesMatch) {
    throw "Artifact byte length mismatch for $($Record.name)"
  }
  if (-not $shaMatch) {
    throw "Artifact sha256 mismatch for $($Record.name)"
  }
  $relative = [string] $Record.path
  if (-not $ChecksumEntries.ContainsKey($relative)) {
    throw "checksums.sha256 does not contain the exact path $relative"
  }
  if ($ChecksumEntries[$relative] -ne $hash) {
    throw "checksums.sha256 digest mismatch for $relative"
  }
}

function Invoke-OptionalUiLaunchSmoke {
  param(
    [Parameter(Mandatory = $true)][string] $ReleaseRoot,
    [Parameter(Mandatory = $true)][int] $Seconds
  )

  $uiExe = Join-Path $ReleaseRoot "gateway-ui.exe"
  $expectedGatewayPath = [System.IO.Path]::GetFullPath((Join-Path $ReleaseRoot "gateway.exe"))
  $baselineGatewayProcesses = @(Get-ReleaseOwnedGatewayProcesses `
    -ReleaseRoot $ReleaseRoot `
    -ExpectedGatewayPath $expectedGatewayPath)
  $baselineGatewayProcessIds = @($baselineGatewayProcesses | ForEach-Object { [int] $_.ProcessId })
  $uiProcess = $null
  $newGatewayProcesses = @()
  $newGatewayProcessIds = @()

  try {
    Write-Smoke "launching gateway-ui.exe for $Seconds seconds; UI may auto-start gateway.exe"
    $uiProcess = Start-Process -FilePath $uiExe -WorkingDirectory $ReleaseRoot -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds $Seconds

    if ($uiProcess.HasExited) {
      throw "gateway-ui.exe exited during smoke window with code $($uiProcess.ExitCode)"
    }

    $afterGatewayProcesses = @(Get-ReleaseOwnedGatewayProcesses `
      -ReleaseRoot $ReleaseRoot `
      -ExpectedGatewayPath $expectedGatewayPath)
    $newGatewayProcesses = @($afterGatewayProcesses | Where-Object {
      $baselineGatewayProcessIds -notcontains [int] $_.ProcessId
    })
    $newGatewayProcessIds = @($newGatewayProcesses | ForEach-Object { [int] $_.ProcessId })
    if ($newGatewayProcessIds.Count -gt 0) {
      Write-Smoke "gateway-ui.exe auto-started release-owned gateway.exe process ids: $($newGatewayProcessIds -join ', ')"
    } else {
      Write-Smoke "gateway-ui.exe remained open; no release-owned gateway.exe process was observed during the smoke window"
    }

    Write-Smoke "optional UI launch smoke passed; release-owned gateway.exe sidecars will be cleaned"
  } finally {
    if ($null -ne $uiProcess -and -not $uiProcess.HasExited) {
      Stop-Process -Id $uiProcess.Id -Force -ErrorAction SilentlyContinue
      Write-Smoke "closed gateway-ui.exe smoke process: pid=$($uiProcess.Id)"
    }
    # A failed UI launch can leave a sidecar behind. Re-scan after closing the
    # shell and terminate only Gateway PIDs that did not exist at the start.
    $remainingGatewayProcesses = @(Get-ReleaseOwnedGatewayProcesses `
      -ReleaseRoot $ReleaseRoot `
      -ExpectedGatewayPath $expectedGatewayPath |
      Where-Object { $baselineGatewayProcessIds -notcontains [int] $_.ProcessId })
    foreach ($gatewayProcess in $remainingGatewayProcesses) {
      Stop-Process -Id ([int] $gatewayProcess.ProcessId) -Force -ErrorAction SilentlyContinue
      Write-Smoke "cleaned release-owned gateway.exe smoke process: pid=$($gatewayProcess.ProcessId)"
    }
  }
}

$resolvedReleaseDir = Resolve-SmokeReleaseDir -Path $ReleaseDir
$manifestPath = Join-Path $resolvedReleaseDir "manifest.json"
$checksumsPath = Join-Path $resolvedReleaseDir "checksums.sha256"

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
  throw "Missing manifest.json in release directory: $resolvedReleaseDir"
}
if (-not (Test-Path -LiteralPath $checksumsPath -PathType Leaf)) {
  throw "Missing checksums.sha256 in release directory: $resolvedReleaseDir"
}

$integrity = Assert-PackagedReleaseIntegrity -ReleaseRoot $resolvedReleaseDir
$manifest = $integrity.manifest
$checksumEntries = $integrity.checksumEntries
$exeRecords = @($manifest.exes)
$expectedExeNames = @("gateway.exe", "gateway-ui.exe")

foreach ($expectedExeName in $expectedExeNames) {
  if (-not ($exeRecords | Where-Object { $_.name -eq $expectedExeName })) {
    throw "manifest.json does not contain expected artifact: $expectedExeName"
  }
}

foreach ($record in $exeRecords) {
  Assert-ReleaseArtifact -Record $record -ReleaseRoot $resolvedReleaseDir -ChecksumEntries $checksumEntries
}

if ($LaunchUi) {
  Invoke-OptionalUiLaunchSmoke -ReleaseRoot $resolvedReleaseDir -Seconds $LaunchSeconds
} else {
  Write-Smoke "artifact smoke passed; pass -LaunchUi to also launch gateway-ui.exe briefly"
}
