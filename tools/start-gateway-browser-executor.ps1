[CmdletBinding()]
param(
    [string]$EnvFile = "",
    [int]$Port = 42341,
    [string]$SunoBrowserCdpUrl = "http://127.0.0.1:9226",
    [string]$SunoBrowserCdpTargetUrl = "https://suno.com/create",
    [string]$UdioBrowserCdpUrl = "http://127.0.0.1:9225",
    [string]$UdioBrowserCdpTargetUrl = "https://www.udio.com/create",
    [ValidateRange(0, 1800000)]
    [int]$UdioHcaptchaManualWaitMs = 300000,
    [switch]$ForceRestart
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-FullPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [switch]$RequireExisting
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($RequireExisting -and -not (Test-Path -LiteralPath $fullPath)) {
        throw "Path does not exist: $fullPath"
    }
    return $fullPath
}

function Get-DotEnvValue {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Key
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $null
    }

    $escapedKey = [Regex]::Escape($Key)
    foreach ($line in @(Get-Content -LiteralPath $Path -Encoding UTF8)) {
        if ($line -match ("^{0}=(.*)$" -f $escapedKey)) {
            return $Matches[1]
        }
    }

    return $null
}

function Ensure-ScriptsNodeDependencies {
    param(
        [Parameter(Mandatory = $true)][string]$ScriptsRoot
    )

    $lockPath = Join-Path $ScriptsRoot "package-lock.json"
    $nodeModules = Join-Path $ScriptsRoot "node_modules"
    $stampPath = Join-Path $nodeModules ".gateway-package-lock.sha256"
    $lockHash = (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $needsInstall =
        -not (Test-Path -LiteralPath $nodeModules -PathType Container) -or
        -not (Test-Path -LiteralPath $stampPath -PathType Leaf) -or
        ((Get-Content -LiteralPath $stampPath -Encoding UTF8 -Raw).Trim().ToLowerInvariant() -ne $lockHash)

    if (-not $needsInstall) {
        return
    }

    & npm ci --prefix $ScriptsRoot --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) {
        throw "npm ci failed for $ScriptsRoot with exit code $LASTEXITCODE."
    }
    [System.IO.File]::WriteAllText($stampPath, $lockHash, [System.Text.UTF8Encoding]::new($false))
}

function Get-ExistingProcess {
    param(
        [Parameter(Mandatory = $true)][string]$PidFile
    )

    if (-not (Test-Path -LiteralPath $PidFile -PathType Leaf)) {
        return $null
    }

    try {
        $raw = Get-Content -LiteralPath $PidFile -Encoding UTF8 -Raw
        $payload = $raw | ConvertFrom-Json
        if ($null -eq $payload -or $null -eq $payload.pid) {
            return $null
        }
        return Get-Process -Id ([int]$payload.pid) -ErrorAction Stop
    } catch {
        return $null
    }
}

function Test-ServiceHealthy {
    param(
        [Parameter(Mandatory = $true)][string]$HealthUrl,
        [Parameter(Mandatory = $true)][string]$Token
    )

    try {
        $response = Invoke-RestMethod -Method Get -Uri $HealthUrl -Headers @{ Authorization = "Bearer $Token" } -TimeoutSec 5
        return ($response.ok -eq $true)
    } catch {
        return $false
    }
}

function First-NonEmptyValue {
    param(
        [Parameter(ValueFromRemainingArguments = $true)][AllowEmptyString()][string[]]$Values
    )

    foreach ($value in $Values) {
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            return $value
        }
    }

    return $null
}

$gatewayRoot = Resolve-FullPath -Path (Join-Path $PSScriptRoot "..") -RequireExisting
$deployDir = Resolve-FullPath -Path (Join-Path $gatewayRoot "deploy") -RequireExisting
$scriptsRoot = Resolve-FullPath -Path (Join-Path $gatewayRoot "scripts") -RequireExisting
$scriptPath = Resolve-FullPath -Path (Join-Path $scriptsRoot "local-browser-executor-service.mjs") -RequireExisting
$envPath = if ([string]::IsNullOrWhiteSpace($EnvFile)) {
    Resolve-FullPath -Path (Join-Path $deployDir ".env")
} else {
    Resolve-FullPath -Path $EnvFile -RequireExisting
}

$runtimeDir = Join-Path $deployDir "gateway_data\browser-executor"
$pidFile = Join-Path $runtimeDir "service.pid.json"
$stdoutLog = Join-Path $runtimeDir "stdout.log"
$stderrLog = Join-Path $runtimeDir "stderr.log"
$udioDebugLogPath = Join-Path $runtimeDir "udio-worker.debug.jsonl"
$token = First-NonEmptyValue `
    (Get-DotEnvValue -Path $envPath -Key "GATEWAY_GEMINI_AUTH_EXECUTOR_BEARER_TOKEN") `
    (Get-DotEnvValue -Path $envPath -Key "GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN") `
    "gateway-gemini-auth-dev"
$healthUrl = "http://127.0.0.1:$Port/v1/internal/browser-executor/health"

New-Item -ItemType Directory -Path $runtimeDir -Force | Out-Null
[void](Get-Command node -ErrorAction Stop)
Ensure-ScriptsNodeDependencies -ScriptsRoot $scriptsRoot

$existingProcess = Get-ExistingProcess -PidFile $pidFile
if ($existingProcess) {
    if ($ForceRestart -or -not (Test-ServiceHealthy -HealthUrl $healthUrl -Token $token)) {
        Stop-Process -Id $existingProcess.Id -Force
        Start-Sleep -Milliseconds 500
        Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
    } else {
        Write-Host "Gateway host browser executor already running: pid=$($existingProcess.Id) url=$healthUrl"
        return
    }
}

$previousPort = $env:LOCAL_BROWSER_EXECUTOR_PORT
$previousToken = $env:GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN
$previousSunoCdpUrl = $env:SUNO_BROWSER_CDP_URL
$previousSunoCdpTargetUrl = $env:SUNO_BROWSER_CDP_TARGET_URL
$previousUdioCdpUrl = $env:UDIO_BROWSER_CDP_URL
$previousUdioCdpTargetUrl = $env:UDIO_BROWSER_CDP_TARGET_URL
$previousUdioHcaptchaManualWaitMs = $env:UDIO_HCAPTCHA_MANUAL_WAIT_MS
$previousUdioBrowserDebugLogPath = $env:UDIO_BROWSER_DEBUG_LOG_PATH
try {
    $env:LOCAL_BROWSER_EXECUTOR_PORT = [string]$Port
    $env:GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN = $token
    $env:SUNO_BROWSER_CDP_URL = $SunoBrowserCdpUrl
    $env:SUNO_BROWSER_CDP_TARGET_URL = $SunoBrowserCdpTargetUrl
    $env:UDIO_BROWSER_CDP_URL = $UdioBrowserCdpUrl
    $env:UDIO_BROWSER_CDP_TARGET_URL = $UdioBrowserCdpTargetUrl
    $env:UDIO_HCAPTCHA_MANUAL_WAIT_MS = [string]$UdioHcaptchaManualWaitMs
    $env:UDIO_BROWSER_DEBUG_LOG_PATH = $udioDebugLogPath
    $process = Start-Process `
        -FilePath "node" `
        -ArgumentList @($scriptPath) `
        -WorkingDirectory $gatewayRoot `
        -RedirectStandardOutput $stdoutLog `
        -RedirectStandardError $stderrLog `
        -WindowStyle Hidden `
        -PassThru
} finally {
    if ($null -ne $previousPort) {
        $env:LOCAL_BROWSER_EXECUTOR_PORT = $previousPort
    } else {
        Remove-Item Env:LOCAL_BROWSER_EXECUTOR_PORT -ErrorAction SilentlyContinue
    }
    if ($null -ne $previousToken) {
        $env:GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN = $previousToken
    } else {
        Remove-Item Env:GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN -ErrorAction SilentlyContinue
    }
    foreach ($entry in @(
        @{ Name = "SUNO_BROWSER_CDP_URL"; Value = $previousSunoCdpUrl },
        @{ Name = "SUNO_BROWSER_CDP_TARGET_URL"; Value = $previousSunoCdpTargetUrl },
        @{ Name = "UDIO_BROWSER_CDP_URL"; Value = $previousUdioCdpUrl },
        @{ Name = "UDIO_BROWSER_CDP_TARGET_URL"; Value = $previousUdioCdpTargetUrl },
        @{ Name = "UDIO_HCAPTCHA_MANUAL_WAIT_MS"; Value = $previousUdioHcaptchaManualWaitMs },
        @{ Name = "UDIO_BROWSER_DEBUG_LOG_PATH"; Value = $previousUdioBrowserDebugLogPath }
    )) {
        if ($null -ne $entry.Value) {
            Set-Item -LiteralPath ("Env:" + $entry.Name) -Value $entry.Value
        } else {
            Remove-Item -LiteralPath ("Env:" + $entry.Name) -ErrorAction SilentlyContinue
        }
    }
}

$healthy = $false
for ($attempt = 0; $attempt -lt 30; $attempt++) {
    Start-Sleep -Seconds 1
    if ($process.HasExited) {
        break
    }
    if (Test-ServiceHealthy -HealthUrl $healthUrl -Token $token) {
        $healthy = $true
        break
    }
}

if (-not $healthy) {
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
    $stderrTail = if (Test-Path -LiteralPath $stderrLog -PathType Leaf) {
        (Get-Content -LiteralPath $stderrLog -Encoding UTF8 -Tail 40) -join [Environment]::NewLine
    } else {
        ""
    }
    throw "Gateway host browser executor failed to start on $healthUrl. stderr: $stderrTail"
}

$pidPayload = [ordered]@{
    pid = $process.Id
    port = $Port
    healthUrl = $healthUrl
    startedAt = [DateTimeOffset]::UtcNow.ToString("o")
}
$pidPayload | ConvertTo-Json | Set-Content -LiteralPath $pidFile -Encoding UTF8

Write-Host "Gateway host browser executor started: pid=$($process.Id) url=$healthUrl"
