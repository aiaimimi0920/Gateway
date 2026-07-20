[CmdletBinding()]
param(
    [string]$PromtoolPath,
    [switch]$RequirePromtool
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$gatewayRoot = Split-Path -Parent $PSScriptRoot
$rulesPath = Join-Path $gatewayRoot "docs/operations-alerts.yaml"

if ([string]::IsNullOrWhiteSpace($PromtoolPath)) {
    $command = Get-Command "promtool" -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        $PromtoolPath = $command.Source
    }
}

if ([string]::IsNullOrWhiteSpace($PromtoolPath)) {
    $message = "promtool was not found; structural contracts still validate docs/operations-alerts.yaml, but external Prometheus rule validation was not executed."
    if ($RequirePromtool) {
        throw $message
    }
    Write-Warning $message
    return
}

& $PromtoolPath check rules $rulesPath
if ($LASTEXITCODE -ne 0) {
    throw "promtool check rules failed for docs/operations-alerts.yaml with exit code $LASTEXITCODE."
}

Write-Output "promtool check rules passed for docs/operations-alerts.yaml."
