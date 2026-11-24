[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Output)
# Dedicated tmpfs Redis fixture; no existing account, Redis namespace or database is used.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$run = [guid]::NewGuid().ToString("N")
$container = "gateway-luma-session-$run"
$previous = [Environment]::GetEnvironmentVariable("GATEWAY_LUMA_TEST_REDIS_URL", "Process")
$started = $false
Push-Location -LiteralPath $repo
try {
    & docker image inspect redis:7-alpine --format '{{.Id}}'
    if ($LASTEXITCODE -ne 0) { throw "Cached redis:7-alpine is required" }
    & docker run --detach --rm --name $container --label "gateway.luma.session=$run" `
        --tmpfs /data -p '127.0.0.1::6379' redis:7-alpine redis-server --appendonly no
    if ($LASTEXITCODE -ne 0) { throw "Could not create isolated Redis fixture" }
    $started = $true
    $port = ((& docker port $container 6379/tcp).Trim() -split ':')[-1]
    $env:GATEWAY_LUMA_TEST_REDIS_URL = "redis://127.0.0.1:$port/0"
    & python (Join-Path $PSScriptRoot "run-check.py") --output $Output -- `
        cargo test --locked --offline --lib --no-default-features --features runtime-defaults `
        --config 'profile.test.package.gateway.debug=0' -j 4 -- luma_web_session_redis_cas --ignored --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw "Luma Web Redis contract failed" }
} finally {
    if ($started) {
        $label = & docker inspect --format '{{json .Config.Labels}}' $container
        if ($LASTEXITCODE -eq 0 -and ($label | ConvertFrom-Json).'gateway.luma.session' -eq $run) {
            & docker stop --time 10 $container
            if ($LASTEXITCODE -ne 0) { Write-Warning "Could not stop owned fixture $container" }
        }
    }
    [Environment]::SetEnvironmentVariable("GATEWAY_LUMA_TEST_REDIS_URL", $previous, "Process")
    Pop-Location
}
