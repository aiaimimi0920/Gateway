[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Output,
    [switch]$ServerRestart,
    [switch]$McpCreation,
    [switch]$McpRestart
)
# Only a new tmpfs-backed container is used; existing databases and containers are never targets.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$outputPath = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $outputPath) { throw "Output already exists: $outputPath" }
if (@($ServerRestart, $McpCreation, $McpRestart).Where({ $_ }).Count -gt 1) { throw "Select only one Server contract" }
$run = [guid]::NewGuid().ToString("N")
$container = "gateway-luma-pg-$run"
$image = "postgres:16-bookworm"
$redisImage = "redis:7-alpine"
$redis = "gateway-luma-redis-$run"
$saved = @{}
foreach ($key in @("GATEWAY_LUMA_TEST_DATABASE_URL", "GATEWAY_LUMA_TEST_RUN_ID", `
        "GATEWAY_LUMA_TEST_REDIS_URL", "GATEWAY_LUMA_TEST_PROCESS_ROOT")) {
    $saved[$key] = [Environment]::GetEnvironmentVariable($key, "Process")
}
Push-Location -LiteralPath $repo
$owned = @()
try {
    & docker image inspect $image --format '{{.Id}}'
    if ($LASTEXITCODE -ne 0) { throw "Cached fixture image required: $image" }
    $ddl = Join-Path $repo "deploy/postgres/initdb"
    & docker run --detach --rm --name $container --label "gateway.luma.run=$run" `
        --tmpfs /var/lib/postgresql/data -e POSTGRES_PASSWORD=luma-fixture-only `
        -e POSTGRES_DB=gateway_luma_fixture -p '127.0.0.1::5432' `
        --mount "type=bind,source=$ddl,target=/docker-entrypoint-initdb.d,readonly" $image
    if ($LASTEXITCODE -ne 0) { throw "Could not start isolated PostgreSQL fixture" }
    $owned += $container
    $port = ((& docker port $container 5432/tcp).Trim() -split ':')[-1]
    $ready = $false
    for ($i = 0; $i -lt 120; $i++) {
        & docker exec $container pg_isready -h 127.0.0.1 -U postgres -d gateway_luma_fixture *> $null
        if ($LASTEXITCODE -eq 0) { $ready = $true; break }
        Start-Sleep -Milliseconds 500
    }
    if (-not $ready) { throw "PostgreSQL fixture did not become ready" }
    & docker exec $container psql -U postgres -d gateway_luma_fixture -v ON_ERROR_STOP=1 -c `
        "CREATE TABLE luma_fixture_guard(run_id text NOT NULL); INSERT INTO luma_fixture_guard VALUES ('$run');"
    if ($LASTEXITCODE -ne 0) { throw "Could not bind fixture run identity" }
    $env:GATEWAY_LUMA_TEST_DATABASE_URL = "postgres://postgres:luma-fixture-only@127.0.0.1:$port/gateway_luma_fixture"
    $env:GATEWAY_LUMA_TEST_RUN_ID = $run
    $filter = "luma_reconciliation_postgres"
    if ($ServerRestart -or $McpCreation -or $McpRestart) {
        & docker image inspect $redisImage --format '{{.Id}}'
        if ($LASTEXITCODE -ne 0) { throw "Cached fixture image required: $redisImage" }
        & docker run --detach --rm --name $redis --label "gateway.luma.run=$run" `
            --tmpfs /data -p '127.0.0.1::6379' $redisImage redis-server --appendonly no
        if ($LASTEXITCODE -ne 0) { throw "Could not start isolated Redis fixture" }
        $owned += $redis
        $redisPort = ((& docker port $redis 6379/tcp).Trim() -split ':')[-1]
        $redisReady = $false
        for ($i = 0; $i -lt 60; $i++) {
            $pong = & docker exec $redis redis-cli ping 2>$null
            if ($LASTEXITCODE -eq 0 -and $pong -eq "PONG") { $redisReady = $true; break }
            Start-Sleep -Milliseconds 500
        }
        if (-not $redisReady) { throw "Redis fixture did not become ready" }
        $env:GATEWAY_LUMA_TEST_REDIS_URL = "redis://127.0.0.1:$redisPort/0"
        $env:GATEWAY_LUMA_TEST_PROCESS_ROOT = Join-Path $outputPath "restart-fixture"
        $filter = if ($McpCreation) { "luma_mcp_creation_server_durable_owner" } `
            elseif ($McpRestart) { "luma_mcp_server_restart_after_os_kill" } `
            else { "luma_reconciliation_server_restart_after_os_kill" }
    }
    & python (Join-Path $PSScriptRoot "run-check.py") --output $outputPath -- `
        cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 `
        $filter -- --ignored --test-threads=1 --nocapture
    if ($LASTEXITCODE -ne 0) { throw "Luma PostgreSQL contract failed; see $outputPath" }
} finally {
    foreach ($name in $owned) {
        $labelJson = & docker inspect --format '{{json .Config.Labels}}' $name
        if ($LASTEXITCODE -eq 0 -and ($labelJson | ConvertFrom-Json).'gateway.luma.run' -eq $run) {
            & docker stop --time 10 $name
            if ($LASTEXITCODE -ne 0) { Write-Warning "Could not stop owned fixture: $name" }
        } else { Write-Warning "Fixture identity changed; container left untouched: $name" }
    }
    foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key], "Process") }
    Pop-Location
}
