[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [Alias("PackageDir")]
    [string]$ReleaseDir,

    [string]$OutputDir = "",

    [Alias("Tag")]
    [string]$VersionId = "",

    [switch]$Force,

    [switch]$DryRun
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

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseFull = (Resolve-FullPath -Path $BasePath).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = Resolve-FullPath -Path $Path
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Assert-VersionId {
    param([Parameter(Mandatory = $true)][string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "VersionId must not be empty."
    }
    if ($Value -notmatch "^[A-Za-z0-9][A-Za-z0-9._-]*$") {
        throw "VersionId contains unsupported path characters: $Value"
    }
}

function Test-PathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $rootWithSeparator = (Resolve-FullPath -Path $Root).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = Resolve-FullPath -Path $Path
    return $pathFull.StartsWith($rootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)
}

function Write-AsciiNoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    [System.IO.File]::WriteAllText($Path, $Value, [System.Text.ASCIIEncoding]::new())
}

$releaseFull = Resolve-FullPath -Path $ReleaseDir
if ([string]::IsNullOrWhiteSpace($VersionId)) {
    $VersionId = Split-Path -Leaf $releaseFull
}
Assert-VersionId -Value $VersionId

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path (Split-Path -Parent $releaseFull) "packages"
}
$outputFull = Resolve-FullPath -Path $OutputDir
$assetName = "Gateway-$VersionId-windows-x64.zip"
$zipPath = Join-Path $outputFull $assetName
$checksumPath = "$zipPath.sha256"

if ($DryRun) {
    [ordered]@{
        releaseDir = $releaseFull
        outputDir = $outputFull
        versionId = $VersionId
        assetName = $assetName
        zipPath = $zipPath
        checksumPath = $checksumPath
        deterministicTimestamp = "1980-01-01T00:00:00Z"
    } | ConvertTo-Json -Depth 5
    exit 0
}

if (-not (Test-Path -LiteralPath $releaseFull -PathType Container)) {
    throw "Release directory does not exist: $releaseFull"
}
foreach ($requiredFile in @("manifest.json", "checksums.sha256", "gateway.exe", "gateway-ui.exe")) {
    $requiredPath = Join-Path $releaseFull $requiredFile
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Release directory is missing required file: $requiredPath"
    }
}
if ([string]::Equals($outputFull, $releaseFull, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputDir must not be the release directory itself."
}

New-Item -ItemType Directory -Path $outputFull -Force | Out-Null
if ((Test-Path -LiteralPath $zipPath -PathType Leaf) -and -not $Force) {
    throw "Release ZIP already exists. Choose a new version or pass -Force: $zipPath"
}
if ((Test-Path -LiteralPath $checksumPath -PathType Leaf) -and -not $Force) {
    throw "Release checksum already exists. Choose a new version or pass -Force: $checksumPath"
}
if (Test-Path -LiteralPath $zipPath -PathType Leaf) {
    Remove-Item -LiteralPath $zipPath -Force
}
if (Test-Path -LiteralPath $checksumPath -PathType Leaf) {
    Remove-Item -LiteralPath $checksumPath -Force
}

$stagingRoot = Join-Path $outputFull (".gateway-compress-{0}" -f [guid]::NewGuid().ToString("N"))
$stagingZip = Join-Path $stagingRoot $assetName
$stagingChecksum = "$stagingZip.sha256"
$zipArchive = $null
$zipStream = $null
$publishedZip = $false
$publishedChecksum = $false

try {
    New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null

    $releaseFiles = @(Get-ChildItem -LiteralPath $releaseFull -Recurse -File -Force |
        Where-Object {
            ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0 -and
            -not (Test-PathWithin -Root $outputFull -Path $_.FullName)
        } |
        ForEach-Object {
            [pscustomobject]@{
                Item = $_
                RelativePath = Get-RelativeUnixPath -BasePath $releaseFull -Path $_.FullName
            }
        } |
        Sort-Object RelativePath)

    if ($releaseFiles.Count -eq 0) {
        throw "Release directory contains no files to compress: $releaseFull"
    }

    try {
        Add-Type -AssemblyName System.IO.Compression
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zipStream = [System.IO.File]::Open(
            $stagingZip,
            [System.IO.FileMode]::CreateNew,
            [System.IO.FileAccess]::ReadWrite,
            [System.IO.FileShare]::None
        )
        $zipArchive = New-Object System.IO.Compression.ZipArchive(
            $zipStream,
            [System.IO.Compression.ZipArchiveMode]::Create,
            $false,
            [System.Text.Encoding]::UTF8
        )
        $fixedTimestamp = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
        foreach ($releaseFile in $releaseFiles) {
            $entry = $zipArchive.CreateEntry(
                $releaseFile.RelativePath,
                [System.IO.Compression.CompressionLevel]::Optimal
            )
            $entry.LastWriteTime = $fixedTimestamp
            $input = $null
            $entryStream = $null
            try {
                $input = [System.IO.File]::OpenRead($releaseFile.Item.FullName)
                $entryStream = $entry.Open()
                $input.CopyTo($entryStream, 1048576)
            } finally {
                if ($null -ne $entryStream) {
                    $entryStream.Dispose()
                }
                if ($null -ne $input) {
                    $input.Dispose()
                }
            }
        }
    } finally {
        if ($null -ne $zipArchive) {
            $zipArchive.Dispose()
            $zipArchive = $null
        }
        if ($null -ne $zipStream) {
            $zipStream.Dispose()
            $zipStream = $null
        }
    }

    $zipHash = (Get-FileHash -LiteralPath $stagingZip -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-AsciiNoBom -Path $stagingChecksum -Value ("{0}  {1}{2}" -f $zipHash, $assetName, [Environment]::NewLine)

    [System.IO.File]::Move($stagingZip, $zipPath)
    $publishedZip = $true
    [System.IO.File]::Move($stagingChecksum, $checksumPath)
    $publishedChecksum = $true

    Write-Output "[gateway-compress] ZIP $zipPath"
    Write-Output "[gateway-compress] SHA256 $checksumPath ($zipHash)"
    Write-Output "[gateway-compress] files=$($releaseFiles.Count)"
} catch {
    if ($publishedChecksum -and (Test-Path -LiteralPath $checksumPath)) {
        Remove-Item -LiteralPath $checksumPath -Force -ErrorAction SilentlyContinue
    }
    if ($publishedZip -and (Test-Path -LiteralPath $zipPath)) {
        Remove-Item -LiteralPath $zipPath -Force -ErrorAction SilentlyContinue
    }
    throw
} finally {
    if ($null -ne $zipArchive) {
        $zipArchive.Dispose()
    }
    if ($null -ne $zipStream) {
        $zipStream.Dispose()
    }
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
