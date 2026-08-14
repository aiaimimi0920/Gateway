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

Write-Host "Gateway dev entrypoint contract passed."
