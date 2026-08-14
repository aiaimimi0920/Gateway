[CmdletBinding()]
param(
    [ValidateSet("all", "suno", "udio")]
    [string]$Provider = "all"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$gatewayRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$chromePaths = @(
    "C:\Program Files\Google\Chrome\Application\chrome.exe",
    "C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"
)
$chromePath = $chromePaths | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (-not $chromePath) {
    throw "Google Chrome was not found."
}

$providers = [ordered]@{
    udio = [ordered]@{
        port = 9225
        url = "https://www.udio.com/create"
    }
    suno = [ordered]@{
        port = 9226
        url = "https://suno.com/create"
    }
}

function Test-LocalPortListening {
    param([Parameter(Mandatory = $true)][int]$Port)

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $result = $client.BeginConnect("127.0.0.1", $Port, $null, $null)
        if (-not $result.AsyncWaitHandle.WaitOne(500)) {
            return $false
        }
        $client.EndConnect($result)
        return $true
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

$selectedProviders = if ($Provider -eq "all") { @("udio", "suno") } else { @($Provider) }
$results = @()
foreach ($providerName in $selectedProviders) {
    $configuration = $providers[$providerName]
    if (Test-LocalPortListening -Port $configuration.port) {
        $results += [pscustomobject]@{
            provider = $providerName
            started = $false
            reason = "already-running"
            port = $configuration.port
        }
        continue
    }

    $profilePath = Join-Path $gatewayRoot "deploy\gateway_data\browser-profiles\$providerName"
    $statusPath = Join-Path $gatewayRoot "deploy\gateway_data\browser-profiles\$providerName.status.json"
    New-Item -ItemType Directory -Path $profilePath -Force | Out-Null

    $arguments = @(
        "--user-data-dir=$profilePath",
        "--remote-debugging-address=127.0.0.1",
        "--remote-debugging-port=$($configuration.port)",
        "--remote-allow-origins=*",
        "--no-first-run",
        "--no-default-browser-check",
        "--new-window",
        $configuration.url
    )
    $process = Start-Process `
        -FilePath $chromePath `
        -ArgumentList $arguments `
        -WorkingDirectory $gatewayRoot `
        -PassThru

    $ready = $false
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        Start-Sleep -Milliseconds 500
        if (Test-LocalPortListening -Port $configuration.port) {
            $ready = $true
            break
        }
        if ($process.HasExited) {
            break
        }
    }

    $status = [ordered]@{
        provider = $providerName
        pid = $process.Id
        port = $configuration.port
        url = $configuration.url
        profilePath = $profilePath
        ready = $ready
        startedAt = [DateTimeOffset]::UtcNow.ToString("o")
    }
    $status | ConvertTo-Json | Set-Content -LiteralPath $statusPath -Encoding UTF8
    $results += [pscustomobject]$status
}

$results | ConvertTo-Json -Depth 4
