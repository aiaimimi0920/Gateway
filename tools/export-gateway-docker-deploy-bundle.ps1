[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$VersionId,
    [string]$OutputDir = ""
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

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $parent = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    [System.IO.File]::WriteAllText($Path, $Value, [System.Text.UTF8Encoding]::new($false))
}

function Copy-RepositoryPayload {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    $destinationParent = Split-Path -Parent $Destination
    if (-not [string]::IsNullOrWhiteSpace($destinationParent)) {
        New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
    }
    Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force
}

$gatewayRoot = Resolve-FullPath -Path (Join-Path $PSScriptRoot "..") -RequireExisting
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $gatewayRoot "release\Gateway\packages"
}
$outputDirFull = Resolve-FullPath -Path $OutputDir
New-Item -ItemType Directory -Path $outputDirFull -Force | Out-Null

$bundleRootName = "Gateway-$VersionId-docker-deploy"
$zipPath = Join-Path $outputDirFull ($bundleRootName + ".zip")
$hashPath = $zipPath + ".sha256"
$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("gateway-docker-deploy-{0}" -f [guid]::NewGuid().ToString("N"))
$bundleRoot = Join-Path $stagingRoot $bundleRootName

try {
    New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null

    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "deploy") -Destination (Join-Path $bundleRoot "deploy")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "tools\deploy-gateway-docker.ps1") -Destination (Join-Path $bundleRoot "tools\deploy-gateway-docker.ps1")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "README.md") -Destination (Join-Path $bundleRoot "README.md")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "README.zh-CN.md") -Destination (Join-Path $bundleRoot "README.zh-CN.md")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "LICENSE") -Destination (Join-Path $bundleRoot "LICENSE")

    if (Test-Path -LiteralPath $zipPath -PathType Leaf) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $bundleRoot,
        $zipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $true
    )

    $zipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $zipName = Split-Path -Leaf $zipPath
    Write-Utf8NoBom -Path $hashPath -Value ("{0} *{1}`n" -f $zipHash, $zipName)

    [pscustomobject]@{
        zip = $zipPath
        sha256 = $hashPath
    } | ConvertTo-Json -Depth 4
}
finally {
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force
    }
}
