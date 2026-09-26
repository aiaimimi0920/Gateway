# Package manifest and checksum validation for the packaged runtime smoke.

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseFull = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Assert-SafePackageRelativePath {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path) -or
        $Path.Contains("\") -or
        $Path.StartsWith("/") -or
        $Path.Contains(":") -or
        (($Path -split "/") -contains "..")) {
        throw "Package manifest contains an unsafe relative path: $Path"
    }
    return $Path
}

function Read-ChecksumIndex {
    param([Parameter(Mandatory = $true)][string]$Path)

    $entries = [System.Collections.Generic.Dictionary[string, string]]::new(
        [System.StringComparer]::Ordinal
    )
    $lineNumber = 0
    foreach ($line in (Get-Content -LiteralPath $Path -Encoding ASCII)) {
        $lineNumber++
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        # The checksum contract is exactly: 64 hex chars, two spaces, path.
        if ($line -notmatch "^(?<digest>[0-9a-fA-F]{64})  (?<path>[^\r\n]+)$") {
            throw "Invalid checksums.sha256 line $lineNumber (expected digest + two spaces + relative path)"
        }
        $digest = $Matches["digest"].ToLowerInvariant()
        $relative = Assert-SafePackageRelativePath -Path $Matches["path"]
        if ($entries.ContainsKey($relative)) {
            throw "Duplicate checksums.sha256 path: $relative"
        }
        $entries.Add($relative, $digest)
    }
    if ($entries.Count -eq 0) {
        throw "checksums.sha256 contains no entries"
    }
    return $entries
}

function Get-ObjectPropertyValue {
    param(
        [Parameter(Mandatory = $true)][object]$Object,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Assert-PackagedReleaseIntegrity {
    param([Parameter(Mandatory = $true)][string]$ReleaseRoot)

    $manifestPath = Join-Path $ReleaseRoot "manifest.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "Missing manifest.json in packaged release: $ReleaseRoot"
    }
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $checksumsRelative = [string](Get-ObjectPropertyValue -Object $manifest -Name "checksums")
    if ($checksumsRelative -ne "checksums.sha256") {
        throw "Packaged manifest must point to checksums.sha256 exactly"
    }
    $checksumsPath = Join-Path $ReleaseRoot (Assert-SafePackageRelativePath -Path $checksumsRelative)
    if (-not (Test-Path -LiteralPath $checksumsPath -PathType Leaf)) {
        throw "Missing checksum file in packaged release: $checksumsPath"
    }
    $checksumEntries = Read-ChecksumIndex -Path $checksumsPath

    $actualFiles = [System.Collections.Generic.Dictionary[string, string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($file in @(Get-ChildItem -LiteralPath $ReleaseRoot -Recurse -File -Force)) {
        $relative = Get-RelativeUnixPath -BasePath $ReleaseRoot -Path $file.FullName
        if ($relative -eq "checksums.sha256") {
            continue
        }
        Assert-SafePackageRelativePath -Path $relative | Out-Null
        $actualFiles[$relative] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    if ($actualFiles.Count -ne $checksumEntries.Count) {
        throw "checksums.sha256 entry count does not match packaged files"
    }
    foreach ($relative in $checksumEntries.Keys) {
        if (-not $actualFiles.ContainsKey($relative)) {
            throw "checksums.sha256 references a missing packaged file: $relative"
        }
        if ($actualFiles[$relative] -ne $checksumEntries[$relative]) {
            throw "Packaged checksum mismatch: $relative"
        }
    }

    $manifestFiles = @(Get-ObjectPropertyValue -Object $manifest -Name "files")
    foreach ($record in $manifestFiles) {
        $relative = Assert-SafePackageRelativePath -Path ([string](Get-ObjectPropertyValue -Object $record -Name "path"))
        if (-not $actualFiles.ContainsKey($relative)) {
            throw "Manifest references a missing packaged file: $relative"
        }
        $payloadPath = Join-Path $ReleaseRoot $relative
        $item = Get-Item -LiteralPath $payloadPath
        $recordBytes = [int64](Get-ObjectPropertyValue -Object $record -Name "bytes")
        $recordSha = [string](Get-ObjectPropertyValue -Object $record -Name "sha256")
        if ($item.Length -ne $recordBytes -or $actualFiles[$relative] -ne $recordSha.ToLowerInvariant()) {
            throw "Manifest artifact record does not match payload: $relative"
        }
    if ($checksumEntries[$relative] -ne $actualFiles[$relative]) {
            throw "Manifest artifact is not represented by the exact checksum entry: $relative"
        }
    }

    if (-not $checksumEntries.ContainsKey("manifest.json")) {
        throw "checksums.sha256 must contain an exact manifest.json entry"
    }
    $manifestListedPaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($record in $manifestFiles) {
        $manifestListedPaths.Add([string](Get-ObjectPropertyValue -Object $record -Name "path")) | Out-Null
    }
    foreach ($relative in $checksumEntries.Keys) {
        if ($relative -ne "manifest.json" -and -not $manifestListedPaths.Contains($relative)) {
            throw "checksums.sha256 contains a payload not listed in manifest.files: $relative"
        }
    }

    $exeRecords = @(Get-ObjectPropertyValue -Object $manifest -Name "exes")
    $headlessRecords = @($exeRecords | Where-Object {
        ([string](Get-ObjectPropertyValue -Object $_ -Name "name") -eq "gateway.exe") -and
        ([string](Get-ObjectPropertyValue -Object $_ -Name "path") -eq "gateway.exe")
    })
    if ($headlessRecords.Count -ne 1) {
        throw "manifest.exes must contain exactly one gateway.exe record at the package root"
    }

    return [pscustomobject]@{
        manifest = $manifest
        checksumEntries = $checksumEntries
        manifestSha256 = $actualFiles["manifest.json"]
        checksumsSha256 = (Get-FileHash -LiteralPath $checksumsPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}
