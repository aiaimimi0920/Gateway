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

function Write-EncodedFileNew {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value,
        [Parameter(Mandatory = $true)][System.Text.Encoding]$Encoding
    )

    $bytes = $Encoding.GetBytes($Value)
    $stream = [System.IO.File]::Open(
        $Path,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None
    )
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
}

function Complete-InterruptedCompressionPublication {
    param(
        [Parameter(Mandatory = $true)][string]$ZipPath,
        [Parameter(Mandatory = $true)][string]$ChecksumPath,
        [Parameter(Mandatory = $true)][string]$JournalPath
    )

    if (-not (Test-Path -LiteralPath $JournalPath)) {
        return $false
    }
    if (-not (Test-Path -LiteralPath $JournalPath -PathType Leaf)) {
        throw "Release publication journal is not a file: $JournalPath"
    }

    $journalText = Get-Content -LiteralPath $JournalPath -Raw -Encoding UTF8
    try {
        $journal = $journalText | ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        throw "Release publication journal is invalid: $JournalPath"
    }
    foreach ($property in @("schemaVersion", "zipName", "zipSha256", "checksumRecord")) {
        if ($journal.PSObject.Properties.Name -notcontains $property) {
            throw "Release publication journal is missing '$property': $JournalPath"
        }
    }
    $zipName = Split-Path -Leaf $ZipPath
    $zipHash = [string]$journal.zipSha256
    $expectedChecksumRecord = "{0}  {1}{2}" -f $zipHash, $zipName, [Environment]::NewLine
    if (
        [int]$journal.schemaVersion -ne 1 -or
        [string]$journal.zipName -cne $zipName -or
        $zipHash -notmatch "^[0-9a-f]{64}$" -or
        [string]$journal.checksumRecord -cne $expectedChecksumRecord
    ) {
        throw "Release publication journal does not match the requested artifact: $JournalPath"
    }

    if (-not (Test-Path -LiteralPath $ZipPath)) {
        if (Test-Path -LiteralPath $ChecksumPath) {
            throw "Interrupted release publication has a checksum but no ZIP: $ChecksumPath"
        }
        Remove-Item -LiteralPath $JournalPath -Force
        return $false
    }
    if (-not (Test-Path -LiteralPath $ZipPath -PathType Leaf)) {
        throw "Interrupted release publication ZIP is not a file: $ZipPath"
    }
    $actualZipHash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualZipHash -cne $zipHash) {
        throw "Interrupted release publication ZIP does not match its journal: $ZipPath"
    }

    if (Test-Path -LiteralPath $ChecksumPath) {
        if (-not (Test-Path -LiteralPath $ChecksumPath -PathType Leaf)) {
            throw "Interrupted release publication checksum is not a file: $ChecksumPath"
        }
        $actualChecksumRecord = Get-Content -LiteralPath $ChecksumPath -Raw -Encoding ASCII
        if ($actualChecksumRecord -cne $expectedChecksumRecord) {
            throw "Interrupted release publication checksum does not match its journal: $ChecksumPath"
        }
    }
    else {
        $recoveryChecksum = $ChecksumPath + ".recover-" + [guid]::NewGuid().ToString("N")
        try {
            Write-EncodedFileNew `
                -Path $recoveryChecksum `
                -Value $expectedChecksumRecord `
                -Encoding ([System.Text.ASCIIEncoding]::new())
            [System.IO.File]::Move($recoveryChecksum, $ChecksumPath)
        }
        finally {
            if (Test-Path -LiteralPath $recoveryChecksum) {
                Remove-Item -LiteralPath $recoveryChecksum -Force
            }
        }
    }

    Remove-Item -LiteralPath $JournalPath -Force
    return $true
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
$journalPath = "$zipPath.publishing.json"

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
if (Complete-InterruptedCompressionPublication -ZipPath $zipPath -ChecksumPath $checksumPath -JournalPath $journalPath) {
    Write-Output "[gateway-compress] recovered interrupted ZIP/checksum publication"
    Write-Output "[gateway-compress] ZIP $zipPath"
    Write-Output "[gateway-compress] SHA256 $checksumPath"
    exit 0
}
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
$journalRecord = $null

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
    $checksumRecord = "{0}  {1}{2}" -f $zipHash, $assetName, [Environment]::NewLine
    Write-AsciiNoBom -Path $stagingChecksum -Value $checksumRecord
    $journalRecord = @{
        schemaVersion = 1
        zipName = $assetName
        zipSha256 = $zipHash
        checksumRecord = $checksumRecord
    } | ConvertTo-Json -Compress
    $journalRecord += "`n"
    Write-EncodedFileNew `
        -Path $journalPath `
        -Value $journalRecord `
        -Encoding ([System.Text.UTF8Encoding]::new($false))

    [System.IO.File]::Move($stagingZip, $zipPath)
    $publishedZip = $true
    [System.IO.File]::Move($stagingChecksum, $checksumPath)
    $publishedChecksum = $true

    $publishedZipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $publishedChecksumRecord = Get-Content -LiteralPath $checksumPath -Raw -Encoding ASCII
    if ($publishedZipHash -cne $zipHash -or $publishedChecksumRecord -cne $checksumRecord) {
        throw "Release ZIP/checksum final publication verification failed."
    }
    Remove-Item -LiteralPath $journalPath -Force

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
    if ($null -ne $journalRecord -and (Test-Path -LiteralPath $journalPath -PathType Leaf)) {
        $currentJournalRecord = Get-Content -LiteralPath $journalPath -Raw -Encoding UTF8
        if ($currentJournalRecord -ceq $journalRecord) {
            Remove-Item -LiteralPath $journalPath -Force -ErrorAction SilentlyContinue
        }
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
