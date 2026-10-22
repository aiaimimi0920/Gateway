[CmdletBinding()]
param(
    [ValidateSet("api", "web", "desktop")][string]$Edition = "desktop",
    [ValidateRange(1, 32)][int]$Jobs = [Math]::Max(1, [Math]::Min(4, [Environment]::ProcessorCount)),
    [string]$VersionId = "gateway-devtest-$(Get-Date -Format 'yyyyMMdd-HHmmss')",
    [switch]$DryRun
)
# All disposable staging stays in linshi; the published directory is never overwritten.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$scratch = "C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-devtest-packages"
$release = "C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway"
if ($VersionId -notmatch '^gateway-devtest-[A-Za-z0-9][A-Za-z0-9._-]*$') { throw "Invalid devtest VersionId" }
$destination = Join-Path $release $VersionId
if (Test-Path -LiteralPath $destination) { throw "Destination already exists: $destination" }
if ($DryRun) {
    & "$PSScriptRoot/build-gateway-devtest.ps1" -Edition $Edition -Jobs $Jobs -DryRun
    "package buildKind=devtest staging=$scratch destination=$destination"
    return
}
& "$PSScriptRoot/build-gateway-devtest.ps1" -Edition $Edition -Jobs $Jobs
& "$PSScriptRoot/package-gateway-release.ps1" -Edition $Edition -BuildKind devtest -VersionId $VersionId `
    -SkipBuild -UseExampleRoutes -ReleaseRoot $scratch -AllowCustomReleaseRoot
$package = Join-Path $scratch $VersionId
& "$PSScriptRoot/smoke-gateway-ui-release.ps1" -ReleaseDir $package
New-Item -ItemType Directory -Path $release -Force | Out-Null
# Creating the directory first fails on a publication race, rather than merging trees.
New-Item -ItemType Directory -Path $destination -ErrorAction Stop | Out-Null
Get-ChildItem -LiteralPath $package -Force | Copy-Item -Destination $destination -Recurse
& "$PSScriptRoot/smoke-gateway-ui-release.ps1" -ReleaseDir $destination
Write-Host "[gateway-devtest] published $destination"
