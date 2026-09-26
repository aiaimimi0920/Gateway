param(
  [switch]$SkipPythonTests,
  [switch]$SkipLineMatrix,
  [switch]$SkipRustAllTargets,
  [switch]$SkipReleaseBuild,
  [switch]$SkipBrowserWorkers,
  [switch]$IncludeDockerBuild,
  [switch]$IncludeRuntimeSmoke,
  [string]$SharedCargoTargetDir,
  [string]$LogRoot,
  [switch]$AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$GatewayRoot = Split-Path -Parent $PSScriptRoot
$CargoToml = Join-Path $GatewayRoot "Cargo.toml"
$ReleaseBinary = Join-Path $GatewayRoot "target\release\gateway.exe"
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
  $ReleaseBinary = Join-Path $GatewayRoot "target/release/gateway"
}

if ([string]::IsNullOrWhiteSpace($LogRoot)) {
  $LogRoot = Join-Path $GatewayRoot ("target\release-candidate-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
}
New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null

if ([string]::IsNullOrWhiteSpace($SharedCargoTargetDir)) {
  $SharedCargoTargetDir = Join-Path ([System.IO.Path]::GetTempPath()) "ngw-line-targets\release-candidate-shared"
}

$steps = [System.Collections.Generic.List[object]]::new()

function Resolve-PowerShellExecutable {
  $candidates = if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
    @("powershell", "pwsh")
  } else {
    @("pwsh", "powershell")
  }
  foreach ($candidate in $candidates) {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) {
      return $candidate
    }
  }
  throw "Neither pwsh nor powershell is available on PATH"
}

$PowerShellExecutable = Resolve-PowerShellExecutable

function Write-ProgressLine {
  param([string]$Message)

  if (-not $AsJson) {
    Write-Host $Message
  }
}

function Add-StepResult {
  param(
    [string]$Name,
    [string]$Status,
    [string]$CommandText,
    [string]$LogPath,
    [Nullable[int]]$ExitCode,
    [double]$DurationSeconds,
    [string]$Reason
  )

  $steps.Add([pscustomobject]@{
      name = $Name
      status = $Status
      command = $CommandText
      logPath = $LogPath
      exitCode = $ExitCode
      durationSeconds = [Math]::Round($DurationSeconds, 3)
      reason = $Reason
    }) | Out-Null
}

function Add-SkippedStep {
  param(
    [string]$Name,
    [string]$Reason
  )

  Add-StepResult -Name $Name -Status "skipped" -CommandText $null -LogPath $null -ExitCode $null -DurationSeconds 0 -Reason $Reason
}

function Invoke-CheckedCommand {
  param(
    [string]$Name,
    [string[]]$Command
  )

  $safeName = $Name -replace '[^A-Za-z0-9_.-]', '-'
  $logPath = Join-Path $LogRoot "$safeName.log"
  $commandText = $Command -join " "
  Write-ProgressLine ">>> [$Name] $commandText"
  $started = Get-Date
  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    & $Command[0] @($Command | Select-Object -Skip 1) *> $logPath
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  $duration = ((Get-Date) - $started).TotalSeconds
  if ($exitCode -ne 0) {
    Add-StepResult -Name $Name -Status "fail" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason "command exited non-zero"
    throw "Release candidate step failed: $Name exit=$exitCode log=$logPath"
  }

  Add-StepResult -Name $Name -Status "pass" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason $null
}

function Invoke-OptionalCommand {
  param(
    [string]$Name,
    [bool]$Enabled,
    [string]$SkipReason,
    [string[]]$Command
  )

  if (-not $Enabled) {
    Add-SkippedStep -Name $Name -Reason $SkipReason
    return
  }
  Invoke-CheckedCommand -Name $Name -Command $Command
}

function Invoke-LiveProviderCanaryPreflight {
  $name = "live-provider-canary-preflight"
  $artifactRoot = Join-Path $LogRoot $name
  $summaryPath = Join-Path $artifactRoot "summary.json"
  $logPath = Join-Path $LogRoot "$name.log"
  $command = @(
    $PowerShellExecutable,
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    (Join-Path $GatewayRoot "scripts\invoke-gateway-live-provider-canary.ps1"),
    "-ArtifactRoot",
    $artifactRoot,
    "-AsJson"
  )
  $commandText = $command -join " "
  Write-ProgressLine ">>> [$name] $commandText"
  $started = Get-Date
  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    & $command[0] @($command | Select-Object -Skip 1) *> $logPath
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }

  $duration = ((Get-Date) - $started).TotalSeconds
  if ($exitCode -ne 0) {
    Add-StepResult -Name $name -Status "fail" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason "command exited non-zero"
    throw "Release candidate step failed: $name exit=$exitCode log=$logPath"
  }

  if (-not (Test-Path -LiteralPath $summaryPath)) {
    Add-StepResult -Name $name -Status "fail" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason "summary.json missing"
    throw "Release candidate step failed: $name missing summary=$summaryPath log=$logPath"
  }

  $summary = Get-Content -LiteralPath $summaryPath -Raw -Encoding UTF8 | ConvertFrom-Json
  if ($summary.status -ne "skipped") {
    Add-StepResult -Name $name -Status "fail" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason "expected skipped canary status"
    throw "Release candidate step failed: $name expected skipped status, got $($summary.status) summary=$summaryPath log=$logPath"
  }
  if ([bool]$summary.allowLiveProviderCalls) {
    Add-StepResult -Name $name -Status "fail" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason "live provider calls were enabled"
    throw "Release candidate step failed: $name unexpectedly enabled live provider calls summary=$summaryPath log=$logPath"
  }

  Add-StepResult -Name $name -Status "pass" -CommandText $commandText -LogPath $logPath -ExitCode $exitCode -DurationSeconds $duration -Reason "safe skipped"
}

$runtimeOwner = Join-Path $PSScriptRoot "verify-gateway-release-candidate.runtime.ps1"
if (-not (Test-Path -LiteralPath $runtimeOwner)) {
  throw "Release candidate runtime owner missing: $runtimeOwner"
}
. $runtimeOwner

$overallStatus = "pass"
$errorMessage = $null

try {
  Invoke-CheckedCommand -Name "manifest-validator" -Command @(
    "python",
    (Join-Path $GatewayRoot "tools\validate-gateway-line-manifests.py")
  )

  Invoke-LiveProviderCanaryPreflight

  Invoke-OptionalCommand -Name "python-tests" -Enabled (-not $SkipPythonTests) -SkipReason "disabled by -SkipPythonTests" -Command @(
    "python",
    "-m",
    "unittest",
    "discover",
    "-s",
    "tests/python",
    "-p",
    "test_*.py",
    "-v"
  )

  Invoke-OptionalCommand -Name "line-matrix" -Enabled (-not $SkipLineMatrix) -SkipReason "disabled by -SkipLineMatrix" -Command @(
    $PowerShellExecutable,
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    (Join-Path $GatewayRoot "tools\verify-gateway-line.ps1"),
    "-All",
    "-LibOnly",
    "-SharedCargoTargetDir",
    $SharedCargoTargetDir
  )

  Invoke-OptionalCommand -Name "rust-all-targets" -Enabled (-not $SkipRustAllTargets) -SkipReason "disabled by -SkipRustAllTargets" -Command @(
    "cargo",
    "test",
    "--manifest-path",
    $CargoToml,
    "--locked",
    "--all-targets",
    "--",
    "--test-threads=1"
  )

  Invoke-OptionalCommand -Name "release-build" -Enabled (-not $SkipReleaseBuild) -SkipReason "disabled by -SkipReleaseBuild" -Command @(
    "cargo",
    "build",
    "--manifest-path",
    $CargoToml,
    "--locked",
    "--release",
    "--bin",
    "gateway"
  )

  if ($SkipBrowserWorkers) {
    Add-SkippedStep -Name "browser-worker-install" -Reason "disabled by -SkipBrowserWorkers"
    Add-SkippedStep -Name "browser-worker-tests" -Reason "disabled by -SkipBrowserWorkers"
  } else {
    Invoke-CheckedCommand -Name "browser-worker-install" -Command @("npm", "ci", "--prefix", "scripts")
    Invoke-CheckedCommand -Name "browser-worker-tests" -Command @("node", "--test", "scripts/tests/*.test.mjs")
  }

  $auditNonce = [guid]::NewGuid().ToString("N")
  Invoke-OptionalCommand -Name "docker-build" -Enabled ([bool]$IncludeDockerBuild) -SkipReason "opt-in only; pass -IncludeDockerBuild" -Command @(
    "docker",
    "build",
    "--build-arg",
    "GATEWAY_AUDIT_NONCE=$auditNonce",
    "-f",
    "Dockerfile",
    "-t",
    "gateway:release-candidate",
    "."
  )

  if ($IncludeRuntimeSmoke) {
    Invoke-GatewayRuntimeSmoke
  } else {
    Add-SkippedStep -Name "runtime-smoke" -Reason "opt-in only; requires a free port, release binary, and Redis"
  }
} catch {
  $overallStatus = "fail"
  $errorMessage = $_.Exception.Message
}

$payload = [pscustomobject]@{
  status = $overallStatus
  logRoot = $LogRoot
  sharedCargoTargetDir = $SharedCargoTargetDir
  releaseBinary = $ReleaseBinary
  error = $errorMessage
  steps = $steps
}

if ($AsJson) {
  $payload | ConvertTo-Json -Depth 5
} else {
  Write-Host "[verify-gateway-release-candidate] status=$overallStatus logRoot=$LogRoot"
  foreach ($step in $steps) {
    $suffix = if ([string]::IsNullOrWhiteSpace($step.logPath)) { "" } else { " log=$($step.logPath)" }
    Write-Host "  - $($step.name): $($step.status)$suffix"
  }
  if ($errorMessage) {
    Write-Host "error=$errorMessage"
  }
}

if ($overallStatus -ne "pass") {
  exit 1
}
