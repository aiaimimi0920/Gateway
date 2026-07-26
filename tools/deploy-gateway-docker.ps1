[CmdletBinding()]
param(
    [ValidateSet("up", "down", "ps", "logs", "restart")]
    [string]$Action = "up",
    [ValidateSet("named", "local")]
    [string]$Mode = "named",
    [string]$EnvFile = "",
    [string]$ComposeProjectName = "gateway",
    [string]$ImageTag = "latest",
    [string]$BindHost = "0.0.0.0",
    [int]$GatewayPort = 4200,
    [string]$RuntimeRole = "standalone",
    [switch]$Follow,
    [int]$Tail = 200,
    [switch]$RemoveVolumes
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

function Write-Utf8NoBomLines {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][AllowEmptyCollection()][string[]]$Lines
    )

    $parent = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    [System.IO.File]::WriteAllLines($Path, $Lines, [System.Text.UTF8Encoding]::new($false))
}

function Set-DotEnvValue {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $existing = if (Test-Path -LiteralPath $Path -PathType Leaf) {
        @(Get-Content -LiteralPath $Path -Encoding UTF8)
    } else {
        @()
    }
    $updated = $false
    $escapedKey = [Regex]::Escape($Key)
    for ($index = 0; $index -lt $existing.Count; $index++) {
        if ($existing[$index] -match ("^{0}=" -f $escapedKey)) {
            $existing[$index] = "$Key=$Value"
            $updated = $true
            break
        }
    }
    if (-not $updated) {
        $existing += "$Key=$Value"
    }
    Write-Utf8NoBomLines -Path $Path -Lines $existing
}

function Invoke-DockerCompose {
    param(
        [Parameter(Mandatory = $true)][string]$ComposeFile,
        [Parameter(Mandatory = $true)][string]$EnvFilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $composeArguments = @("compose", "--env-file", $EnvFilePath, "-f", $ComposeFile) + $Arguments
    & docker @composeArguments
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose failed with exit code $LASTEXITCODE."
    }
}

$gatewayRoot = Resolve-FullPath -Path (Join-Path $PSScriptRoot "..") -RequireExisting
$deployDir = Resolve-FullPath -Path (Join-Path $gatewayRoot "deploy") -RequireExisting
$composeFileName = if ($Mode -eq "local") { "docker-compose.local.yml" } else { "docker-compose.yml" }
$composeFile = Resolve-FullPath -Path (Join-Path $deployDir $composeFileName) -RequireExisting
$envTemplate = Resolve-FullPath -Path (Join-Path $deployDir ".env.example") -RequireExisting
$envPath = if ([string]::IsNullOrWhiteSpace($EnvFile)) {
    Resolve-FullPath -Path (Join-Path $deployDir ".env")
} else {
    Resolve-FullPath -Path $EnvFile
}

$createdEnv = $false
if (-not (Test-Path -LiteralPath $envPath -PathType Leaf)) {
    $templateLines = @(Get-Content -LiteralPath $envTemplate -Encoding UTF8)
    Write-Utf8NoBomLines -Path $envPath -Lines $templateLines
    $createdEnv = $true
}

if ($Action -eq "up") {
    if ($Mode -eq "local") {
        New-Item -ItemType Directory -Path (Join-Path $deployDir "gateway_data") -Force | Out-Null
        New-Item -ItemType Directory -Path (Join-Path $deployDir "redis_data") -Force | Out-Null
    }

    $updatableValues = [ordered]@{}
    if ($createdEnv -or $PSBoundParameters.ContainsKey("ComposeProjectName")) {
        $updatableValues["COMPOSE_PROJECT_NAME"] = $ComposeProjectName
    }
    if ($createdEnv -or $PSBoundParameters.ContainsKey("ImageTag")) {
        $updatableValues["IMAGE_TAG"] = $ImageTag
    }
    if ($createdEnv -or $PSBoundParameters.ContainsKey("BindHost")) {
        $updatableValues["GATEWAY_BIND_HOST"] = $BindHost
    }
    if ($createdEnv -or $PSBoundParameters.ContainsKey("GatewayPort")) {
        $updatableValues["GATEWAY_PORT"] = [string]$GatewayPort
    }
    if ($createdEnv -or $PSBoundParameters.ContainsKey("RuntimeRole")) {
        $updatableValues["GATEWAY_RUNTIME_ROLE"] = $RuntimeRole
    }
    $updatableValues["GATEWAY_ENV_FILE"] = $envPath

    foreach ($entry in $updatableValues.GetEnumerator()) {
        Set-DotEnvValue -Path $envPath -Key $entry.Key -Value $entry.Value
    }
}

switch ($Action) {
    "up" {
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments @("up", "-d")
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments @("ps")
    }
    "down" {
        $arguments = @("down")
        if ($RemoveVolumes) {
            $arguments += "-v"
        }
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments $arguments
    }
    "ps" {
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments @("ps")
    }
    "restart" {
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments @("restart")
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments @("ps")
    }
    "logs" {
        $arguments = @("logs", "--tail", [string]$Tail)
        if ($Follow) {
            $arguments += "-f"
        }
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $envPath -Arguments $arguments
    }
}
