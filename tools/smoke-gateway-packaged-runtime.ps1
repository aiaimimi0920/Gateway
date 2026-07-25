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

function Resolve-NativeExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if (Test-Path -LiteralPath $Command -PathType Leaf) {
        return [System.IO.Path]::GetFullPath($Command)
    }
    $resolved = @(Get-Command $Command -All -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandType -eq "Application" } |
        Select-Object -First 1)
    if ($resolved.Count -eq 0) {
        $resolved = @(Get-Command $Command -ErrorAction SilentlyContinue | Select-Object -First 1)
    }
    if ($resolved.Count -eq 0 -or $null -eq $resolved[0]) {
        throw "$Name executable was not found. Supply an explicit command path."
    }
    return $resolved[0].Source
}

function Invoke-NativeCommandCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $captureRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        "gateway-packaged-smoke-{0}" -f [guid]::NewGuid().ToString("N")
    )
    $stdoutPath = Join-Path $captureRoot "stdout.log"
    $stderrPath = Join-Path $captureRoot "stderr.log"
    New-Item -ItemType Directory -Path $captureRoot -Force | Out-Null

    try {
        $startInfo = @{
            FilePath = $Command
            ArgumentList = $Arguments
            RedirectStandardOutput = $stdoutPath
            RedirectStandardError = $stderrPath
            PassThru = $true
            Wait = $true
            WindowStyle = "Hidden"
        }
        $process = Start-Process @startInfo
        $stdoutLines = if (Test-Path -LiteralPath $stdoutPath -PathType Leaf) {
            @(Get-Content -LiteralPath $stdoutPath -Encoding UTF8)
        } else {
            @()
        }
        $stderrLines = if (Test-Path -LiteralPath $stderrPath -PathType Leaf) {
            @(Get-Content -LiteralPath $stderrPath -Encoding UTF8)
        } else {
            @()
        }

        return [pscustomobject]@{
            ExitCode = [int]$process.ExitCode
            Text = (($stdoutLines + $stderrLines) | ForEach-Object { [string]$_ }) -join "`n"
        }
    } finally {
        if (Test-Path -LiteralPath $captureRoot) {
            Remove-Item -LiteralPath $captureRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Invoke-Docker {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation,
        [switch]$AllowFailure
    )

    $result = Invoke-NativeCommandCapture -Command $DockerExecutable -Arguments $Arguments
    if ($result.ExitCode -ne 0 -and -not $AllowFailure) {
        $details = $result.Text.Trim()
        if ([string]::IsNullOrWhiteSpace($details)) {
            throw "$Operation failed with exit code $($result.ExitCode)."
        }
        throw "$Operation failed with exit code $($result.ExitCode): $details"
    }
    return [pscustomobject]@{
        ExitCode = $result.ExitCode
        Text = $result.Text.Trim()
    }
}

function Stop-TemporaryRedis {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string]$ContainerName
    )

    $removed = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("rm", "--force", $ContainerName) `
        -Operation "remove disposable Redis container" `
        -AllowFailure
    if ($removed.ExitCode -eq 0) {
        return $true
    }

    $inspect = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("inspect", $ContainerName) `
        -Operation "inspect disposable Redis container" `
        -AllowFailure
    return $inspect.ExitCode -ne 0
}

function Start-TemporaryRedis {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string]$Image,
        [Parameter(Mandatory = $true)][int]$Database,
        [Parameter(Mandatory = $true)][int]$StartupTimeoutSeconds
    )

    $runId = [guid]::NewGuid().ToString("N")
    $containerName = "gateway-packaged-smoke-redis-$($runId.Substring(0, 12))"
    $containerStarted = $false
    try {
        $run = Invoke-Docker `
            -DockerExecutable $DockerExecutable `
            -Arguments @(
                "run",
                "--detach",
                "--name",
                $containerName,
                "--publish",
                "127.0.0.1::6379",
                "--label",
                "neuro.gateway.packaged-smoke=$runId",
                $Image
            ) `
            -Operation "start disposable Redis container"
        if ([string]::IsNullOrWhiteSpace($run.Text)) {
            throw "docker run did not return a disposable Redis container id."
        }
        $containerStarted = $true

        $portDeadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
        $hostPort = 0
        while ([DateTimeOffset]::UtcNow -lt $portDeadline) {
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

        $pingDeadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
        while ([DateTimeOffset]::UtcNow -lt $pingDeadline) {
            $ping = Invoke-Docker `
                -DockerExecutable $DockerExecutable `
                -Arguments @(
                    "exec",
                    $containerName,
                    "redis-cli",
                    "-n",
                    [string]$Database,
                    "PING"
                ) `
                -Operation "ping disposable Redis" `
                -AllowFailure
            if ($ping.ExitCode -eq 0 -and $ping.Text.Trim() -eq "PONG") {
                return [pscustomobject]@{
                    ContainerName = $containerName
                    HostPort = $hostPort
                    Url = "redis://127.0.0.1:$hostPort/$Database"
                    Image = $Image
                }
            }
            Start-Sleep -Milliseconds 200
        }
        throw "Disposable Redis did not become ready before the timeout."
    } catch {
        if ($containerStarted) {
            $null = Stop-TemporaryRedis `
                -DockerExecutable $DockerExecutable `
                -ContainerName $containerName
        }
        throw
    }
}

function Get-EvidenceRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseFull = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
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

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseFull = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Assert-SafePackageRelativePath {
    param([Parameter(Mandatory = $true)][string]$Path)

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
    param([Parameter(Mandatory = $true)][string]$Path)

    $entries = [System.Collections.Generic.Dictionary[string, string]]::new(
        [System.StringComparer]::Ordinal
    )
    $lineNumber = 0
    foreach ($line in (Get-Content -LiteralPath $Path -Encoding ASCII)) {
        $lineNumber++
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        # The checksum contract is exactly: 64 hex chars, two spaces, path.
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
        [Parameter(Mandatory = $true)][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Assert-PackagedReleaseIntegrity {
    param([Parameter(Mandatory = $true)][string]$ReleaseRoot)

    $manifestPath = Join-Path $ReleaseRoot "manifest.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "Missing manifest.json in packaged release: $ReleaseRoot"
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $checksumsRelative = [string](Get-ObjectPropertyValue -Object $manifest -Name "checksums")
    if ($checksumsRelative -ne "checksums.sha256") {
        throw "Packaged manifest must point to checksums.sha256 exactly"
    }
    $checksumsPath = Join-Path $ReleaseRoot (Assert-SafePackageRelativePath -Path $checksumsRelative)
    if (-not (Test-Path -LiteralPath $checksumsPath -PathType Leaf)) {
        throw "Missing checksum file in packaged release: $checksumsPath"
    }
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

    $manifestFiles = @(Get-ObjectPropertyValue -Object $manifest -Name "files")
    foreach ($record in $manifestFiles) {
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
    foreach ($record in $manifestFiles) {
        $manifestListedPaths.Add([string](Get-ObjectPropertyValue -Object $record -Name "path")) | Out-Null
    }
    foreach ($relative in $checksumEntries.Keys) {
        if ($relative -ne "manifest.json" -and -not $manifestListedPaths.Contains($relative)) {
            throw "checksums.sha256 contains a payload not listed in manifest.files: $relative"
        }
    }

    $exeRecords = @(Get-ObjectPropertyValue -Object $manifest -Name "exes")
    $headlessRecords = @($exeRecords | Where-Object {
        ([string](Get-ObjectPropertyValue -Object $_ -Name "name") -eq "neuro-gateway.exe") -and
        ([string](Get-ObjectPropertyValue -Object $_ -Name "path") -eq "neuro-gateway.exe")
    })
    if ($headlessRecords.Count -ne 1) {
        throw "manifest.exes must contain exactly one neuro-gateway.exe record at the package root"
    }

    return [pscustomobject]@{
        manifest = $manifest
        checksumEntries = $checksumEntries
        manifestSha256 = $actualFiles["manifest.json"]
        checksumsSha256 = (Get-FileHash -LiteralPath $checksumsPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function Invoke-SmokeHttpRequest {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Uri,
        [hashtable]$Headers = @{},
        [string]$Body = "",
        [string]$ContentType = "application/json",
        [ValidateRange(1, 10)][int]$RetryCount = 1
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
                TransportError = $false
            }
        } catch {
            $statusCode = 0
            $headers = @{}
            $responseProperty = $_.Exception.PSObject.Properties['Response']
            if ($null -ne $responseProperty -and $null -ne $responseProperty.Value) {
                try { $statusCode = [int]$responseProperty.Value.StatusCode } catch { $statusCode = 0 }
                try { $headers = $responseProperty.Value.Headers } catch { $headers = @{} }
            }
            $errorDetailsProperty = $_.PSObject.Properties['ErrorDetails']
            $errorDetails = if ($null -ne $errorDetailsProperty) { $errorDetailsProperty.Value } else { $null }
            $errorDetailsMessage = $null
            if ($null -ne $errorDetails) {
                $messageProperty = $errorDetails.PSObject.Properties['Message']
                if ($null -ne $messageProperty) {
                    $errorDetailsMessage = [string]$messageProperty.Value
                }
            }
            $content = if (-not [string]::IsNullOrWhiteSpace($errorDetailsMessage)) {
                $errorDetailsMessage
            } else {
                $_.Exception.Message
            }
            $result = [pscustomobject]@{
                StatusCode = $statusCode
                Content = [string]$content
                Headers = $headers
                TransportError = ($statusCode -eq 0)
            }
            if ($result.TransportError -and $attempt -lt $RetryCount) {
                Start-Sleep -Milliseconds ([Math]::Min(500, 100 * $attempt))
                continue
            }
            return $result
        }
    }
    throw "HTTP smoke request did not produce a result"
}

function Assert-SmokeStatus {
    param(
        [string]$Name,
        [int]$Expected,
        [object]$Response
    )

    if ($Response.StatusCode -ne $Expected) {
        throw "$Name expected HTTP $Expected but received $($Response.StatusCode): $($Response.Content)"
    }
    Write-Smoke "$Name HTTP $Expected"
}

function Test-HeaderExists {
    param([object]$Headers, [string]$Name)

    if ($null -eq $Headers) {
        return $false
    }
    foreach ($key in $Headers.Keys) {
        if ([string]::Equals([string]$key, $Name, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $true
        }
    }
    return $false
}

$releaseDirFull = [System.IO.Path]::GetFullPath($ReleaseDir)
if (-not (Test-Path -LiteralPath $releaseDirFull -PathType Container)) {
    throw "ReleaseDir does not exist: $releaseDirFull"
}
$gatewayBinary = Join-Path $releaseDirFull "neuro-gateway.exe"
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
        binary = "neuro-gateway.exe"
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
