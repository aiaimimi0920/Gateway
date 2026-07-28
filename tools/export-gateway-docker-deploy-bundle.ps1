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

    $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Value)
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
$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("gateway-docker-deploy-{0}" -f [guid]::NewGuid().ToString("N"))
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

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $bundleRoot,
        $stagingZip,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $true
    )

    $zipHash = (Get-FileHash -LiteralPath $stagingZip -Algorithm SHA256).Hash.ToLowerInvariant()
    $zipName = Split-Path -Leaf $zipPath
    $expectedHashRecord = "{0} *{1}`n" -f $zipHash, $zipName
    Write-Utf8NoBom -Path $stagingHash -Value $expectedHashRecord

    $verifiedZipHash = (Get-FileHash -LiteralPath $stagingZip -Algorithm SHA256).Hash.ToLowerInvariant()
    $verifiedHashRecord = Get-Content -LiteralPath $stagingHash -Raw -Encoding UTF8
    if ($verifiedZipHash -ne $zipHash -or $verifiedHashRecord -ne $expectedHashRecord) {
        throw "Docker deploy bundle staging verification failed."
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
    try {
        [System.IO.File]::Move($stagingHash, $hashPath)
    }
    catch {
        $hashPublishError = $_
        try {
            if (Test-Path -LiteralPath $zipPath -PathType Leaf) {
                $publishedZipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
                if ($publishedZipHash -ne $zipHash) {
                    throw "Refusing to roll back a ZIP that no longer matches this export: $zipPath"
                }
                Remove-Item -LiteralPath $zipPath -Force -ErrorAction Stop
            }
        }
        catch {
            throw "Docker deploy bundle checksum publication failed, and ZIP rollback also failed. Checksum error: $($hashPublishError.Exception.Message) Rollback error: $($_.Exception.Message)"
        }
        if (Test-Path -LiteralPath $journalPath -PathType Leaf) {
            $currentJournalRecord = Get-Content -LiteralPath $journalPath -Raw -Encoding UTF8
            if ($currentJournalRecord -ceq $journalRecord) {
                Remove-Item -LiteralPath $journalPath -Force
            }
        }
        throw "Docker deploy bundle checksum publication failed; the new ZIP was rolled back. $($hashPublishError.Exception.Message)"
    }

    $publishedZipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $publishedHashRecord = Get-Content -LiteralPath $hashPath -Raw -Encoding UTF8
    if ($publishedZipHash -cne $zipHash -or $publishedHashRecord -cne $expectedHashRecord) {
        throw "Docker deploy bundle final publication verification failed."
    }
    Remove-Item -LiteralPath $journalPath -Force

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
