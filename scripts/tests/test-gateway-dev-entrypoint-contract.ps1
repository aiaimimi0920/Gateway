[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$entrypointPath = Join-Path $repoRoot "deploy\docker-dev-entrypoint.sh"
$composePath = Join-Path $repoRoot "deploy\docker-compose.dev.yml"

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

Assert-True (Test-Path -LiteralPath $entrypointPath) "Missing dev entrypoint script: $entrypointPath"
Assert-True (Test-Path -LiteralPath $composePath) "Missing dev compose file: $composePath"

$entrypointSource = Get-Content -LiteralPath $entrypointPath -Raw
$composeSource = Get-Content -LiteralPath $composePath -Raw

Assert-True ($entrypointSource.Contains(': "${GATEWAY_DEV_RUN_AUDIT:=0}"')) "Dev entrypoint must default GATEWAY_DEV_RUN_AUDIT to 0 so audit does not block the dev stack."
Assert-True ($entrypointSource.Contains('skipping production dependency audit in dev entrypoint')) "Dev entrypoint must log when audit is skipped."
Assert-True ($entrypointSource.Contains('if [[ "${GATEWAY_DEV_RUN_AUDIT}" != "1" ]]; then')) "Dev entrypoint must guard audit execution behind GATEWAY_DEV_RUN_AUDIT."
Assert-True ($composeSource.Contains('GATEWAY_DEV_RUN_AUDIT=${GATEWAY_DEV_RUN_AUDIT:-0}')) "Dev compose must expose GATEWAY_DEV_RUN_AUDIT with a default of 0."
Assert-True ($entrypointSource.Contains(': "${GATEWAY_DEV_WATCH:=0}"')) "Dev entrypoint must default to stable one-shot mode."
Assert-True ($composeSource.Contains('GATEWAY_DEV_WATCH=${GATEWAY_DEV_WATCH:-0}')) "Dev compose must expose the opt-in watch mode flag."
Assert-True ($entrypointSource.Contains('if [[ "${GATEWAY_DEV_WATCH}" == "1" ]]; then')) "Dev entrypoint must start polling watchers only when explicitly requested."
Assert-True ($entrypointSource.Contains('exec "${WORKSPACE_ROOT}/target/debug/gateway"')) "Stable dev mode must exec the built Gateway without a resident Cargo wrapper."
Assert-True (-not $composeSource.Contains('- WATCHPACK_POLLING=true')) "Dev compose must not force Watchpack polling in stable mode."
Assert-True (-not $composeSource.Contains('- CHOKIDAR_USEPOLLING=1')) "Dev compose must not force Chokidar polling in stable mode."
Assert-True ($composeSource.Contains('gateway-build:')) "Dev compose must isolate compilation in a one-shot builder service."
Assert-True (([regex]::Matches($composeSource, '(?m)^\s{4}build:\s+\*gateway-dev-build\s*$')).Count -eq 1) "Only the one-shot builder service may own the dev image build."
Assert-True ($composeSource.Contains('GATEWAY_DEV_MODE=build')) "Builder service must select build-only entrypoint mode."
Assert-True ($composeSource.Contains('GATEWAY_DEV_MODE=run')) "Gateway service must select run-only entrypoint mode."
Assert-True ($composeSource.Contains('condition: service_completed_successfully')) "Gateway must start only after the one-shot builder succeeds."
Assert-True ($entrypointSource.Contains(': "${GATEWAY_DEV_MODE:=all}"')) "Direct dev image use must retain a build-and-run fallback mode."
Assert-True ($entrypointSource.Contains('"${GATEWAY_DEV_MODE}" == "build"')) "Entrypoint must implement build-only mode."
Assert-True ($entrypointSource.Contains('"${GATEWAY_DEV_MODE}" == "run"')) "Entrypoint must implement run-only mode."

Write-Host "Gateway dev entrypoint contract passed."
