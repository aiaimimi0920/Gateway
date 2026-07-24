[CmdletBinding()]
param(
  [ValidateSet("desktop-chromium", "mobile-chromium")]
  [string] $Project = "desktop-chromium",

  [switch] $InstallBrowsers,

  [switch] $AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$UiRoot = Join-Path $RepoRoot "apps\desktop"
$RuntimeRoot = Join-Path $RepoRoot ".runtime\console-live-e2e"
$RunId = Get-Date -Format "yyyyMMdd-HHmmss"
$SessionRoot = Join-Path $RuntimeRoot $RunId
$LogsRoot = Join-Path $SessionRoot "logs"
$GatewayStateRoot = Join-Path $SessionRoot "gateway-state"
$RoutesPath = Join-Path $SessionRoot "routes.yaml"
$ResultsRoot = Join-Path $SessionRoot "test-results"
$ArtifactsRoot = Join-Path $SessionRoot "artifacts"
$GatewayBinary = Join-Path $RepoRoot "target\debug\neuro-gateway.exe"
$LiveSpecPath = "e2e/console.live.spec.ts"
$RedisImage = "redis:7-alpine"

function Write-LiveLog {
  param([Parameter(Mandatory = $true)][string] $Message)
  Write-Host "[gateway-console-live-e2e] $Message"
}

function Ensure-Directory {
  param([Parameter(Mandatory = $true)][string] $Path)
  New-Item -ItemType Directory -Force -Path $Path | Out-Null
  return [System.IO.Path]::GetFullPath($Path)
}

function Resolve-Executable {
  param([Parameter(Mandatory = $true)][string] $Command)

  if (Test-Path -LiteralPath $Command -PathType Leaf) {
    return [System.IO.Path]::GetFullPath($Command)
  }

  $resolved = Get-Command $Command -ErrorAction Stop | Select-Object -First 1
  if ($null -eq $resolved) {
    throw "Command not found: $Command"
  }
  return $resolved.Source
}

function Get-FreeTcpPort {
  $listener = [System.Net.Sockets.TcpListener]::new(
    [System.Net.IPAddress]::Loopback,
    0
  )
  $listener.Start()
  try {
    return ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
  } finally {
    $listener.Stop()
  }
}

function Invoke-LoggedCommand {
  param(
    [Parameter(Mandatory = $true)][string] $LogPath,
    [Parameter(Mandatory = $true)][string[]] $Command,
    [string] $WorkingDirectory = $RepoRoot
  )

  Write-LiveLog ("running: " + ($Command -join " "))
  $stdoutCapture = Join-Path $env:TEMP ("gateway-live-e2e-stdout-" + [guid]::NewGuid().ToString("N") + ".log")
  $stderrCapture = Join-Path $env:TEMP ("gateway-live-e2e-stderr-" + [guid]::NewGuid().ToString("N") + ".log")
  try {
    $filePath = Resolve-Executable -Command $Command[0]
    $process = Start-Process `
      -FilePath $filePath `
      -ArgumentList @($Command | Select-Object -Skip 1) `
      -WorkingDirectory $WorkingDirectory `
      -RedirectStandardOutput $stdoutCapture `
      -RedirectStandardError $stderrCapture `
      -NoNewWindow `
      -PassThru `
      -Wait
    $exitCode = $process.ExitCode

    foreach ($capturePath in @($stdoutCapture, $stderrCapture)) {
      if (Test-Path -LiteralPath $capturePath) {
        Get-Content -LiteralPath $capturePath -Encoding UTF8 | Tee-Object -FilePath $LogPath -Append
      }
    }
  } finally {
    foreach ($capturePath in @($stdoutCapture, $stderrCapture)) {
      if (Test-Path -LiteralPath $capturePath) {
        Remove-Item -LiteralPath $capturePath -Force -ErrorAction SilentlyContinue
      }
    }
  }
  if ($exitCode -ne 0) {
    throw "Command failed with exit code ${exitCode}: $($Command -join ' ')"
  }
}

function Invoke-Docker {
  param(
    [Parameter(Mandatory = $true)][string] $DockerExecutable,
    [Parameter(Mandatory = $true)][string[]] $Arguments,
    [Parameter(Mandatory = $true)][string] $Operation,
    [switch] $AllowFailure
  )

  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    $output = & $DockerExecutable @Arguments 2>&1
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  if ($exitCode -ne 0 -and -not $AllowFailure) {
    $details = (($output | ForEach-Object { [string]$_ }) -join "`n").Trim()
    if ([string]::IsNullOrWhiteSpace($details)) {
      throw "$Operation failed with exit code $exitCode."
    }
    throw "$Operation failed with exit code ${exitCode}: $details"
  }

  return [pscustomobject]@{
    ExitCode = $exitCode
    Text = (($output | ForEach-Object { [string]$_ }) -join "`n").Trim()
  }
}

function Stop-TemporaryRedis {
  param(
    [Parameter(Mandatory = $true)][string] $DockerExecutable,
    [Parameter(Mandatory = $true)][string] $ContainerName
  )

  $null = Invoke-Docker `
    -DockerExecutable $DockerExecutable `
    -Arguments @("rm", "--force", $ContainerName) `
    -Operation "remove disposable Redis container" `
    -AllowFailure
}

function Start-TemporaryRedis {
  param(
    [Parameter(Mandatory = $true)][string] $DockerExecutable,
    [Parameter(Mandatory = $true)][int] $StartupTimeoutSeconds
  )

  $containerName = "gateway-console-live-e2e-$([guid]::NewGuid().ToString('N').Substring(0, 12))"
  $containerStarted = $false
  try {
    $run = Invoke-Docker `
      -DockerExecutable $DockerExecutable `
      -Arguments @(
        "run",
        "--detach",
        "--rm",
        "--name",
        $containerName,
        "--publish",
        "127.0.0.1::6379",
        $RedisImage
      ) `
      -Operation "start disposable Redis container"
    if ([string]::IsNullOrWhiteSpace($run.Text)) {
      throw "docker run did not return a container id."
    }
    $containerStarted = $true

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $hostPort = 0
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
      $port = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("port", $containerName, "6379/tcp") `
        -Operation "resolve disposable Redis host port" `
        -AllowFailure
      if ($port.ExitCode -eq 0 -and $port.Text -match ":(?<port>[0-9]+)\s*$") {
        $hostPort = [int]$Matches["port"]
        if ($hostPort -gt 0) {
          break
        }
      }
      Start-Sleep -Milliseconds 200
    }
    if ($hostPort -le 0) {
      throw "Disposable Redis did not publish a host port before the timeout."
    }

    while ([DateTimeOffset]::UtcNow -lt $deadline) {
      $ping = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("exec", $containerName, "redis-cli", "PING") `
        -Operation "ping disposable Redis" `
        -AllowFailure
      if ($ping.ExitCode -eq 0 -and $ping.Text.Trim() -eq "PONG") {
        return [pscustomobject]@{
          ContainerName = $containerName
          HostPort = $hostPort
          Url = "redis://127.0.0.1:$hostPort/0"
        }
      }
      Start-Sleep -Milliseconds 200
    }

    throw "Disposable Redis did not become ready before the timeout."
  } catch {
    if ($containerStarted) {
      Stop-TemporaryRedis -DockerExecutable $DockerExecutable -ContainerName $containerName
    }
    throw
  }
}

function Invoke-SmokeHttpRequest {
  param(
    [Parameter(Mandatory = $true)][string] $Method,
    [Parameter(Mandatory = $true)][string] $Uri,
    [hashtable] $Headers = @{},
    [string] $Body = "",
    [string] $ContentType = "application/json",
    [ValidateRange(1, 10)][int] $RetryCount = 1
  )

  $arguments = @{
    Method = $Method
    Uri = $Uri
    Headers = $Headers
    UseBasicParsing = $true
    TimeoutSec = 5
  }
  if (-not [string]::IsNullOrEmpty($Body)) {
    $arguments.Body = $Body
    $arguments.ContentType = $ContentType
  }

  for ($attempt = 1; $attempt -le $RetryCount; $attempt++) {
    try {
      $response = Invoke-WebRequest @arguments
      return [pscustomobject]@{
        StatusCode = [int]$response.StatusCode
        Content = [string]$response.Content
        Headers = $response.Headers
      }
    } catch {
      $statusCode = 0
      $responseProperty = $_.Exception.PSObject.Properties["Response"]
      if ($null -ne $responseProperty -and $null -ne $responseProperty.Value) {
        try {
          $statusCode = [int]$responseProperty.Value.StatusCode
        } catch {
          $statusCode = 0
        }
      }
      if ($statusCode -eq 0 -and $attempt -lt $RetryCount) {
        Start-Sleep -Milliseconds 250
        continue
      }
      return [pscustomobject]@{
        StatusCode = $statusCode
        Content = $_.Exception.Message
        Headers = @{}
      }
    }
  }

  throw "HTTP smoke request did not produce a result."
}

function Assert-StatusCode {
  param(
    [Parameter(Mandatory = $true)][string] $Name,
    [Parameter(Mandatory = $true)][int] $Expected,
    [Parameter(Mandatory = $true)] $Response
  )

  if ($Response.StatusCode -ne $Expected) {
    throw "$Name expected HTTP $Expected but received $($Response.StatusCode): $($Response.Content)"
  }
}

if (-not (Test-Path -LiteralPath $UiRoot -PathType Container)) {
  throw "UI workspace is missing: $UiRoot"
}

$null = Ensure-Directory -Path $RuntimeRoot
$null = Ensure-Directory -Path $SessionRoot
$null = Ensure-Directory -Path $LogsRoot
$null = Ensure-Directory -Path $GatewayStateRoot
$null = Ensure-Directory -Path $ResultsRoot
$null = Ensure-Directory -Path $ArtifactsRoot

$buildLog = Join-Path $LogsRoot "build.log"
$playwrightLog = Join-Path $LogsRoot "playwright.log"
$gatewayStdout = Join-Path $LogsRoot "gateway.stdout.log"
$gatewayStderr = Join-Path $LogsRoot "gateway.stderr.log"

$routesYaml = @'
providers:
  - id: managed-provider
    preset: openai
    base_url: "https://api.primary.example.com"
    api_key: "sk-test"
    supported_models: [gpt-5.4]
model_routes:
  - pattern: "gpt-5.4"
    provider_ids: [managed-provider]
    priority: 10
aliases:
  answer: gpt-5.4
'@
[System.IO.File]::WriteAllText(
  $RoutesPath,
  $routesYaml.Replace("`r`n", "`n") + "`n",
  [System.Text.UTF8Encoding]::new($false)
)

if ($InstallBrowsers) {
  Invoke-LoggedCommand -LogPath $buildLog -Command @(
    "npx.cmd",
    "--prefix",
    $UiRoot,
    "playwright",
    "install",
    "chromium"
  )
}

Invoke-LoggedCommand -LogPath $buildLog -Command @(
  "npm.cmd",
  "run",
  "build:web",
  "--prefix",
  $UiRoot
)

Invoke-LoggedCommand -LogPath $buildLog -Command @(
  "cargo",
  "build",
  "--locked",
  "--bin",
  "neuro-gateway"
)

if (-not (Test-Path -LiteralPath $GatewayBinary -PathType Leaf)) {
  throw "Expected debug gateway binary was not produced: target\debug\neuro-gateway.exe"
}

$dockerExecutable = (Get-Command docker -ErrorAction Stop | Select-Object -First 1).Source
$temporaryRedis = $null
$gatewayProcess = $null
$previousEnvironment = @{}
$environmentKeys = @(
  "GATEWAY_RUNTIME_ROLE",
  "PORT",
  "GATEWAY_REDIS_URL",
  "GATEWAY_API_KEY",
  "GATEWAY_API_KEY_SECRET",
  "GATEWAY_MANAGEMENT_TOKEN",
  "GATEWAY_PUBLIC_BASE_URL",
  "GATEWAY_ROUTES_FILE",
  "GATEWAY_STATE_DIR",
  "RUST_LOG",
  "GATEWAY_UI_BASE_URL",
  "PLAYWRIGHT_SKIP_WEBSERVER",
  "GATEWAY_LIVE_MANAGEMENT_TOKEN",
  "GATEWAY_LIVE_API_BASE_URL",
  "GATEWAY_LIVE_API_TOKEN",
  "GATEWAY_LIVE_EXPECT_PROVIDER_ID",
  "GATEWAY_LIVE_EXPECT_MODEL",
  "PLAYWRIGHT_HTML_OUTPUT_DIR"
)
foreach ($key in $environmentKeys) {
  $previousEnvironment[$key] = [System.Environment]::GetEnvironmentVariable($key, "Process")
}

try {
  $temporaryRedis = Start-TemporaryRedis -DockerExecutable $dockerExecutable -StartupTimeoutSeconds 30
  Write-LiveLog "started disposable Redis container=$($temporaryRedis.ContainerName) hostPort=$($temporaryRedis.HostPort)"

  $port = Get-FreeTcpPort
  $baseUrl = "http://127.0.0.1:$port"
  $managementToken = "gateway-live-management-$([guid]::NewGuid().ToString('N'))"
  $apiToken = "gateway-live-api-$([guid]::NewGuid().ToString('N'))"
  $apiHeaders = @{ Authorization = "Bearer $apiToken" }

  [System.Environment]::SetEnvironmentVariable("GATEWAY_RUNTIME_ROLE", "standalone", "Process")
  [System.Environment]::SetEnvironmentVariable("PORT", [string]$port, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_REDIS_URL", $temporaryRedis.Url, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_API_KEY", $apiToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_API_KEY_SECRET", "gateway-live-secret-$([guid]::NewGuid().ToString('N'))", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_MANAGEMENT_TOKEN", $managementToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_PUBLIC_BASE_URL", $baseUrl, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_ROUTES_FILE", $RoutesPath, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_STATE_DIR", $GatewayStateRoot, "Process")
  [System.Environment]::SetEnvironmentVariable("RUST_LOG", "info", "Process")

  $gatewayProcess = Start-Process `
    -FilePath $GatewayBinary `
    -WorkingDirectory $RepoRoot `
    -RedirectStandardOutput $gatewayStdout `
    -RedirectStandardError $gatewayStderr `
    -PassThru `
    -WindowStyle Hidden
  Write-LiveLog "started Gateway pid=$($gatewayProcess.Id) port=$port"

  $deadline = [DateTimeOffset]::UtcNow.AddSeconds(30)
  $health = $null
  while ([DateTimeOffset]::UtcNow -lt $deadline) {
    if ($gatewayProcess.HasExited) {
      $stderrTail = if (Test-Path -LiteralPath $gatewayStderr) {
        (Get-Content -LiteralPath $gatewayStderr -Tail 50 -Encoding UTF8) -join "`n"
      } else {
        ""
      }
      throw "Gateway exited before readiness: $stderrTail"
    }

    $health = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/healthz" -RetryCount 3
    if ($health.StatusCode -eq 200) {
      break
    }
    Start-Sleep -Milliseconds 250
  }
  Assert-StatusCode -Name "/healthz" -Expected 200 -Response $health

  $uiShellResponse = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/ui/" -RetryCount 3
  Assert-StatusCode -Name "/ui/" -Expected 200 -Response $uiShellResponse
  if ($uiShellResponse.Content -notmatch "Neuro Gateway") {
    throw "Gateway /ui/ did not return the embedded browser console shell."
  }

  $models = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/v1/models" -Headers $apiHeaders -RetryCount 3
  Assert-StatusCode -Name "/v1/models" -Expected 200 -Response $models
  if ($models.Content -notmatch "gpt-5\.4") {
    throw "/v1/models did not contain the expected gpt-5.4 entry."
  }

  [System.Environment]::SetEnvironmentVariable("GATEWAY_UI_BASE_URL", "$baseUrl/ui/", "Process")
  [System.Environment]::SetEnvironmentVariable("PLAYWRIGHT_SKIP_WEBSERVER", "1", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_MANAGEMENT_TOKEN", $managementToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_API_BASE_URL", $baseUrl, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_API_TOKEN", $apiToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_PROVIDER_ID", "managed-provider", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_MODEL", "gpt-5.4", "Process")
  [System.Environment]::SetEnvironmentVariable("PLAYWRIGHT_HTML_OUTPUT_DIR", $ArtifactsRoot, "Process")

  Invoke-LoggedCommand -LogPath $playwrightLog -Command @(
    "npm.cmd",
    "run",
    "e2e",
    "--prefix",
    $UiRoot,
    "--",
    "--project=$Project",
    "--output",
    $ResultsRoot,
    $LiveSpecPath
  )

  $payload = [pscustomobject]@{
    status = "pass"
    project = $Project
    runtimeRoot = $SessionRoot
    gatewayBaseUrl = $baseUrl
    redisUrl = $temporaryRedis.Url
    routesPath = $RoutesPath
    gatewayBinary = "target\debug\neuro-gateway.exe"
    playwrightSpec = $LiveSpecPath
    resultsRoot = $ResultsRoot
    artifactsRoot = $ArtifactsRoot
    logs = [pscustomobject]@{
      build = $buildLog
      playwright = $playwrightLog
      stdout = $gatewayStdout
      stderr = $gatewayStderr
    }
  }

  if ($AsJson) {
    $payload | ConvertTo-Json -Depth 5
  } else {
    Write-LiveLog "status=pass project=$Project"
    Write-LiveLog "runtimeRoot=$SessionRoot"
  }
} finally {
  if ($null -ne $gatewayProcess -and -not $gatewayProcess.HasExited) {
    Stop-Process -Id $gatewayProcess.Id -Force -ErrorAction SilentlyContinue
    $gatewayProcess.WaitForExit(5000) | Out-Null
  }
  if ($null -ne $temporaryRedis) {
    Stop-TemporaryRedis -DockerExecutable $dockerExecutable -ContainerName $temporaryRedis.ContainerName
  }
  foreach ($key in $environmentKeys) {
    [System.Environment]::SetEnvironmentVariable($key, $previousEnvironment[$key], "Process")
  }
}
