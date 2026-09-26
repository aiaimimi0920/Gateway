function Assert-ArtifactPathWithoutLinks {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path
    )
    $rootFull = Resolve-FullPath -Path $Root
    $cursor = Resolve-FullPath -Path $Path
    Assert-PathUnderRoot -Root $rootFull -Path $cursor
    # The configured root is the authority; reject linked descendants only.
    while (-not $cursor.Equals($rootFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        try {
            $attributes = [System.IO.File]::GetAttributes($cursor)
            if (($attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Artifact destination contains a linked descendant: $cursor"
            }
        } catch [System.IO.FileNotFoundException] {
        } catch [System.IO.DirectoryNotFoundException] {
        }
        $cursor = Split-Path -Parent $cursor
    }
}

function New-ArtifactRecord {
    param(
        [Parameter(Mandatory = $true)][string]$PackageRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Kind
    )

    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    if ($item.PSIsContainer) {
        throw "Artifact record path is not a file: $Path"
    }

    return [ordered]@{
        kind = $Kind
        name = $item.Name
        path = Get-RelativeUnixPath -BasePath $PackageRoot -Path $Path
        bytes = [int64]$item.Length
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function Copy-PayloadFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$PackageRoot,
        [Parameter(Mandatory = $true)][string]$DestinationRelativePath
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Required Gateway support file is missing: $Source"
    }

    $destination = Join-Path $PackageRoot $DestinationRelativePath
    Assert-PathUnderRoot -Root $PackageRoot -Path $destination
    $destinationParent = Split-Path -Parent $destination
    New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
    Copy-Item -LiteralPath $Source -Destination $destination -Force
    return $destination
}

function Copy-ImmutableEvidenceFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][string]$TrustedRoot
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Evidence source file is missing: $Source"
    }
    if (Test-Path -LiteralPath $Destination) {
        throw "Evidence file already exists and is immutable: $Destination"
    }

    $destinationFull = Resolve-FullPath -Path $Destination
    Assert-ArtifactPathWithoutLinks -Root $TrustedRoot -Path $destinationFull
    $destinationParent = Split-Path -Parent $destinationFull
    $destinationParentExisted = Test-Path -LiteralPath $destinationParent -PathType Container
    New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
    Assert-ArtifactPathWithoutLinks -Root $TrustedRoot -Path $destinationFull
    $temporaryPath = Join-Path $destinationParent (
        ".{0}.staging-{1}" -f (Split-Path -Leaf $destinationFull), [guid]::NewGuid().ToString("N")
    )

    try {
        [System.IO.File]::Copy((Resolve-FullPath -Path $Source), $temporaryPath, $false)
        $sourceItem = Get-Item -LiteralPath $Source
        $temporaryItem = Get-Item -LiteralPath $temporaryPath
        $sourceHash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
        $temporaryHash = (Get-FileHash -LiteralPath $temporaryPath -Algorithm SHA256).Hash
        if ($sourceItem.Length -ne $temporaryItem.Length -or $sourceHash -ne $temporaryHash) {
            throw "Immutable evidence copy verification failed: $Destination"
        }

        # File.Move does not replace an existing destination, so a concurrent
        # writer cannot overwrite evidence after the preflight check.
        [System.IO.File]::Move($temporaryPath, $destinationFull)
    } finally {
        if (Test-Path -LiteralPath $temporaryPath) {
            Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
        }
        if (-not $destinationParentExisted -and (Test-Path -LiteralPath $destinationParent -PathType Container)) {
            $remaining = @(Get-ChildItem -LiteralPath $destinationParent -Force -ErrorAction SilentlyContinue)
            if ($remaining.Count -eq 0) {
                Remove-Item -LiteralPath $destinationParent -Force -ErrorAction SilentlyContinue
            }
        }
    }

    return $destinationFull
}

function Copy-FilteredTree {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [switch]$ExcludeEvidenceJson,
        [string[]]$ExcludedRootDirectories = @(),
        [string[]]$ExcludedFileNames = @()
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
        throw "Required Gateway support directory is missing: $Source"
    }

    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    $excludedDirectories = @("node_modules", ".runtime", "output")
    $excludedFileNamesLower = @($ExcludedFileNames | ForEach-Object { $_.ToLowerInvariant() })
    $items = @(Get-ChildItem -LiteralPath $Source -Force | Sort-Object Name)
    foreach ($item in $items) {
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            continue
        }

        if ($item.PSIsContainer) {
            $lowerName = $item.Name.ToLowerInvariant()
            if (($excludedDirectories -contains $lowerName) -or
                ($ExcludedRootDirectories -contains $lowerName) -or
                $item.Name -like "tmp-*") {
                continue
            }
            Copy-FilteredTree `
                -Source $item.FullName `
                -Destination (Join-Path $Destination $item.Name) `
                -ExcludeEvidenceJson:$ExcludeEvidenceJson `
                -ExcludedFileNames $ExcludedFileNames
            continue
        }

        if ($item.Name -like "*.pyc" -or
            $item.Name -eq ".DS_Store" -or
            $excludedFileNamesLower -contains $item.Name.ToLowerInvariant()) {
            continue
        }

        if ($ExcludeEvidenceJson -and
            $item.Extension -ieq ".json" -and
            (($item.FullName -split "[\\/]") -contains "evidence")) {
            continue
        }

        Copy-Item -LiteralPath $item.FullName -Destination (Join-Path $Destination $item.Name) -Force
    }
}
