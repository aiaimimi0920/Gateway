[CmdletBinding()]
param(
    [string]$ImageName = "ghcr.io/aiaimimi0920/gateway",
    [string]$ImageTag = "local-verify",
    [string]$SourceImage = "",
    [switch]$BuildImage,
    [ValidateSet("named", "local")]
    [string]$Mode = "named",
    [string]$BindHost = "127.0.0.1",
    [int]$GatewayPort = 4252,
    [int]$StartupTimeoutSeconds = 300
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

function Wait-GatewayEndpoint {
    param(
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][datetime]$Deadline
    )

    do {
        try {
            return Invoke-RestMethod -Uri $Uri -TimeoutSec 5
        } catch {
            Start-Sleep -Seconds 5
        }
    } while ((Get-Date) -lt $Deadline)

    throw "Timed out waiting for $Uri"
}

$gatewayRoot = Resolve-FullPath -Path (Join-Path $PSScriptRoot "..") -RequireExisting
$deployDir = Resolve-FullPath -Path (Join-Path $gatewayRoot "deploy") -RequireExisting
$composeFileName = if ($Mode -eq "local") { "docker-compose.local.yml" } else { "docker-compose.yml" }
$composeFile = Resolve-FullPath -Path (Join-Path $deployDir $composeFileName) -RequireExisting
$projectName = "gatewayverify-" + [guid]::NewGuid().ToString("N").Substring(0, 12)
$composeEnvFile = Join-Path $deployDir ".env"
$composeEnvBackupPath = Join-Path ([System.IO.Path]::GetTempPath()) ("gateway-docker-verify-backup-{0}.env" -f [guid]::NewGuid().ToString("N"))
$createdComposeEnvFile = $false
$targetImage = ("{0}:{1}" -f $ImageName, $ImageTag)

try {
    if (Test-Path -LiteralPath $composeEnvFile -PathType Leaf) {
        Copy-Item -LiteralPath $composeEnvFile -Destination $composeEnvBackupPath -Force
    } else {
        $createdComposeEnvFile = $true
    }

    if ($BuildImage) {
        & docker build -t $targetImage -f (Join-Path $gatewayRoot "Dockerfile") $gatewayRoot
        if ($LASTEXITCODE -ne 0) {
            throw "docker build failed with exit code $LASTEXITCODE."
        }
    } elseif (-not [string]::IsNullOrWhiteSpace($SourceImage) -and
        -not [string]::Equals($SourceImage, $targetImage, [System.StringComparison]::OrdinalIgnoreCase)) {
        & docker tag $SourceImage $targetImage
        if ($LASTEXITCODE -ne 0) {
            throw "docker tag failed with exit code $LASTEXITCODE."
        }
    }

    $envLines = @(
        "COMPOSE_PROJECT_NAME=$projectName",
        "IMAGE_TAG=$ImageTag",
        "GATEWAY_BIND_HOST=$BindHost",
        "GATEWAY_PORT=$GatewayPort",
        "RUST_LOG=info",
        "GATEWAY_RUNTIME_ROLE=standalone",
        "GATEWAY_REDIS_URL=redis://redis:6379/0",
        "GATEWAY_ROUTES_FILE=/data/routes.yaml",
        "GATEWAY_STATE_DIR=/data/state",
        "GATEWAY_CONSOLE_REMOTE_ACCESS=false",
        "GATEWAY_ENV_FILE=.env"
    )
    Write-Utf8NoBomLines -Path $composeEnvFile -Lines $envLines

    if ($Mode -eq "local") {
        New-Item -ItemType Directory -Path (Join-Path $deployDir "gateway_data") -Force | Out-Null
        New-Item -ItemType Directory -Path (Join-Path $deployDir "redis_data") -Force | Out-Null
    }

    Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $composeEnvFile -Arguments @("up", "-d")
    Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $composeEnvFile -Arguments @("ps")

    $deadline = (Get-Date).AddSeconds($StartupTimeoutSeconds)
    $health = Wait-GatewayEndpoint -Uri ("http://{0}:{1}/healthz" -f $BindHost, $GatewayPort) -Deadline $deadline
    $ready = Wait-GatewayEndpoint -Uri ("http://{0}:{1}/readyz" -f $BindHost, $GatewayPort) -Deadline $deadline

    [pscustomobject]@{
        image = $targetImage
        mode = $Mode
        port = $GatewayPort
        health = $health
        ready = $ready
    } | ConvertTo-Json -Depth 8
}
catch {
    Write-Warning $_
    try {
        Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $composeEnvFile -Arguments @("logs", "--no-color", "--tail", "200")
    }
    catch {
        Write-Warning $_
    }
    throw
}
finally {
    if (Test-Path -LiteralPath $composeEnvFile -PathType Leaf) {
        try {
            Invoke-DockerCompose -ComposeFile $composeFile -EnvFilePath $composeEnvFile -Arguments @("down", "-v")
        }
        catch {
            Write-Warning $_
        }
    }

    if (Test-Path -LiteralPath $composeEnvBackupPath -PathType Leaf) {
        Move-Item -LiteralPath $composeEnvBackupPath -Destination $composeEnvFile -Force
    } elseif ($createdComposeEnvFile -and (Test-Path -LiteralPath $composeEnvFile -PathType Leaf)) {
        Remove-Item -LiteralPath $composeEnvFile -Force
    }
}
