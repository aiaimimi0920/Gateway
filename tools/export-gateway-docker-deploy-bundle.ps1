[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$VersionId,
    [string]$OutputDir = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "release-publication/lock.ps1")

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

function Write-Utf8NoBomNew {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $temporaryPath = $Path + ".write-" + [guid]::NewGuid().ToString("N")
    $stream = $null
    try {
        $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Value)
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

function Remove-DockerDeployStagingDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)

    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while (Test-Path -LiteralPath $Path) {
        try {
            Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
        }
        catch {
            if ([DateTime]::UtcNow -ge $deadline) {
                throw
            }
            Start-Sleep -Milliseconds 50
        }
    }
}

function Complete-InterruptedPublication {
    param(
        [Parameter(Mandatory = $true)][string]$ZipPath,
        [Parameter(Mandatory = $true)][string]$HashPath,
        [Parameter(Mandatory = $true)][string]$JournalPath
    )

    if (-not (Test-Path -LiteralPath $JournalPath)) {
        return $false
    }
    if (-not (Test-Path -LiteralPath $JournalPath -PathType Leaf)) {
        throw "Docker deploy bundle publication journal is not a file: $JournalPath"
    }

    $journalText = Get-Content -LiteralPath $JournalPath -Raw -Encoding UTF8
    try {
        $journal = $journalText | ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        throw "Docker deploy bundle publication journal is invalid: $JournalPath"
    }
    foreach ($property in @("schemaVersion", "zipName", "zipSha256", "hashRecord")) {
        if ($journal.PSObject.Properties.Name -notcontains $property) {
            throw "Docker deploy bundle publication journal is missing '$property': $JournalPath"
        }
    }
    $zipName = Split-Path -Leaf $ZipPath
    $zipHash = [string]$journal.zipSha256
    $expectedHashRecord = "{0} *{1}`n" -f $zipHash, $zipName
    if (
        [int]$journal.schemaVersion -ne 1 -or
        [string]$journal.zipName -cne $zipName -or
        $zipHash -notmatch "^[0-9a-f]{64}$" -or
        [string]$journal.hashRecord -cne $expectedHashRecord
    ) {
        throw "Docker deploy bundle publication journal does not match the requested artifact: $JournalPath"
    }

    if (-not (Test-Path -LiteralPath $ZipPath)) {
        if (Test-Path -LiteralPath $HashPath) {
            throw "Interrupted Docker deploy publication has a checksum but no ZIP: $HashPath"
        }
        Remove-Item -LiteralPath $JournalPath -Force
        return $false
    }
    if (-not (Test-Path -LiteralPath $ZipPath -PathType Leaf)) {
        throw "Interrupted Docker deploy publication ZIP is not a file: $ZipPath"
    }
    $actualZipHash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualZipHash -cne $zipHash) {
        throw "Interrupted Docker deploy publication ZIP does not match its journal: $ZipPath"
    }

    if (Test-Path -LiteralPath $HashPath) {
        if (-not (Test-Path -LiteralPath $HashPath -PathType Leaf)) {
            throw "Interrupted Docker deploy publication checksum is not a file: $HashPath"
        }
        $actualHashRecord = Get-Content -LiteralPath $HashPath -Raw -Encoding UTF8
        if ($actualHashRecord -cne $expectedHashRecord) {
            throw "Interrupted Docker deploy publication checksum does not match its journal: $HashPath"
        }
    }
    else {
        $recoveryHash = $HashPath + ".recover-" + [guid]::NewGuid().ToString("N")
        try {
            Write-Utf8NoBomNew -Path $recoveryHash -Value $expectedHashRecord
            [System.IO.File]::Move($recoveryHash, $HashPath)
        }
        finally {
            if (Test-Path -LiteralPath $recoveryHash) {
                Remove-Item -LiteralPath $recoveryHash -Force
            }
        }
    }

    Remove-Item -LiteralPath $JournalPath -Force
    return $true
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
Assert-VersionId -Value $VersionId
$deployPayloadRelativePaths = @(
    ".env.example",
    "README.md",
    "docker-compose.local.yml",
    "docker-compose.yml",
    "docker-deploy.sh",
    "docker-entrypoint.sh"
)
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $gatewayRoot "release\Gateway\packages"
}
$outputDirFull = Resolve-FullPath -Path $OutputDir
New-Item -ItemType Directory -Path $outputDirFull -Force | Out-Null

$bundleRootName = "Gateway-$VersionId-docker-deploy"
$zipPath = Join-Path $outputDirFull ($bundleRootName + ".zip")
$hashPath = $zipPath + ".sha256"
$journalPath = $zipPath + ".publishing.json"
$lockPath = $zipPath + ".publishing.lock"
$publicationLock = Enter-PublicationLock -LockPath $lockPath -ArtifactName ($bundleRootName + ".zip")
try {
$stagingParent = [System.IO.Path]::GetTempPath()
$artifactKey = $publicationLock.MutexName.Substring($publicationLock.MutexName.Length - 16)
$stagingPrefix = "gateway-docker-deploy-$VersionId-$artifactKey-"
foreach ($staleStaging in @(Get-ChildItem -LiteralPath $stagingParent -Directory -Filter ($stagingPrefix + "*") -ErrorAction Stop)) {
    if (($staleStaging.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Refusing to clean a reparse-point Docker deploy staging directory: $($staleStaging.FullName)"
    }
    Remove-DockerDeployStagingDirectory -Path $staleStaging.FullName
}
foreach ($privatePattern in @(
    ((Split-Path -Leaf $journalPath) + ".write-*"),
    ((Split-Path -Leaf $hashPath) + ".recover-*")
)) {
    foreach ($privateFile in @(Get-ChildItem -LiteralPath $outputDirFull -File -Filter $privatePattern -ErrorAction Stop)) {
        Remove-Item -LiteralPath $privateFile.FullName -Force -ErrorAction Stop
    }
}
if (Complete-InterruptedPublication -ZipPath $zipPath -HashPath $hashPath -JournalPath $journalPath) {
    [pscustomobject]@{
        zip = $zipPath
        sha256 = $hashPath
        recovered = $true
    } | ConvertTo-Json -Depth 4
    exit 0
}
foreach ($artifactPath in @($zipPath, $hashPath)) {
    if (Test-Path -LiteralPath $artifactPath) {
        throw "Docker deploy bundle artifact already exists and is immutable: $artifactPath"
    }
}
$stagingRoot = Join-Path $stagingParent ($stagingPrefix + $publicationLock.OwnerToken)
$bundleRoot = Join-Path $stagingRoot $bundleRootName
$stagingZip = Join-Path $stagingRoot ($bundleRootName + ".zip")
$stagingHash = $stagingZip + ".sha256"

try {
    New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null

    foreach ($deployRelativePath in $deployPayloadRelativePaths) {
        Copy-RepositoryPayload `
            -Source (Join-Path (Join-Path $gatewayRoot "deploy") $deployRelativePath) `
            -Destination (Join-Path (Join-Path $bundleRoot "deploy") $deployRelativePath)
    }
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "tools\deploy-gateway-docker.ps1") -Destination (Join-Path $bundleRoot "tools\deploy-gateway-docker.ps1")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "README.md") -Destination (Join-Path $bundleRoot "README.md")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "README.zh-CN.md") -Destination (Join-Path $bundleRoot "README.zh-CN.md")
    Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "LICENSE") -Destination (Join-Path $bundleRoot "LICENSE")

    $bundleFiles = @(Get-ChildItem -LiteralPath $bundleRoot -Recurse -File -Force |
        ForEach-Object {
            $relativePath = Get-RelativeUnixPath -BasePath $bundleRoot -Path $_.FullName
            [pscustomobject]@{
                Item = $_
                EntryName = "$bundleRootName/$relativePath"
            }
        } |
        Sort-Object { $_.EntryName.ToLowerInvariant() })
    $zipArchive = $null
    $zipStream = $null
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
        foreach ($bundleFile in $bundleFiles) {
            $entry = $zipArchive.CreateEntry(
                $bundleFile.EntryName,
                [System.IO.Compression.CompressionLevel]::Optimal
            )
            $entry.LastWriteTime = $fixedTimestamp
            $input = $null
            $entryStream = $null
            try {
                $input = [System.IO.File]::OpenRead($bundleFile.Item.FullName)
                $entryStream = $entry.Open()
                $input.CopyTo($entryStream, 1048576)
            }
            finally {
                if ($null -ne $entryStream) {
                    $entryStream.Dispose()
                }
                if ($null -ne $input) {
                    $input.Dispose()
                }
            }
        }
    }
    finally {
        if ($null -ne $zipArchive) {
            $zipArchive.Dispose()
        }
        if ($null -ne $zipStream) {
            $zipStream.Dispose()
        }
    }

    $zipHash = (Get-FileHash -LiteralPath $stagingZip -Algorithm SHA256).Hash.ToLowerInvariant()
    $zipName = Split-Path -Leaf $zipPath
    $expectedHashRecord = "{0} *{1}`n" -f $zipHash, $zipName
    Write-Utf8NoBom -Path $stagingHash -Value $expectedHashRecord

    $verifiedZipHash = (Get-FileHash -LiteralPath $stagingZip -Algorithm SHA256).Hash.ToLowerInvariant()
    $verifiedHashRecord = Get-Content -LiteralPath $stagingHash -Raw -Encoding UTF8
    if ($verifiedZipHash -ne $zipHash -or $verifiedHashRecord -ne $expectedHashRecord) {
        throw "Docker deploy bundle staging verification failed."
    }

    foreach ($artifactPath in @($zipPath, $hashPath, $journalPath)) {
        if (Test-Path -LiteralPath $artifactPath) {
            throw "Docker deploy bundle publication path became occupied and is immutable: $artifactPath"
        }
    }
    $journalRecord = @{
        schemaVersion = 1
        zipName = $zipName
        zipSha256 = $zipHash
        hashRecord = $expectedHashRecord
    } | ConvertTo-Json -Compress
    $journalRecord += "`n"
    Write-Utf8NoBomNew -Path $journalPath -Value $journalRecord

    [System.IO.File]::Move($stagingZip, $zipPath)
    [System.IO.File]::Move($stagingHash, $hashPath)

    $publishedZipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $publishedHashRecord = Get-Content -LiteralPath $hashPath -Raw -Encoding UTF8
    if ($publishedZipHash -cne $zipHash -or $publishedHashRecord -cne $expectedHashRecord) {
        throw "Docker deploy bundle final publication verification failed."
    }
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-DockerDeployStagingDirectory -Path $stagingRoot
    }
    if (Test-Path -LiteralPath $stagingRoot) {
        throw "Docker deploy publication staging cleanup did not complete: $stagingRoot"
    }
    Remove-Item -LiteralPath $journalPath -Force

    [pscustomobject]@{
        zip = $zipPath
        sha256 = $hashPath
    } | ConvertTo-Json -Depth 4
}
finally {
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-DockerDeployStagingDirectory -Path $stagingRoot
    }
}
}
finally {
    Exit-PublicationLock -Lock $publicationLock
}
