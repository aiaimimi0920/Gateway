[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ReleaseDir,
    [string]$RedisUrl = "",
    [string]$DockerPath = "docker",
    [string]$RedisImage = "redis:7-alpine",
    [ValidateRange(0, 15)][int]$RedisDatabase = 0,
    [int]$Port = 0,
    [string]$EvidencePath = "",
    [int]$StartupTimeoutSeconds = 30,
    [int]$ShutdownTimeoutSeconds = 15,
    [switch]$IntegrityOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Smoke {
    param([string]$Message)
    Write-Output "[gateway-packaged-smoke] $Message"
}


$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $scriptRoot "smoke-gateway-packaged-runtime.integrity.ps1")
. (Join-Path $scriptRoot "smoke-gateway-packaged-runtime.runtime.ps1")

$releaseDirFull = [System.IO.Path]::GetFullPath($ReleaseDir)
if (-not (Test-Path -LiteralPath $releaseDirFull -PathType Container)) {
    throw "ReleaseDir does not exist: $releaseDirFull"
}
$gatewayBinary = Join-Path $releaseDirFull "gateway.exe"
if (-not (Test-Path -LiteralPath $gatewayBinary -PathType Leaf)) {
    throw "Packaged Gateway binary is missing: $gatewayBinary"
}
foreach ($required in @("manifest.json", "checksums.sha256", "manifests", "scripts")) {
    if (-not (Test-Path -LiteralPath (Join-Path $releaseDirFull $required))) {
        throw "Packaged Gateway support entry is missing: $required"
    }
}
# Verify the manifest and every exact checksum path+digest before starting
# packaged runtime; a tampered package must never be executed.
$integrity = Assert-PackagedReleaseIntegrity -ReleaseRoot $releaseDirFull
if ($IntegrityOnly) {
    Write-Smoke "package integrity passed; manifestSha256=$($integrity.manifestSha256) checksumsSha256=$($integrity.checksumsSha256)"
    exit 0
}

if ($Port -le 0) {
    $Port = Get-FreeTcpPort
}
if ([string]::IsNullOrWhiteSpace($EvidencePath)) {
    $EvidencePath = Join-Path ([System.IO.Path]::GetTempPath()) "gateway-packaged-smoke-$Port.json"
}
$evidencePathFull = [System.IO.Path]::GetFullPath($EvidencePath)
$evidenceParent = Split-Path -Parent $evidencePathFull
New-Item -ItemType Directory -Path $evidenceParent -Force | Out-Null

$baseUrl = "http://127.0.0.1:$Port"
$gatewayApiKey = "temporary-local-api-$([guid]::NewGuid().ToString('N'))"
$managementToken = "temporary-local-management-$([guid]::NewGuid().ToString('N'))"
$apiHeaders = @{ Authorization = "Bearer $gatewayApiKey" }
$managementHeaders = @{ "x-management-token" = $managementToken }
$logRoot = Join-Path $evidenceParent "gateway-packaged-smoke-$Port-$([guid]::NewGuid().ToString('N').Substring(0, 12))"
New-Item -ItemType Directory -Path $logRoot -Force | Out-Null
$stateRoot = Join-Path $logRoot "gateway-state"
New-Item -ItemType Directory -Path $stateRoot -Force | Out-Null
$stdoutPath = Join-Path $logRoot "gateway.stdout.log"
$stderrPath = Join-Path $logRoot "gateway.stderr.log"

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
    "RUST_LOG"
)
$previousEnvironment = @{}
foreach ($key in $environmentKeys) {
    $previousEnvironment[$key] = [System.Environment]::GetEnvironmentVariable($key, "Process")
}

$process = $null
$docker = $null
$temporaryRedis = $null
$containerStarted = $false
$containerRemoved = $false
$redisMode = if ([string]::IsNullOrWhiteSpace($RedisUrl)) { "disposable-docker" } else { "explicit-url" }
$startedAt = [DateTimeOffset]::UtcNow
$checks = [System.Collections.Generic.List[object]]::new()
try {
    if ($redisMode -eq "disposable-docker") {
        $docker = Resolve-NativeExecutable -Command $DockerPath -Name "docker"
        $temporaryRedis = Start-TemporaryRedis `
            -DockerExecutable $docker `
            -Image $RedisImage `
            -Database $RedisDatabase `
            -StartupTimeoutSeconds $StartupTimeoutSeconds
        $containerStarted = $true
        $RedisUrl = $temporaryRedis.Url
        Write-Smoke "started disposable Redis container=$($temporaryRedis.ContainerName) hostPort=$($temporaryRedis.HostPort)"
    }

    $env:GATEWAY_RUNTIME_ROLE = "standalone"
    $env:PORT = [string]$Port
    $env:GATEWAY_REDIS_URL = $RedisUrl
    $env:GATEWAY_API_KEY = $gatewayApiKey
    $env:GATEWAY_API_KEY_SECRET = "temporary-local-signing-$([guid]::NewGuid().ToString('N'))"
    $env:GATEWAY_MANAGEMENT_TOKEN = $managementToken
    $env:GATEWAY_PUBLIC_BASE_URL = $baseUrl
    $env:GATEWAY_ROUTES_FILE = Join-Path $releaseDirFull "routes.yaml"
    $env:GATEWAY_STATE_DIR = $stateRoot
    $env:RUST_LOG = "info"

    $startArguments = @{
        FilePath = $gatewayBinary
        WorkingDirectory = $releaseDirFull
        RedirectStandardOutput = $stdoutPath
        RedirectStandardError = $stderrPath
        PassThru = $true
        WindowStyle = "Hidden"
    }
    $process = Start-Process @startArguments
    Write-Smoke "started packaged runtime pid=$($process.Id) port=$Port"

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $health = $null
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ($process.HasExited) {
            $stderrTail = if (Test-Path -LiteralPath $stderrPath) {
                (Get-Content -LiteralPath $stderrPath -Tail 50 -Encoding UTF8) -join "`n"
            } else { "" }
            throw "Packaged Gateway exited before /healthz became ready: $stderrTail"
        }
        $health = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/healthz" -RetryCount 3
        if ($health.StatusCode -eq 200) {
            break
        }
        Start-Sleep -Milliseconds 250
    }
    Assert-SmokeStatus -Name "GET /healthz" -Expected 200 -Response $health
    $checks.Add([ordered]@{ name = "healthz"; status = 200 }) | Out-Null

    $ready = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/readyz"
    Assert-SmokeStatus -Name "GET /readyz" -Expected 200 -Response $ready
    $checks.Add([ordered]@{ name = "readyz"; status = 200 }) | Out-Null

    $models = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/v1/models" -Headers $apiHeaders
    Assert-SmokeStatus -Name "GET /v1/models" -Expected 200 -Response $models
    $checks.Add([ordered]@{ name = "models"; status = 200 }) | Out-Null

    $head = Invoke-SmokeHttpRequest -Method "HEAD" -Uri "$baseUrl/healthz"
    Assert-SmokeStatus -Name "HEAD /healthz" -Expected 200 -Response $head
    if (-not (Test-HeaderExists -Headers $head.Headers -Name "x-request-id")) {
        throw "HEAD /healthz is missing x-request-id"
    }
    $checks.Add([ordered]@{ name = "request-id"; status = 200 }) | Out-Null

    foreach ($endpoint in @("/v1/chat/completions", "/v1/messages", "/v1/responses")) {
        $response = Invoke-SmokeHttpRequest `
            -Method "POST" `
            -Uri "$baseUrl$endpoint" `
            -Headers $apiHeaders `
            -Body "{}"
        Assert-SmokeStatus -Name "POST $endpoint empty request" -Expected 400 -Response $response
        $checks.Add([ordered]@{ name = $endpoint; status = 400 }) | Out-Null
    }

    $unknown = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/v1/packaged-smoke-missing"
    Assert-SmokeStatus -Name "GET unknown route" -Expected 404 -Response $unknown
    $checks.Add([ordered]@{ name = "unknown-route"; status = 404 }) | Out-Null

    $metrics = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/metrics"
    Assert-SmokeStatus -Name "GET /metrics" -Expected 200 -Response $metrics
    if ($metrics.Content -notmatch "gateway_requests_total") {
        throw "/metrics does not expose gateway_requests_total"
    }
    $checks.Add([ordered]@{ name = "metrics"; status = 200 }) | Out-Null

    $drain = Invoke-SmokeHttpRequest `
        -Method "POST" `
        -Uri "$baseUrl/v1/internal/gateway/runtime/drain" `
        -Headers $managementHeaders `
        -Body '{"reason":"packaged_runtime_smoke"}'
    Assert-SmokeStatus -Name "POST runtime drain" -Expected 202 -Response $drain
    $checks.Add([ordered]@{ name = "drain"; status = 202 }) | Out-Null

    if (-not $process.WaitForExit($ShutdownTimeoutSeconds * 1000)) {
        throw "Packaged Gateway did not exit after authorized drain"
    }
    $process.WaitForExit() | Out-Null
    $process.Refresh()
    $processExitCode = [int]$process.ExitCode
    if ($processExitCode -ne 0) {
        throw "Packaged Gateway exited with code $processExitCode after authorized drain"
    }
    Write-Smoke "authorized drain completed with exit code 0; no leaked packaged Gateway process remains"

    if ($containerStarted) {
        $containerRemoved = Stop-TemporaryRedis `
            -DockerExecutable $docker `
            -ContainerName $temporaryRedis.ContainerName
        if (-not $containerRemoved) {
            throw "Disposable Redis container cleanup could not be verified."
        }
        $containerStarted = $false
        Write-Smoke "removed disposable Redis container=$($temporaryRedis.ContainerName)"
    }

    $evidence = [ordered]@{
        schemaVersion = 1
        kind = "gateway-packaged-runtime-smoke"
        status = "pass"
        pathBase = "evidence-file-directory"
        releaseDir = Get-EvidenceRelativePath -BasePath $evidenceParent -Path $releaseDirFull
        binary = "gateway.exe"
        runtimeRole = "standalone"
        port = $Port
        startedAt = $startedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        checks = @($checks)
        processExitCode = $processExitCode
        manifestSha256 = $integrity.manifestSha256
        checksumsSha256 = $integrity.checksumsSha256
        stdoutPath = Get-EvidenceRelativePath -BasePath $evidenceParent -Path $stdoutPath
        stderrPath = Get-EvidenceRelativePath -BasePath $evidenceParent -Path $stderrPath
        redis = [ordered]@{
            mode = $redisMode
            isolated = ($redisMode -eq "disposable-docker")
            image = if ($null -eq $temporaryRedis) { $null } else { $temporaryRedis.Image }
            database = if ($null -eq $temporaryRedis) { $null } else { $RedisDatabase }
            hostPort = if ($null -eq $temporaryRedis) { $null } else { $temporaryRedis.HostPort }
            containerRemoved = if ($null -eq $temporaryRedis) { $null } else { $containerRemoved }
        }
    }
    [System.IO.File]::WriteAllText(
        $evidencePathFull,
        (($evidence | ConvertTo-Json -Depth 8) + [Environment]::NewLine),
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Smoke "evidence=$evidencePathFull"
} finally {
    if ($null -ne $process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        $process.WaitForExit(5000) | Out-Null
    }
    if ($containerStarted -and $null -ne $docker -and $null -ne $temporaryRedis) {
        try {
            $containerRemoved = Stop-TemporaryRedis `
                -DockerExecutable $docker `
                -ContainerName $temporaryRedis.ContainerName
            $containerStarted = $false
        } catch {
            Write-Warning "Failed to clean disposable Redis container $($temporaryRedis.ContainerName): $($_.Exception.Message)"
        }
    }
    foreach ($key in $environmentKeys) {
        [System.Environment]::SetEnvironmentVariable(
            $key,
            $previousEnvironment[$key],
            "Process"
        )
    }
}
