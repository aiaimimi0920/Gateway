param(
  [string]$LineId,
  [switch]$All,
  [switch]$ListOnly,
  [switch]$AsJson,
  [switch]$SkipCargo,
  [switch]$LibOnly,
  [string]$SharedCargoTargetDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$GatewayRoot = Split-Path -Parent $PSScriptRoot
$ManifestRoot = Join-Path $GatewayRoot "manifests\lines"
$Validator = Join-Path $GatewayRoot "tools\validate-gateway-line-manifests.py"
$CargoToml = Join-Path $GatewayRoot "Cargo.toml"

function Get-OptionalProperty {
  param(
    [object]$Object,
    [string]$Name
  )

  if ($null -eq $Object) {
    return $null
  }
  $property = $Object.PSObject.Properties[$Name]
  if ($null -eq $property) {
    return $null
  }
  $property.Value
}

function Get-GatewayLineManifests {
  Get-ChildItem -LiteralPath $ManifestRoot -Recurse -Filter "*.json" |
    Sort-Object FullName |
    ForEach-Object {
      $manifest = Get-Content -LiteralPath $_.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
      $familyFeatures = Get-OptionalProperty -Object $manifest.compilation -Name "familyCommonFeatures"
      [pscustomobject]@{
        Id = [string]$manifest.id
        Path = $_.FullName
        ServiceProviderKey = [string]$manifest.identity.serviceProviderKey
        ProviderSurfaceKey = [string]$manifest.identity.providerSurfaceKey
        ProtocolProfile = [string]$manifest.identity.protocolProfile
        Adapter = [string]$manifest.identity.adapter
        LineFeature = [string](Get-OptionalProperty -Object $manifest.compilation -Name "lineFeature")
        FamilyFeatures = @($familyFeatures)
        UseNoDefaultFeatures = [bool]$manifest.verification.useNoDefaultFeatures
        CargoTargetDirSlug = [string]$manifest.verification.recommendedCargoTargetDirSlug
        FocusedCargoFilters = @($manifest.verification.focusedCargoFilters)
      }
    }
}

function Write-ListOnlyOutput {
  param([array]$Lines)

  if ($AsJson) {
    $Lines | ConvertTo-Json -Depth 8
    return
  }

  foreach ($line in $Lines) {
    $feature = if ([string]::IsNullOrWhiteSpace($line.LineFeature)) { "-" } else { $line.LineFeature }
    "{0}`t{1}`t{2}`t{3}`t{4}" -f $line.Id, $line.ServiceProviderKey, $line.ProviderSurfaceKey, $line.ProtocolProfile, $feature
  }
}

function Find-GatewayLine {
  param(
    [array]$Lines,
    [string]$Lookup
  )

  $needle = $Lookup.Trim()
  $matches = @(
    $Lines | Where-Object {
      $_.Id -eq $needle -or
      $_.LineFeature -eq $needle -or
      $_.ProtocolProfile -eq $needle -or
      $_.ProviderSurfaceKey -eq $needle
    }
  )

  if ($matches.Count -eq 0) {
    throw "Gateway line not found: $Lookup"
  }
  if ($matches.Count -gt 1) {
    $ids = ($matches | ForEach-Object { $_.Id }) -join ", "
    throw "Gateway line lookup is ambiguous: $Lookup matched $ids"
  }

  $matches[0]
}

function Invoke-CheckedCommand {
  param([string[]]$Command)

  Write-Host ">>> $($Command -join ' ')"
  $isCargoTest = $Command.Count -ge 2 -and $Command[0] -eq "cargo" -and $Command[1] -eq "test"
  $passedTestCount = 0
  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    & $Command[0] @($Command | Select-Object -Skip 1) 2>&1 | ForEach-Object {
      Write-Host $_
      if ($isCargoTest -and [string]$_ -match '^\s*test result: ok\.\s+(\d+)\s+passed;') {
        $passedTestCount += [int]$Matches[1]
      }
    }
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed with exit code ${LASTEXITCODE}: $($Command -join ' ')"
  }
  if ($isCargoTest -and $passedTestCount -eq 0) {
    throw "Cargo command executed no tests: $($Command -join ' ')"
  }
}

function Invoke-GatewayLineVerification {
  param([object]$Line)

  Invoke-CheckedCommand @("python", $Validator, "--manifest", $Line.Path)

  if (-not $SkipCargo) {
    $features = @()
    if (-not [string]::IsNullOrWhiteSpace($Line.LineFeature)) {
      $features += $Line.LineFeature
    }
    $features += @($Line.FamilyFeatures | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
    $featureText = ($features | Select-Object -Unique) -join ","

    $previousTargetDir = $env:CARGO_TARGET_DIR
    try {
      if (-not [string]::IsNullOrWhiteSpace($SharedCargoTargetDir)) {
        New-Item -ItemType Directory -Force -Path $SharedCargoTargetDir | Out-Null
        $env:CARGO_TARGET_DIR = $SharedCargoTargetDir
      } elseif (-not [string]::IsNullOrWhiteSpace($Line.CargoTargetDirSlug)) {
        $targetRoot = if ([string]::IsNullOrWhiteSpace($env:NEURO_GATEWAY_VERIFY_TARGET_ROOT)) {
          Join-Path ([System.IO.Path]::GetTempPath()) "ngw-line-targets"
        } else {
          $env:NEURO_GATEWAY_VERIFY_TARGET_ROOT
        }
        New-Item -ItemType Directory -Force -Path $targetRoot | Out-Null
        $env:CARGO_TARGET_DIR = Join-Path $targetRoot $Line.CargoTargetDirSlug
      }

      foreach ($filter in $Line.FocusedCargoFilters) {
        $cargoArgs = @("cargo", "test", "--manifest-path", $CargoToml, "--locked")
        if ($LibOnly) {
          $cargoArgs += "--lib"
        }
        if ($Line.UseNoDefaultFeatures) {
          $cargoArgs += "--no-default-features"
          if (-not [string]::IsNullOrWhiteSpace($featureText)) {
            $cargoArgs += "--features"
            $cargoArgs += $featureText
          }
        }
        $cargoArgs += [string]$filter
        $cargoArgs += "--"
        $cargoArgs += "--nocapture"
        $cargoArgs += "--test-threads=1"
        Invoke-CheckedCommand $cargoArgs
      }
    } finally {
      $env:CARGO_TARGET_DIR = $previousTargetDir
    }
  }

  [pscustomobject]@{
    status = "pass"
    lineId = $Line.Id
    manifestPath = $Line.Path
    cargoSkipped = [bool]$SkipCargo
  }
}

$lines = @(Get-GatewayLineManifests)

if ($ListOnly) {
  Write-ListOnlyOutput -Lines $lines
  exit 0
}

if ($All -and -not [string]::IsNullOrWhiteSpace($LineId)) {
  throw "LineId cannot be combined with -All."
}

if (-not $All -and [string]::IsNullOrWhiteSpace($LineId)) {
  throw "LineId is required unless -ListOnly is provided."
}

[array]$results = if ($All) {
  foreach ($line in $lines) {
    Invoke-GatewayLineVerification -Line $line
  }
} else {
  $line = Find-GatewayLine -Lines $lines -Lookup $LineId
  @(Invoke-GatewayLineVerification -Line $line)
}

if ($AsJson) {
  [pscustomobject]@{
    status = "pass"
    lineId = if ($All) { $null } else { $results[0].lineId }
    manifestPath = if ($All) { $null } else { $results[0].manifestPath }
    lineCount = $results.Count
    cargoSkipped = [bool]$SkipCargo
    libOnly = [bool]$LibOnly
    sharedCargoTargetDir = if ([string]::IsNullOrWhiteSpace($SharedCargoTargetDir)) { $null } else { $SharedCargoTargetDir }
    results = $results
  } | ConvertTo-Json -Depth 4
} else {
  if ($All) {
    Write-Host "[verify-gateway-line] status=pass lines=$($results.Count)"
  } else {
    Write-Host "[verify-gateway-line] status=pass line=$($results[0].lineId)"
  }
}
