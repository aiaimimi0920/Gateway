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

function Invoke-SmokeHttpRequest {
  param(
    [string]$Method,
    [string]$Uri,
    [string]$Body,
    [string]$ContentType,
    [hashtable]$Headers
  )

  $curl = if (Get-Command "curl.exe" -ErrorAction SilentlyContinue) { "curl.exe" } else { "curl" }
  $requestId = [guid]::NewGuid().ToString("N")
  $bodyPath = Join-Path $LogRoot "runtime-smoke.$requestId.body"
  $headersPath = Join-Path $LogRoot "runtime-smoke.$requestId.headers"
  $stderrPath = Join-Path $LogRoot "runtime-smoke.$requestId.curl.stderr"
  $curlArgs = @(
    "--silent",
    "--show-error",
    "--noproxy",
    "*",
    "--max-time",
    "10",
    "--dump-header",
    $headersPath,
    "--output",
    $bodyPath,
    "--write-out",
    "%{http_code}"
  )
  if ([string]::Equals($Method, "HEAD", [System.StringComparison]::OrdinalIgnoreCase)) {
    $curlArgs += "--head"
  } elseif (-not [string]::Equals($Method, "GET", [System.StringComparison]::OrdinalIgnoreCase)) {
    $curlArgs += "--request"
    $curlArgs += $Method
  }
  if (-not [string]::IsNullOrWhiteSpace($ContentType)) {
    $curlArgs += "--header"
    $curlArgs += "Content-Type: $ContentType"
  }
  if ($null -ne $Headers) {
    foreach ($key in $Headers.Keys) {
      $curlArgs += "--header"
      $curlArgs += "${key}: $($Headers[$key])"
    }
  }
  if ($PSBoundParameters.ContainsKey("Body")) {
    $curlArgs += "--data-binary"
    $curlArgs += $Body
  }
  $curlArgs += $Uri

  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    $statusText = & $curl @curlArgs 2> $stderrPath
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  if ($exitCode -ne 0) {
    $curlError = if (Test-Path -LiteralPath $stderrPath) {
      (Get-Content -LiteralPath $stderrPath -Raw -Encoding UTF8).Trim()
    } else {
      ""
    }
    throw "curl smoke request failed: method=$Method uri=$Uri exit=$exitCode stderr=$curlError"
  }

  $statusCandidate = (($statusText | Out-String).Trim() -split "\s+")[-1]
  if (-not ($statusCandidate -match '^\d{3}$')) {
    throw "curl smoke request did not return an HTTP status: method=$Method uri=$Uri output=$statusText"
  }
  $headerMap = @{}
  if (Test-Path -LiteralPath $headersPath) {
    foreach ($line in Get-Content -LiteralPath $headersPath -Encoding UTF8) {
      $separator = $line.IndexOf(":")
      if ($separator -gt 0) {
        $key = $line.Substring(0, $separator).Trim()
        $value = $line.Substring($separator + 1).Trim()
        $headerMap[$key] = $value
      }
    }
  }
  $content = if (Test-Path -LiteralPath $bodyPath) {
    Get-Content -LiteralPath $bodyPath -Raw -Encoding UTF8
  } else {
    ""
  }

  [pscustomobject]@{
    statusCode = [int]$statusCandidate
    content = $content
    headers = $headerMap
  }
}

function Assert-SmokeStatus {
  param(
    [string]$Name,
    [int]$Expected,
    [object]$Response,
    [System.Collections.Generic.List[string]]$Lines
  )

  if ($Response.statusCode -ne $Expected) {
    throw "$Name expected HTTP $Expected, got HTTP $($Response.statusCode)"
  }
  $Lines.Add("PASS $Name HTTP $($Response.statusCode)") | Out-Null
}

function Assert-SmokeStatusRange {
  param(
    [string]$Name,
    [int]$Minimum,
    [int]$Maximum,
    [object]$Response,
    [System.Collections.Generic.List[string]]$Lines
  )

  if ($Response.statusCode -lt $Minimum -or $Response.statusCode -gt $Maximum) {
    throw "$Name expected HTTP $Minimum..$Maximum, got HTTP $($Response.statusCode)"
  }
  $Lines.Add("PASS $Name HTTP $($Response.statusCode)") | Out-Null
}

function Assert-SmokeContains {
  param(
    [string]$Name,
    [string]$Text,
    [string]$Needle,
    [System.Collections.Generic.List[string]]$Lines
  )

  if ($null -eq $Text -or $Text.IndexOf($Needle, [System.StringComparison]::OrdinalIgnoreCase) -lt 0) {
    throw "$Name missing expected text: $Needle"
  }
  $Lines.Add("PASS $Name contains $Needle") | Out-Null
}

function Assert-SmokeHeader {
  param(
    [string]$Name,
    [object]$Headers,
    [string]$HeaderName,
    [System.Collections.Generic.List[string]]$Lines
  )

  $found = $false
  foreach ($key in $Headers.Keys) {
    if ([string]::Equals([string]$key, $HeaderName, [System.StringComparison]::OrdinalIgnoreCase)) {
      $found = $true
      break
    }
  }
  if (-not $found) {
    throw "$Name missing expected header: $HeaderName"
  }
  $Lines.Add("PASS $Name has header $HeaderName") | Out-Null
}

function Start-GatewaySmokeProcess {
  param(
    [string]$Binary,
    [string]$WorkingDirectory,
    [string]$StdoutPath,
    [string]$StderrPath
  )

  $args = @{
    FilePath = $Binary
    WorkingDirectory = $WorkingDirectory
    RedirectStandardOutput = $StdoutPath
    RedirectStandardError = $StderrPath
    PassThru = $true
  }
  if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
    $args.WindowStyle = "Hidden"
  }

  Start-Process @args
}

function Invoke-GatewayRuntimeSmoke {
  $name = "runtime-smoke"
  $logPath = Join-Path $LogRoot "$name.log"
  $stdoutPath = Join-Path $LogRoot "$name.gateway.stdout.log"
  $stderrPath = Join-Path $LogRoot "$name.gateway.stderr.log"
  $started = Get-Date
  $process = $null
  $lines = [System.Collections.Generic.List[string]]::new()
  $previousGatewayBin = $env:GATEWAY_BIN
  $previousRuntimeRole = $env:GATEWAY_RUNTIME_ROLE
  $previousPort = $env:PORT
  $previousRedisUrl = $env:GATEWAY_REDIS_URL
  $previousRustLog = $env:RUST_LOG
  $previousGatewayApiKey = $env:GATEWAY_API_KEY

  try {
    if (-not (Test-Path -LiteralPath $ReleaseBinary)) {
      throw "release binary not found: $ReleaseBinary"
    }

    $runtimePort = if ([string]::IsNullOrWhiteSpace($env:PORT)) { "7777" } else { $env:PORT }
    $redisUrl = if ([string]::IsNullOrWhiteSpace($env:GATEWAY_REDIS_URL)) { "redis://localhost:6379" } else { $env:GATEWAY_REDIS_URL }
    $baseUrl = "http://127.0.0.1:$runtimePort"
    $smokeGatewayApiKey = "release-candidate-smoke-key"
    $authHeaders = @{ "Authorization" = "Bearer $smokeGatewayApiKey" }
    $commandText = "$ReleaseBinary smoke $baseUrl"

    Write-ProgressLine ">>> [$name] $commandText"
    $lines.Add("runtime-smoke baseUrl=$baseUrl") | Out-Null
    $lines.Add("runtime-smoke redisUrl=$redisUrl") | Out-Null
    $lines.Add("runtime-smoke binary=$ReleaseBinary") | Out-Null
    $lines.Add("runtime-smoke gatewayApiKey=temporary-local") | Out-Null

    $env:GATEWAY_BIN = $ReleaseBinary
    $env:GATEWAY_RUNTIME_ROLE = "standalone"
    $env:PORT = $runtimePort
    $env:GATEWAY_REDIS_URL = $redisUrl
    $env:GATEWAY_API_KEY = $smokeGatewayApiKey
    if ([string]::IsNullOrWhiteSpace($env:RUST_LOG)) {
      $env:RUST_LOG = "info"
    }

    $process = Start-GatewaySmokeProcess `
      -Binary $ReleaseBinary `
      -WorkingDirectory $GatewayRoot `
      -StdoutPath $stdoutPath `
      -StderrPath $stderrPath
    $lines.Add("gateway pid=$($process.Id)") | Out-Null

    $health = $null
    for ($attempt = 1; $attempt -le 120; $attempt++) {
      if ($process.HasExited) {
        $stderrTail = if (Test-Path -LiteralPath $stderrPath) {
          (Get-Content -LiteralPath $stderrPath -Tail 40 -Encoding UTF8) -join "`n"
        } else {
          ""
        }
        throw "gateway exited before healthz became ready; exit=$($process.ExitCode); stderrTail=$stderrTail"
      }
      try {
        $health = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/healthz"
        if ($health.statusCode -eq 200) {
          break
        }
      } catch {
        Start-Sleep -Milliseconds 500
      }
      Start-Sleep -Milliseconds 500
    }
    if ($null -eq $health -or $health.statusCode -ne 200) {
      $netstat = (netstat -ano | Select-String ":$runtimePort" | Out-String).Trim()
      if (-not [string]::IsNullOrWhiteSpace($netstat)) {
        $lines.Add("netstat for port ${runtimePort}:") | Out-Null
        $lines.Add($netstat) | Out-Null
      }
      if (Test-Path -LiteralPath $stdoutPath) {
        $stdoutTail = (Get-Content -LiteralPath $stdoutPath -Tail 60 -Encoding UTF8) -join "`n"
        if (-not [string]::IsNullOrWhiteSpace($stdoutTail)) {
          $lines.Add("gateway stdout tail:") | Out-Null
          $lines.Add($stdoutTail) | Out-Null
        }
      }
      if (Test-Path -LiteralPath $stderrPath) {
        $stderrTail = (Get-Content -LiteralPath $stderrPath -Tail 60 -Encoding UTF8) -join "`n"
        if (-not [string]::IsNullOrWhiteSpace($stderrTail)) {
          $lines.Add("gateway stderr tail:") | Out-Null
          $lines.Add($stderrTail) | Out-Null
        }
      }
      throw "gateway did not return HTTP 200 from /healthz"
    }

    Assert-SmokeStatus -Name "/healthz" -Expected 200 -Response $health -Lines $lines
    Assert-SmokeContains -Name "/healthz body" -Text $health.content -Needle "status" -Lines $lines

    $models = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/v1/models" -Headers $authHeaders
    Assert-SmokeStatus -Name "/v1/models" -Expected 200 -Response $models -Lines $lines
    Assert-SmokeContains -Name "/v1/models body" -Text $models.content -Needle "data" -Lines $lines

    $head = Invoke-SmokeHttpRequest -Method "HEAD" -Uri "$baseUrl/healthz"
    Assert-SmokeStatus -Name "HEAD /healthz" -Expected 200 -Response $head -Lines $lines
    Assert-SmokeHeader -Name "HEAD /healthz" -Headers $head.headers -HeaderName "x-request-id" -Lines $lines

    $chat = Invoke-SmokeHttpRequest -Method "POST" -Uri "$baseUrl/v1/chat/completions" -Body "{}" -ContentType "application/json" -Headers $authHeaders
    Assert-SmokeStatus -Name "POST /v1/chat/completions empty body" -Expected 400 -Response $chat -Lines $lines
    Assert-SmokeContains -Name "chat error body" -Text $chat.content -Needle "error" -Lines $lines

    $messages = Invoke-SmokeHttpRequest -Method "POST" -Uri "$baseUrl/v1/messages" -Body "{}" -ContentType "application/json" -Headers $authHeaders
    Assert-SmokeStatus -Name "POST /v1/messages empty body" -Expected 400 -Response $messages -Lines $lines
    Assert-SmokeContains -Name "messages error body" -Text $messages.content -Needle "error" -Lines $lines

    $responses = Invoke-SmokeHttpRequest -Method "POST" -Uri "$baseUrl/v1/responses" -Body "{}" -ContentType "application/json" -Headers $authHeaders
    Assert-SmokeStatus -Name "POST /v1/responses empty body" -Expected 400 -Response $responses -Lines $lines

    $raw = Invoke-SmokeHttpRequest -Method "POST" -Uri "$baseUrl/v1/chat/completions" -Body "raw text" -Headers $authHeaders
    Assert-SmokeStatusRange -Name "POST /v1/chat/completions raw body" -Minimum 400 -Maximum 499 -Response $raw -Lines $lines

    $missing = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/v1/nonexistent"
    Assert-SmokeStatus -Name "GET /v1/nonexistent" -Expected 404 -Response $missing -Lines $lines

    $duration = ((Get-Date) - $started).TotalSeconds
    $lines | Set-Content -LiteralPath $logPath -Encoding UTF8
    Add-StepResult -Name $name -Status "pass" -CommandText $commandText -LogPath $logPath -ExitCode 0 -DurationSeconds $duration -Reason $null
  } catch {
    $duration = ((Get-Date) - $started).TotalSeconds
    $lines.Add("FAIL $($_.Exception.Message)") | Out-Null
    $lines | Set-Content -LiteralPath $logPath -Encoding UTF8
    Add-StepResult -Name $name -Status "fail" -CommandText "native runtime smoke" -LogPath $logPath -ExitCode 1 -DurationSeconds $duration -Reason $_.Exception.Message
    throw "Release candidate step failed: $name log=$logPath error=$($_.Exception.Message)"
  } finally {
    if ($null -ne $process -and -not $process.HasExited) {
      Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
      $process.WaitForExit(5000) | Out-Null
    }
    $env:GATEWAY_BIN = $previousGatewayBin
    $env:GATEWAY_RUNTIME_ROLE = $previousRuntimeRole
    $env:PORT = $previousPort
    $env:GATEWAY_REDIS_URL = $previousRedisUrl
    $env:RUST_LOG = $previousRustLog
    $env:GATEWAY_API_KEY = $previousGatewayApiKey
  }
}

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
    "--all-targets"
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

  Invoke-OptionalCommand -Name "docker-build" -Enabled ([bool]$IncludeDockerBuild) -SkipReason "opt-in only; pass -IncludeDockerBuild" -Command @(
    "docker",
    "build",
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
