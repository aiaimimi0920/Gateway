[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ReleaseDir,
    [string]$EvidenceRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
function Write-Smoke([string]$Message) { Write-Host "[gateway-local-smoke] $Message" }
. (Join-Path $PSScriptRoot "smoke-gateway-packaged-runtime.integrity.ps1")
. (Join-Path $PSScriptRoot "smoke-gateway-packaged-runtime.runtime.ps1")
$release = [System.IO.Path]::GetFullPath($ReleaseDir)
$null = Assert-PackagedReleaseIntegrity -ReleaseRoot $release
$binary = Join-Path $release "gateway.exe"
if ([string]::IsNullOrWhiteSpace($EvidenceRoot)) {
    $EvidenceRoot = Join-Path $PSScriptRoot "../target/local-storage-smoke-$([guid]::NewGuid().ToString('N'))"
}
$evidence = [System.IO.Path]::GetFullPath($EvidenceRoot)
if (Test-Path -LiteralPath $evidence) { throw "EvidenceRoot must be new: $evidence" }
New-Item -ItemType Directory -Path $evidence | Out-Null
$data = Join-Path $evidence "data"
$port = Get-FreeTcpPort
$base = "http://127.0.0.1:$port"
$management = "local-smoke-$([guid]::NewGuid().ToString('N'))"
$headers = @{ "x-management-token" = $management }
$checks = [System.Collections.Generic.List[string]]::new()
$process = $null
$traps = @()
$overrides = @{
    PORT = [string]$port
    GATEWAY_STORAGE_MODE = "local"
    GATEWAY_RUNTIME_ROLE = "standalone"
    GATEWAY_DATA_DIR = $data
    GATEWAY_BIND_HOST = "127.0.0.1"
    GATEWAY_MANAGEMENT_TOKEN = $management
    GATEWAY_API_KEY = $null
    GATEWAY_API_KEY_SECRET = $null
    GATEWAY_RELEASE_PAYLOAD_ROOT = $release
    GATEWAY_REDIS_URL = $null
    GATEWAY_DATABASE_URL = $null
    DATABASE_URL = $null
    GATEWAY_DESKTOP_MANAGED = $null
    GATEWAY_CONSOLE_STORAGE = $null
}
$previous = @{}
foreach ($key in $overrides.Keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, "Process") }

function Invoke-LocalJson([string]$Method, [string]$Path, [object]$Body, [hashtable]$Auth = $headers, [int]$Status = 200) {
    $json = if ($null -eq $Body) { "" } else { $Body | ConvertTo-Json -Depth 10 -Compress }
    $response = Invoke-SmokeHttpRequest -Method $Method -Uri "$base$Path" -Headers $Auth -Body $json
    Assert-SmokeStatus -Name "$Method $Path" -Expected $Status -Response $response
    return ($response.Content | ConvertFrom-Json)
}

function Start-LocalBackend([string]$Phase) {
    $child = Start-Process -FilePath $binary -WorkingDirectory $release -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $evidence "$Phase.stdout.log") `
        -RedirectStandardError (Join-Path $evidence "$Phase.stderr.log")
    $null = $child.Handle
    $script:process = $child
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    do {
        if ($child.HasExited) { throw "Local backend exited during $Phase; inspect the isolated logs" }
        $ready = Invoke-SmokeHttpRequest -Method GET -Uri "$base/readyz"
        if ($ready.StatusCode -eq 200) {
            $body = $ready.Content | ConvertFrom-Json
            if ($body.storage_backend -ne "sqlite" -or $body.degraded -or $body.redis_required) {
                throw "Local readiness did not select SQLite"
            }
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Local backend startup timed out"
}

function Stop-LocalBackend {
    $null = Invoke-LocalJson POST "/v1/internal/gateway/runtime/drain" @{ reason = "local_storage_smoke" } $headers 202
    if (-not $script:process.WaitForExit(15000)) { throw "Local backend did not drain" }
    if ($script:process.ExitCode -ne 0) { throw "Local backend exited unsuccessfully" }
    $script:process.Dispose()
    $script:process = $null
}

try {
    # Listening traps prove that no external database connection is attempted.
    foreach ($index in 0..1) {
        $trap = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
        $trap.Start()
        $traps += $trap
    }
    $overrides.GATEWAY_REDIS_URL = "redis://127.0.0.1:$($traps[0].LocalEndpoint.Port)"
    $overrides.GATEWAY_DATABASE_URL = "postgres://fixture:fixture@127.0.0.1:$($traps[1].LocalEndpoint.Port)/fixture"
    foreach ($key in $overrides.Keys) { [Environment]::SetEnvironmentVariable($key, $overrides[$key], "Process") }
    Start-LocalBackend "first"
    $checks.Add("SQLite readiness with empty routes")
    $key = Invoke-LocalJson POST "/v1/internal/gateway/access/keys" @{
        ownerType = "user"; ownerId = "fixture"; resolvedProjectId = "local"; resolvedTenantId = "local"
        keyKind = "normal"; publicKeyPrefix = "sk-gw"; displayName = "local storage smoke"
    }
    $id = $key.id
    $serving = @{ Authorization = "Bearer $($key.token)" }
    $balancePath = "/v1/internal/gateway/access/keys/$id/balance"
    $balance = Invoke-LocalJson GET $balancePath $null
    if ($balance.balanceMode -ne "unlimited") { throw "Missing compatibility balance" }
    $null = Invoke-LocalJson POST "/v1/internal/gateway/access/keys/$id/balances/adjust" @{
        balanceMode = "message_prepaid"; totalMessages = 0; remainingMessages = 0
    }
    $inference = @{ model = "nvidia-smoke"; messages = @(@{ role = "user"; content = "local fixture" }) }
    $denied = Invoke-LocalJson POST "/v1/chat/completions" $inference $serving 429
    if ($denied.error.code -ne "message_balance_exhausted") { throw "Quota was not enforced before routing" }
    $checks.Add("Exhausted key denied before upstream execution")
    Stop-LocalBackend
    Start-LocalBackend "restart"
    $balance = Invoke-LocalJson GET $balancePath $null
    if ($balance.remainingMessages -ne 0) { throw "Balance did not survive restart" }
    $null = Invoke-LocalJson GET "/v1/models" $null $serving
    $checks.Add("Key and balance survive restart")
    $null = Invoke-LocalJson POST "/v1/internal/gateway/access/keys/$id/balances/adjust" @{ messageDelta = 1 }
    $null = Invoke-LocalJson POST "/v1/chat/completions" $inference $serving 400
    $balance = Invoke-LocalJson GET $balancePath $null
    if ($balance.remainingMessages -ne 1 -or $balance.totalMessages -ne 1) { throw "Failure refund changed purchased credit" }
    $checks.Add("Routing failure refunds remaining credit only")
    $rotated = Invoke-LocalJson POST "/v1/internal/gateway/access/keys/$id/rotate" @{}
    $balance = Invoke-LocalJson GET "/v1/internal/gateway/access/keys/$($rotated.id)/balance" $null
    if ($balance.remainingMessages -ne 1) { throw "Rotation lost the balance account" }
    $null = Invoke-LocalJson GET "/v1/models" $null $serving 401
    $checks.Add("Rotation preserves balance and revokes old token")
    Stop-LocalBackend
    foreach ($trap in $traps) { if ($trap.Pending()) { throw "Local mode attempted an external database connection" } }
    $checks.Add("Zero Redis and PostgreSQL connections")
    $sqlite = Join-Path $data "local/state/runtime.sqlite3"
    if (-not (Test-Path -LiteralPath $sqlite -PathType Leaf)) { throw "Missing durable SQLite state" }
    $report = [ordered]@{
        passed = $true; storageBackend = "sqlite"; checks = $checks.ToArray(); modelCalls = 0
        executableSha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
        sqliteSha256 = (Get-FileHash -LiteralPath $sqlite -Algorithm SHA256).Hash.ToLowerInvariant()
        processesDrained = $true
    }
    $reportPath = Join-Path $evidence "result.json"
    [IO.File]::WriteAllText($reportPath, ($report | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    Write-Smoke "PASS: $reportPath"
} finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        $process.Dispose()
    }
    foreach ($trap in $traps) { $trap.Stop() }
    foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], "Process") }
}
