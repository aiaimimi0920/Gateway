[CmdletBinding()]
param(
  [ValidateSet("all", "desktop-chromium", "mobile-chromium")]
  [string] $Project = "all",

  [switch] $InstallBrowsers,

  [switch] $AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$UiRoot = Join-Path $RepoRoot "apps\desktop"
$RuntimeRoot = Join-Path $RepoRoot ".runtime\console-e2e"
$LogRoot = Join-Path $RuntimeRoot "logs"
$ResultsRoot = Join-Path $RuntimeRoot "test-results"
$ArtifactsRoot = Join-Path $RuntimeRoot "artifacts"
$ConsoleSpecPath = "e2e/console.spec.ts"

function Write-E2ELog {
  param([Parameter(Mandatory = $true)][string] $Message)
  Write-Host "[gateway-console-e2e] $Message"
}

function Assert-ChildPath {
  param(
    [Parameter(Mandatory = $true)][string] $BasePath,
    [Parameter(Mandatory = $true)][string] $Path
  )

  $resolvedBase = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/")
  $resolvedPath = [System.IO.Path]::GetFullPath($Path)
  $prefix = $resolvedBase + [System.IO.Path]::DirectorySeparatorChar
  if (-not $resolvedPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to operate outside of runtime root: $resolvedPath"
  }
  return $resolvedPath
}

function Reset-Directory {
  param([Parameter(Mandatory = $true)][string] $Path)

  $resolved = Assert-ChildPath -BasePath $RuntimeRoot -Path $Path
  if (Test-Path -LiteralPath $resolved) {
    Remove-Item -LiteralPath $resolved -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $resolved | Out-Null
  return $resolved
}

function Invoke-LoggedCommand {
  param(
    [Parameter(Mandatory = $true)][string] $LogPath,
    [Parameter(Mandatory = $true)][string[]] $Command
  )

  Write-E2ELog ("running: " + ($Command -join " "))
  & $Command[0] @($Command | Select-Object -Skip 1) 2>&1 |
    Tee-Object -FilePath $LogPath -Append
  $exitCode = $LASTEXITCODE
  if ($exitCode -ne 0) {
    throw "Command failed with exit code ${exitCode}: $($Command -join ' ')"
  }
}

if (-not (Test-Path -LiteralPath $UiRoot -PathType Container)) {
  throw "UI workspace is missing: $UiRoot"
}

New-Item -ItemType Directory -Force -Path $RuntimeRoot | Out-Null
New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null
$ResultsRoot = Reset-Directory -Path $ResultsRoot
$ArtifactsRoot = Reset-Directory -Path $ArtifactsRoot

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$logPath = Join-Path $LogRoot "console-e2e-$timestamp.log"

if ($InstallBrowsers) {
  Invoke-LoggedCommand -LogPath $logPath -Command @(
    "npx",
    "--prefix",
    $UiRoot,
    "playwright",
    "install",
    "chromium"
  )
}

$playwrightArgs = @(
  "npm",
  "run",
  "e2e",
  "--prefix",
  $UiRoot,
  "--",
  "--output",
  $ResultsRoot,
  $ConsoleSpecPath
)

if ($Project -ne "all") {
  $playwrightArgs = @(
    "npm",
    "run",
    "e2e",
    "--prefix",
    $UiRoot,
    "--",
    "--project=$Project",
    "--output",
    $ResultsRoot,
    $ConsoleSpecPath
  )
}

$previousHtmlOutput = $env:PLAYWRIGHT_HTML_OUTPUT_DIR
try {
  $env:PLAYWRIGHT_HTML_OUTPUT_DIR = $ArtifactsRoot
  Invoke-LoggedCommand -LogPath $logPath -Command $playwrightArgs
} finally {
  $env:PLAYWRIGHT_HTML_OUTPUT_DIR = $previousHtmlOutput
}

$payload = [pscustomobject]@{
  status = "pass"
  project = $Project
  logPath = $logPath
  resultsRoot = $ResultsRoot
  artifactsRoot = $ArtifactsRoot
  spec = $ConsoleSpecPath
}

if ($AsJson) {
  $payload | ConvertTo-Json -Depth 4
} else {
  Write-E2ELog "status=pass project=$Project log=$logPath"
  Write-E2ELog "results=$ResultsRoot"
  Write-E2ELog "artifacts=$ArtifactsRoot"
}
