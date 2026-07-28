[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [Alias("PackageDir")]
    [string]$ReleaseDir,

    [string]$OutputDir = "",

    [Alias("Tag")]
    [string]$VersionId = "",

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

    $temporaryPath = $Path + ".write-" + [guid]::NewGuid().ToString("N")
    $stream = $null
    try {
        $bytes = $Encoding.GetBytes($Value)
        $stream = [System.IO.File]::Open(
            $temporaryPath,
            [System.IO.FileMode]::CreateNew,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::None
        )
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
        $stream.Dispose()
        $stream = $null
        [System.IO.File]::Move($temporaryPath, $Path)
    }
    finally {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
        if (Test-Path -LiteralPath $temporaryPath) {
            Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
        }
    }
}

function Read-PublicationLockStreamText {
    param([Parameter(Mandatory = $true)][System.IO.FileStream]$Stream)

    $originalPosition = $Stream.Position
    try {
        $Stream.Position = 0
        $bytes = New-Object byte[] ([int]$Stream.Length)
        $offset = 0
        while ($offset -lt $bytes.Length) {
            $read = $Stream.Read($bytes, $offset, $bytes.Length - $offset)
            if ($read -le 0) {
                throw "Publication lock owner record ended unexpectedly."
            }
            $offset += $read
        }
        return [System.Text.UTF8Encoding]::new($false).GetString($bytes)
    }
    finally {
        $Stream.Position = $originalPosition
    }
}

function Read-PublicationLockOwner {
    param([Parameter(Mandatory = $true)][string]$LockPath)

    $reader = $null
    try {
        $reader = [System.IO.File]::Open(
            $LockPath,
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::Read,
            ([System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
        )
        return Read-PublicationLockStreamText -Stream $reader
    }
    catch {
        return "<owner unavailable while publication lock is active>"
    }
    finally {
        if ($null -ne $reader) {
            $reader.Dispose()
        }
    }
}

function Get-PublicationMutexName {
    param([Parameter(Mandatory = $true)][string]$LockPath)

    $normalizedPath = [System.IO.Path]::GetFullPath($LockPath).ToLowerInvariant()
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = $sha256.ComputeHash(
            [System.Text.UTF8Encoding]::new($false).GetBytes($normalizedPath)
        )
    }
    finally {
        $sha256.Dispose()
    }
    $hex = [System.BitConverter]::ToString($digest).Replace("-", "").ToLowerInvariant()
    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        return "Global\GatewayReleasePublication-$hex"
    }
    return "GatewayReleasePublication-$hex"
}

function Enter-PublicationLock {
    param(
        [Parameter(Mandatory = $true)][string]$LockPath,
        [Parameter(Mandatory = $true)][string]$ArtifactName
    )

    $timeoutSeconds = 600
    $configuredTimeout = [Environment]::GetEnvironmentVariable(
        "GATEWAY_RELEASE_PUBLICATION_LOCK_TIMEOUT_SECONDS"
    )
    if (-not [string]::IsNullOrWhiteSpace($configuredTimeout)) {
        $parsedTimeout = 0
        if (-not [int]::TryParse($configuredTimeout, [ref]$parsedTimeout) -or $parsedTimeout -le 0) {
            throw "GATEWAY_RELEASE_PUBLICATION_LOCK_TIMEOUT_SECONDS must be a positive integer."
        }
        $timeoutSeconds = $parsedTimeout
    }

    $mutexName = Get-PublicationMutexName -LockPath $LockPath
    $mutex = [System.Threading.Mutex]::new($false, $mutexName)
    $mutexHeld = $false
    $stream = $null
    try {
        try {
            $mutexHeld = $mutex.WaitOne([int]($timeoutSeconds * 1000))
        }
        catch [System.Threading.AbandonedMutexException] {
            $mutexHeld = $true
        }
        if (-not $mutexHeld) {
            $owner = Read-PublicationLockOwner -LockPath $LockPath
            throw "Timed out waiting for publication lock: $LockPath owner=$owner"
        }

        $ownerToken = [guid]::NewGuid().ToString("N")
        $startedUtc = [DateTime]::UtcNow.ToString("o")
        $ownerRecord = [ordered]@{
            schemaVersion = 1
            ownerToken = $ownerToken
            processId = $PID
            machineName = [Environment]::MachineName
            startedUtc = $startedUtc
            artifactName = $ArtifactName
            state = "active"
        } | ConvertTo-Json -Compress
        $ownerRecord += [Environment]::NewLine
        $releaseRecord = [ordered]@{
            schemaVersion = 1
            ownerToken = $ownerToken
            processId = $PID
            machineName = [Environment]::MachineName
            startedUtc = $startedUtc
            artifactName = $ArtifactName
            state = "releasing"
        } | ConvertTo-Json -Compress
        $releaseRecord += [Environment]::NewLine

        $stream = [System.IO.File]::Open(
            $LockPath,
            [System.IO.FileMode]::Create,
            [System.IO.FileAccess]::ReadWrite,
            [System.IO.FileShare]::Read
        )
        $ownerBytes = [System.Text.UTF8Encoding]::new($false).GetBytes($ownerRecord)
        $stream.Write($ownerBytes, 0, $ownerBytes.Length)
        $stream.Flush($true)

        return [pscustomobject]@{
            Path = $LockPath
            ArtifactName = $ArtifactName
            OwnerToken = $ownerToken
            OwnerRecord = $ownerRecord
            ReleaseRecord = $releaseRecord
            Stream = $stream
            Mutex = $mutex
        }
    }
    catch {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
        if ($mutexHeld) {
            $mutex.ReleaseMutex()
        }
        $mutex.Dispose()
        throw
    }
}

function Exit-PublicationLock {
    param([Parameter(Mandatory = $true)]$Lock)

    $stream = $Lock.Stream
    $streamDisposed = $false
    try {
        $currentOwner = Read-PublicationLockStreamText -Stream $stream
        if ($currentOwner -cne $Lock.OwnerRecord) {
            throw "Refusing to remove a publication lock whose owner record changed: $($Lock.Path)"
        }

        $releaseBytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Lock.ReleaseRecord)
        $stream.SetLength(0)
        $stream.Position = 0
        $stream.Write($releaseBytes, 0, $releaseBytes.Length)
        $stream.Flush($true)
        $stream.Dispose()
        $streamDisposed = $true

        if (-not (Test-Path -LiteralPath $Lock.Path -PathType Leaf)) {
            throw "Publication lock owner record disappeared before cleanup: $($Lock.Path)"
        }
        $persistedOwner = [System.IO.File]::ReadAllText(
            $Lock.Path,
            [System.Text.UTF8Encoding]::new($false)
        )
        if ($persistedOwner -cne $Lock.ReleaseRecord) {
            throw "Refusing to delete a replacement publication lock owner record: $($Lock.Path)"
        }
        [System.IO.File]::Delete($Lock.Path)
    }
    finally {
        if (-not $streamDisposed) {
            $stream.Dispose()
        }
        try {
            $Lock.Mutex.ReleaseMutex()
        }
        finally {
            $Lock.Mutex.Dispose()
        }
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
$lockPath = "$zipPath.publishing.lock"

if ($DryRun) {
    [ordered]@{
        releaseDir = $releaseFull
        outputDir = $outputFull
        versionId = $VersionId
        assetName = $assetName
        zipPath = $zipPath
        checksumPath = $checksumPath
        publicationLockPath = $lockPath
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
$publicationLock = Enter-PublicationLock -LockPath $lockPath -ArtifactName $assetName
try {
$stagingPrefix = ".gateway-compress-$VersionId-"
foreach ($staleStaging in @(Get-ChildItem -LiteralPath $outputFull -Directory -Filter ($stagingPrefix + "*") -ErrorAction Stop)) {
    if (($staleStaging.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Refusing to clean a reparse-point release staging directory: $($staleStaging.FullName)"
    }
    Remove-Item -LiteralPath $staleStaging.FullName -Recurse -Force -ErrorAction Stop
}
foreach ($privatePattern in @(
    ((Split-Path -Leaf $journalPath) + ".write-*"),
    ((Split-Path -Leaf $checksumPath) + ".recover-*")
)) {
    foreach ($privateFile in @(Get-ChildItem -LiteralPath $outputFull -File -Filter $privatePattern -ErrorAction Stop)) {
        Remove-Item -LiteralPath $privateFile.FullName -Force -ErrorAction Stop
    }
}
if (Complete-InterruptedCompressionPublication -ZipPath $zipPath -ChecksumPath $checksumPath -JournalPath $journalPath) {
    Write-Output "[gateway-compress] recovered interrupted ZIP/checksum publication"
    Write-Output "[gateway-compress] ZIP $zipPath"
    Write-Output "[gateway-compress] SHA256 $checksumPath"
    exit 0
}
if (Test-Path -LiteralPath $zipPath) {
    throw "Release ZIP already exists and is immutable. Choose a new version: $zipPath"
}
if (Test-Path -LiteralPath $checksumPath) {
    throw "Release checksum already exists and is immutable. Choose a new version: $checksumPath"
}

$stagingRoot = Join-Path $outputFull ($stagingPrefix + $publicationLock.OwnerToken)
$stagingZip = Join-Path $stagingRoot $assetName
$stagingChecksum = "$stagingZip.sha256"
$zipArchive = $null
$zipStream = $null

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
    foreach ($artifactPath in @($zipPath, $checksumPath, $journalPath)) {
        if (Test-Path -LiteralPath $artifactPath) {
            throw "Release publication path became occupied and is immutable: $artifactPath"
        }
    }
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
    [System.IO.File]::Move($stagingChecksum, $checksumPath)

    $publishedZipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $publishedChecksumRecord = Get-Content -LiteralPath $checksumPath -Raw -Encoding ASCII
    if ($publishedZipHash -cne $zipHash -or $publishedChecksumRecord -cne $checksumRecord) {
        throw "Release ZIP/checksum final publication verification failed."
    }
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction Stop
    }
    if (Test-Path -LiteralPath $stagingRoot) {
        throw "Release publication staging cleanup did not complete: $stagingRoot"
    }
    Remove-Item -LiteralPath $journalPath -Force

    Write-Output "[gateway-compress] ZIP $zipPath"
    Write-Output "[gateway-compress] SHA256 $checksumPath ($zipHash)"
    Write-Output "[gateway-compress] files=$($releaseFiles.Count)"
} finally {
    if ($null -ne $zipArchive) {
        $zipArchive.Dispose()
    }
    if ($null -ne $zipStream) {
        $zipStream.Dispose()
    }
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction Stop
    }
}
}
finally {
    Exit-PublicationLock -Lock $publicationLock
}
