[CmdletBinding()]
param(
    [string]$SourceRoot = "",
    [string]$VersionId = "",
    [string]$ReleaseRoot = "",
    [switch]$StageOnly,
    [switch]$SkipBuild,
    [switch]$AllowCustomReleaseRoot,
    [Alias("CleanStage", "CleanStaging")]
    [switch]$Clean
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

function Resolve-GatewayRoot {
    param([Parameter(Mandatory = $true)][string]$Path)

    $candidate = Resolve-FullPath -Path $Path -RequireExisting
    $item = Get-Item -LiteralPath $candidate
    if (-not $item.PSIsContainer) {
        throw "SourceRoot must be a directory: $candidate"
    }

    $nestedGateway = Join-Path $candidate "Gateway"
    if ((Test-Path -LiteralPath (Join-Path $candidate "Cargo.toml") -PathType Leaf) -or
        (Test-Path -LiteralPath (Join-Path $candidate "target") -PathType Container)) {
        return $candidate
    }
    if (Test-Path -LiteralPath (Join-Path $nestedGateway "Cargo.toml") -PathType Leaf) {
        return (Resolve-FullPath -Path $nestedGateway -RequireExisting)
    }

    # Fixture and dry-run callers may provide a reduced Gateway tree without
    # Cargo metadata; in that case the supplied directory is the source root.
    return $candidate
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

function Assert-PathUnderRoot {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $rootFull = (Resolve-FullPath -Path $Root).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = Resolve-FullPath -Path $Path
    if (-not $pathFull.StartsWith($rootFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to write outside the release root. Root=[$Root] Path=[$Path]"
    }
}

function Assert-CanonicalReleaseRoot {
    param(
        [Parameter(Mandatory = $true)][string]$GatewayRoot,
        [Parameter(Mandatory = $true)][string]$ReleaseRoot,
        [switch]$AllowCustom
    )

    $canonical = Resolve-FullPath -Path (Join-Path $GatewayRoot "release\Gateway")
    $requested = Resolve-FullPath -Path $ReleaseRoot
    if (-not [string]::Equals($requested, $canonical, [System.StringComparison]::OrdinalIgnoreCase) -and
        -not $AllowCustom) {
        throw "ReleaseRoot must be $canonical. Use -AllowCustomReleaseRoot only for isolated test/development staging."
    }
    return $requested
}

function Remove-NewlyPublishedRelease {
    param(
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][string]$ReleaseRoot
    )

    Assert-PathUnderRoot -Root $ReleaseRoot -Path $Destination
    if (-not (Test-Path -LiteralPath $Destination -PathType Container)) {
        return
    }

    Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction Stop
    if (Test-Path -LiteralPath $Destination) {
        throw "Newly published release still exists after rollback: $Destination"
    }
}

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseRoot = (Resolve-FullPath -Path $BasePath).TrimEnd("\", "/")
    $pathFull = Resolve-FullPath -Path $Path
    if ([string]::Equals($baseRoot, $pathFull.TrimEnd("\", "/"), [System.StringComparison]::OrdinalIgnoreCase)) {
        return "."
    }
    $baseFull = $baseRoot + [System.IO.Path]::DirectorySeparatorChar
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [System.IO.File]::WriteAllText($Path, $Value, [System.Text.UTF8Encoding]::new($false))
}

function Write-Ascii {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [System.IO.File]::WriteAllText($Path, $Value, [System.Text.ASCIIEncoding]::new())
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
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Evidence source file is missing: $Source"
    }
    if (Test-Path -LiteralPath $Destination) {
        throw "Evidence file already exists and is immutable: $Destination"
    }

    $destinationFull = Resolve-FullPath -Path $Destination
    $destinationParent = Split-Path -Parent $destinationFull
    $destinationParentExisted = Test-Path -LiteralPath $destinationParent -PathType Container
    New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
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
                -ExcludedRootDirectories @() `
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

function Resolve-PowerShellExecutable {
    $pwsh = Get-Command "pwsh" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $pwsh) {
        return $pwsh.Source
    }

    $powershell = Get-Command "powershell" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $powershell) {
        return $powershell.Source
    }

    throw "Neither pwsh nor powershell is available to invoke the Gateway build script."
}

function Get-GitValue {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$GitArguments
    )

    if ($null -eq (Get-Command "git" -ErrorAction SilentlyContinue)) {
        return $null
    }
    $cursor = Get-Item -LiteralPath $Root
    while ($null -ne $cursor -and -not (Test-Path -LiteralPath (Join-Path $cursor.FullName ".git"))) {
        $cursor = $cursor.Parent
    }
    if ($null -eq $cursor) {
        return $null
    }
    $output = @(& git -C $cursor.FullName @GitArguments 2>$null)
    if ($LASTEXITCODE -ne 0) {
        return $null
    }
    $value = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    $value = $value.Trim()
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $null
    }
    return $value
}

function Get-DeterministicBuildTimestamp {
    param([Parameter(Mandatory = $true)][string]$Root)

    if (-not [string]::IsNullOrWhiteSpace($env:SOURCE_DATE_EPOCH)) {
        $epoch = 0L
        if ([int64]::TryParse($env:SOURCE_DATE_EPOCH, [ref]$epoch)) {
            return [DateTimeOffset]::FromUnixTimeSeconds($epoch).UtcDateTime.ToString("yyyy-MM-ddTHH:mm:ssZ")
        }
    }
    $commitTimestamp = Get-GitValue -Root $Root -GitArguments @("show", "-s", "--format=%cI", "HEAD")
    if (-not [string]::IsNullOrWhiteSpace($commitTimestamp)) {
        return $commitTimestamp
    }
    return "1970-01-01T00:00:00Z"
}

function Get-GitRepositoryRoot {
    param([Parameter(Mandatory = $true)][string]$Root)

    $cursor = Get-Item -LiteralPath $Root
    while ($null -ne $cursor) {
        if (Test-Path -LiteralPath (Join-Path $cursor.FullName ".git")) {
            return $cursor.FullName
        }
        $cursor = $cursor.Parent
    }
    return $null
}

function Get-SourceTreeState {
    param([Parameter(Mandatory = $true)][string]$Root)

    $excludedDirectoryNames = @(".git", "target", "node_modules", ".runtime", "output")
    $excludedRootDirectoryNames = @("release")
    $gitRoot = Get-GitRepositoryRoot -Root $Root
    $gitCommand = Get-Command "git" -ErrorAction SilentlyContinue
    $algorithm = "sha256-file-list-v1"
    $files = @()
    if ($null -ne $gitRoot) {
        if ($null -eq $gitCommand) {
            throw "Unable to enumerate Gateway source files with Git because git is unavailable."
        }
        $pathSpec = Get-RelativeUnixPath -BasePath $gitRoot -Path $Root
        $gitPaths = @(& git -C $gitRoot -c core.quotepath=false ls-files --cached --others --exclude-standard -- $pathSpec 2>$null)
        if ($LASTEXITCODE -ne 0) {
            throw "Unable to enumerate Gateway source files with Git."
        }
        $files = @(
            foreach ($gitPath in $gitPaths) {
                if ([string]::IsNullOrWhiteSpace($gitPath)) {
                    continue
                }
                $fullPath = Join-Path $gitRoot $gitPath
                if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
                    Get-Item -LiteralPath $fullPath
                }
            }
        )
        $algorithm = "sha256-git-source-list-v2"
    } else {
        $files = @(Get-ChildItem -LiteralPath $Root -Recurse -File -Force)
    }
    $records = [System.Collections.Generic.List[string]]::new()
    foreach ($file in $files) {
        $relative = Get-RelativeUnixPath -BasePath $Root -Path $file.FullName
        $skip = $false
        $parts = @($relative -split "/")
        for ($index = 0; $index -lt $parts.Count; $index++) {
            $part = $parts[$index]
            $lowerPart = $part.ToLowerInvariant()
            if (($excludedDirectoryNames -contains $lowerPart) -or
                (($index -eq 0) -and ($excludedRootDirectoryNames -contains $lowerPart)) -or
                $part.StartsWith("tmp-", [System.StringComparison]::OrdinalIgnoreCase)) {
                $skip = $true
                break
            }
        }
        if ($skip) {
            continue
        }

        $digest = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $records.Add("$relative`t$($file.Length)`t$digest`n") | Out-Null
    }

    $sortedRecords = $records.ToArray()
    [Array]::Sort($sortedRecords, [System.StringComparer]::Ordinal)
    $fingerprintPayload = [string]::Concat($sortedRecords)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $fingerprintBytes = $sha256.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($fingerprintPayload))
    } finally {
        $sha256.Dispose()
    }
    $fingerprint = ([System.BitConverter]::ToString($fingerprintBytes)).Replace("-", "").ToLowerInvariant()

    $dirty = $null
    if ($null -ne $gitRoot -and $null -ne $gitCommand) {
        $pathSpec = Get-RelativeUnixPath -BasePath $gitRoot -Path $Root
        $statusOutput = @(& git -C $gitRoot status --porcelain=v1 --untracked-files=all -- $pathSpec 2>$null)
        if ($LASTEXITCODE -eq 0) {
            $dirty = $statusOutput.Count -gt 0
        }
    }

    return [pscustomobject]@{
        algorithm = $algorithm
        fingerprint = $fingerprint
        fileCount = $sortedRecords.Count
        dirty = $dirty
    }
}

function Assert-BuildProvenance {
    param(
        [Parameter(Mandatory = $true)][string]$GatewayRoot,
        [Parameter(Mandatory = $true)][object]$SourceTreeState,
        [Parameter(Mandatory = $true)][string[]]$ArtifactRelativePaths
    )

    $provenancePath = Join-Path $GatewayRoot "target\release\gateway-build-provenance.json"
    if (-not (Test-Path -LiteralPath $provenancePath -PathType Leaf)) {
        throw "Build provenance is missing: $provenancePath. Run tools\build-gateway-release.ps1 before using -SkipBuild or -StageOnly."
    }

    try {
        $provenance = Get-Content -LiteralPath $provenancePath -Raw -Encoding UTF8 | ConvertFrom-Json
    } catch {
        throw "Build provenance is not valid JSON: $provenancePath"
    }
    if ([int]$provenance.schemaVersion -ne 1) {
        throw "Unsupported Gateway build provenance schema: $($provenance.schemaVersion)"
    }
    if ($null -eq $provenance.sourceTree) {
        throw "Gateway build provenance is missing source tree metadata. Rebuild before packaging."
    }
    if ([string]$provenance.sourceTree.algorithm -ne [string]$SourceTreeState.algorithm) {
        throw "Gateway build provenance source tree algorithm does not match the current source enumeration. Rebuild before packaging."
    }
    if ([string]$provenance.sourceTreeFingerprint -ne [string]$SourceTreeState.fingerprint) {
        throw "Gateway source tree changed after the release binaries were built. Rebuild before packaging."
    }
    if ([string]$provenance.sourceTree.fingerprint -ne [string]$SourceTreeState.fingerprint) {
        throw "Gateway build provenance source tree fingerprint is inconsistent. Rebuild before packaging."
    }
    if ([int]$provenance.sourceTree.fileCount -ne [int]$SourceTreeState.fileCount) {
        throw "Gateway build provenance source tree file count is inconsistent. Rebuild before packaging."
    }

    $provenanceArtifacts = @($provenance.artifacts)
    foreach ($relativePath in $ArtifactRelativePaths) {
        $matchingRecords = @($provenanceArtifacts | Where-Object { [string]$_.path -eq $relativePath })
        if ($matchingRecords.Count -ne 1) {
            throw "Build provenance must contain exactly one artifact record for $relativePath"
        }

        $artifactPath = Join-Path $GatewayRoot $relativePath.Replace("/", [System.IO.Path]::DirectorySeparatorChar)
        if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
            throw "Required Gateway build artifact is missing: $artifactPath"
        }
        $artifact = Get-Item -LiteralPath $artifactPath
        $actualHash = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ([int64]$matchingRecords[0].bytes -ne [int64]$artifact.Length -or
            [string]$matchingRecords[0].sha256 -ne $actualHash) {
            throw "Gateway build artifact no longer matches build provenance: $relativePath"
        }
    }

    return $provenance
}

if ([string]::IsNullOrWhiteSpace($SourceRoot)) {
    $SourceRoot = Join-Path $PSScriptRoot ".."
}
if ([string]::IsNullOrWhiteSpace($VersionId)) {
    $VersionId = Get-Date -Format "yyyyMMdd-HHmmss"
}

Assert-VersionId -Value $VersionId
$gatewayRoot = Resolve-GatewayRoot -Path $SourceRoot
if ([string]::IsNullOrWhiteSpace($ReleaseRoot)) {
    $ReleaseRoot = Join-Path $gatewayRoot "release\Gateway"
}
$releaseRootFull = Assert-CanonicalReleaseRoot `
    -GatewayRoot $gatewayRoot `
    -ReleaseRoot $ReleaseRoot `
    -AllowCustom:$AllowCustomReleaseRoot
$destination = Resolve-FullPath -Path (Join-Path $releaseRootFull $VersionId)
Assert-PathUnderRoot -Root $releaseRootFull -Path $destination
$evidenceVersionRoot = Resolve-FullPath -Path (Join-Path $gatewayRoot "target\release-evidence\$VersionId")
$evidenceProvenancePath = Resolve-FullPath -Path (Join-Path $evidenceVersionRoot "gateway-build-provenance.json")

if (Test-Path -LiteralPath $destination) {
    throw "Release destination already exists and is immutable: $destination. Choose a new VersionId."
}
if (Test-Path -LiteralPath $evidenceVersionRoot -PathType Leaf) {
    throw "Release evidence destination must be a directory: $evidenceVersionRoot"
}
if (Test-Path -LiteralPath $evidenceProvenancePath) {
    throw "Evidence build provenance already exists and is immutable: $evidenceProvenancePath. Choose a new VersionId."
}

$buildSkipped = $StageOnly -or $SkipBuild
if (-not $buildSkipped) {
    $buildScript = Join-Path $gatewayRoot "tools\build-gateway-release.ps1"
    if (-not (Test-Path -LiteralPath $buildScript -PathType Leaf)) {
        throw "Gateway build script is missing: $buildScript. Use -StageOnly or -SkipBuild for existing artifacts."
    }

    $powershellExecutable = Resolve-PowerShellExecutable
    Write-Output "[gateway-package] building Gateway binaries"
    & $powershellExecutable -NoProfile -ExecutionPolicy Bypass -File $buildScript
    if ($LASTEXITCODE -ne 0) {
        throw "Gateway build failed with exit code $LASTEXITCODE"
    }
}

$headlessSource = Join-Path $gatewayRoot "target\release\neuro-gateway.exe"
$uiSource = Join-Path $gatewayRoot "apps\desktop\src-tauri\target\release\neuro-gateway-ui.exe"
foreach ($requiredBinary in @($headlessSource, $uiSource)) {
    if (-not (Test-Path -LiteralPath $requiredBinary -PathType Leaf)) {
        throw "Required Gateway build artifact is missing: $requiredBinary"
    }
}

$routesSource = Join-Path $gatewayRoot "routes.yaml"
if (-not (Test-Path -LiteralPath $routesSource -PathType Leaf)) {
    throw "Required Gateway route configuration is missing: $routesSource"
}
$environmentTemplateSource = Join-Path $gatewayRoot ".env.example"
if (-not (Test-Path -LiteralPath $environmentTemplateSource -PathType Leaf)) {
    throw "Required Gateway environment template is missing: $environmentTemplateSource"
}
$buildProvenanceSource = Join-Path $gatewayRoot "target\release\gateway-build-provenance.json"

$manifestsSource = Join-Path $gatewayRoot "manifests"
$scriptsSource = Join-Path $gatewayRoot "scripts"
$docsSource = Join-Path $gatewayRoot "docs"
$toolsSource = Join-Path $gatewayRoot "tools"
foreach ($requiredDirectory in @($manifestsSource, $scriptsSource, $docsSource, $toolsSource)) {
    if (-not (Test-Path -LiteralPath $requiredDirectory -PathType Container)) {
        throw "Required Gateway support directory is missing: $requiredDirectory"
    }
}

$artifactRelativePaths = @(
    "target/release/neuro-gateway.exe",
    "apps/desktop/src-tauri/target/release/neuro-gateway-ui.exe"
)
$sourceTreeState = Get-SourceTreeState -Root $gatewayRoot
Assert-BuildProvenance `
    -GatewayRoot $gatewayRoot `
    -SourceTreeState $sourceTreeState `
    -ArtifactRelativePaths $artifactRelativePaths | Out-Null

$routesExampleSource = Join-Path $gatewayRoot "routes.example.yaml"
$routeSources = [System.Collections.Generic.List[string]]::new()
$routeSources.Add($routesSource) | Out-Null
if (Test-Path -LiteralPath $routesExampleSource -PathType Leaf) {
    $routeSources.Add($routesExampleSource) | Out-Null
}

# Repeat the immutable destination checks after build/provenance validation to
# close the window where another process could claim the version id.
if (Test-Path -LiteralPath $destination) {
    throw "Release destination already exists and is immutable: $destination. Choose a new VersionId."
}
if (Test-Path -LiteralPath $evidenceProvenancePath) {
    throw "Evidence build provenance already exists and is immutable: $evidenceProvenancePath. Choose a new VersionId."
}
New-Item -ItemType Directory -Path $releaseRootFull -Force | Out-Null
Assert-PathUnderRoot -Root $releaseRootFull -Path $destination
$staging = Join-Path $releaseRootFull (".{0}.staging-{1}" -f $VersionId, [guid]::NewGuid().ToString("N"))
Assert-PathUnderRoot -Root $releaseRootFull -Path $staging
$published = $false

try {
    New-Item -ItemType Directory -Path $staging -Force | Out-Null
    Assert-PathUnderRoot -Root $releaseRootFull -Path $staging

    $exeRecords = @()
    $supportRecords = @()

    $headlessDestination = Copy-PayloadFile `
        -Source $headlessSource `
        -PackageRoot $staging `
        -DestinationRelativePath "neuro-gateway.exe"
    $exeRecords += New-ArtifactRecord -PackageRoot $staging -Path $headlessDestination -Kind "exe"

    $uiDestination = Copy-PayloadFile `
        -Source $uiSource `
        -PackageRoot $staging `
        -DestinationRelativePath "neuro-gateway-ui.exe"
    $exeRecords += New-ArtifactRecord -PackageRoot $staging -Path $uiDestination -Kind "exe"

    $environmentTemplateDestination = Copy-PayloadFile `
        -Source $environmentTemplateSource `
        -PackageRoot $staging `
        -DestinationRelativePath ".env.example"
    $supportRecords += New-ArtifactRecord `
        -PackageRoot $staging `
        -Path $environmentTemplateDestination `
        -Kind "environment-template"

    $buildProvenanceDestination = Copy-PayloadFile `
        -Source $buildProvenanceSource `
        -PackageRoot $staging `
        -DestinationRelativePath "gateway-build-provenance.json"
    $supportRecords += New-ArtifactRecord `
        -PackageRoot $staging `
        -Path $buildProvenanceDestination `
        -Kind "build-provenance"

    foreach ($routeSource in $routeSources) {
        $routeFile = Get-Item -LiteralPath $routeSource
        $routeDestination = Copy-PayloadFile `
            -Source $routeFile.FullName `
            -PackageRoot $staging `
            -DestinationRelativePath $routeFile.Name
        $supportRecords += New-ArtifactRecord -PackageRoot $staging -Path $routeDestination -Kind "route-config"
    }

    Copy-FilteredTree -Source $manifestsSource -Destination (Join-Path $staging "manifests")
    $manifestFiles = @(Get-ChildItem -LiteralPath (Join-Path $staging "manifests") -Recurse -File | Sort-Object FullName)
    foreach ($manifestFile in $manifestFiles) {
        $supportRecords += New-ArtifactRecord -PackageRoot $staging -Path $manifestFile.FullName -Kind "gateway-manifest"
    }

    Copy-FilteredTree -Source $scriptsSource -Destination (Join-Path $staging "scripts")
    $scriptFiles = @(Get-ChildItem -LiteralPath (Join-Path $staging "scripts") -Recurse -File | Sort-Object FullName)
    foreach ($scriptFile in $scriptFiles) {
        $supportRecords += New-ArtifactRecord -PackageRoot $staging -Path $scriptFile.FullName -Kind "script"
    }

    Copy-FilteredTree `
        -Source $docsSource `
        -Destination (Join-Path $staging "docs") `
        -ExcludeEvidenceJson `
        -ExcludedRootDirectories @("analysis", "plan", "progress", "superpowers")
    $documentationFiles = @(Get-ChildItem -LiteralPath (Join-Path $staging "docs") -Recurse -File | Sort-Object FullName)
    foreach ($documentationFile in $documentationFiles) {
        $supportRecords += New-ArtifactRecord -PackageRoot $staging -Path $documentationFile.FullName -Kind "documentation"
    }

    Copy-FilteredTree `
        -Source $toolsSource `
        -Destination (Join-Path $staging "tools") `
        -ExcludedFileNames @("run-gateway-line-evidence.ps1")
    $toolFiles = @(Get-ChildItem -LiteralPath (Join-Path $staging "tools") -Recurse -File | Sort-Object FullName)
    foreach ($toolFile in $toolFiles) {
        $supportRecords += New-ArtifactRecord -PackageRoot $staging -Path $toolFile.FullName -Kind "tool"
    }

    $payloadRecords = @($exeRecords + $supportRecords)
    $routePaths = @($supportRecords | Where-Object { $_.kind -eq "route-config" } | ForEach-Object { $_.path })
    $sourceRevision = Get-GitValue -Root $gatewayRoot -GitArguments @("rev-parse", "HEAD")
    if ([string]::IsNullOrWhiteSpace($sourceRevision)) {
        $sourceRevision = "unknown"
    }
    $manifest = [ordered]@{
        schemaVersion = 1
        app = "Gateway"
        sourceProject = "Gateway"
        versionId = $VersionId
        sourceRevision = $sourceRevision
        sourceTreeFingerprint = $sourceTreeState.fingerprint
        sourceTreeDirty = $sourceTreeState.dirty
        sourceTree = [ordered]@{
            algorithm = $sourceTreeState.algorithm
            fingerprint = $sourceTreeState.fingerprint
            fileCount = $sourceTreeState.fileCount
            dirty = $sourceTreeState.dirty
        }
        buildTimestamp = Get-DeterministicBuildTimestamp -Root $gatewayRoot
        packagerVersion = "gateway-package/v3"
        target = "windows-x64"
        layout = [ordered]@{
            executables = @("neuro-gateway.exe", "neuro-gateway-ui.exe")
            environmentTemplate = ".env.example"
            buildProvenance = "gateway-build-provenance.json"
            routes = @($routePaths)
            manifests = "manifests/"
            scripts = "scripts/"
            docs = "docs/"
            tools = "tools/"
        }
        exes = @($exeRecords)
        supportFiles = @($supportRecords)
        files = @($payloadRecords)
        checksums = "checksums.sha256"
    }

    $manifestPath = Join-Path $staging "manifest.json"
    Write-Utf8NoBom -Path $manifestPath -Value (($manifest | ConvertTo-Json -Depth 12) + [Environment]::NewLine)

    $checksumPath = Join-Path $staging "checksums.sha256"
    $checksumFiles = @(Get-ChildItem -LiteralPath $staging -Recurse -File |
        Where-Object { $_.FullName -ne (Resolve-FullPath -Path $checksumPath) } |
        Sort-Object FullName)
    $checksumLines = @()
    foreach ($checksumFile in $checksumFiles) {
        $relative = Get-RelativeUnixPath -BasePath $staging -Path $checksumFile.FullName
        $hash = (Get-FileHash -LiteralPath $checksumFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $checksumLines += "$hash  $relative"
    }
    Write-Ascii -Path $checksumPath -Value (($checksumLines -join "`r`n") + "`r`n")

    $publishSourceTreeState = Get-SourceTreeState -Root $gatewayRoot
    if ([string]$publishSourceTreeState.fingerprint -ne [string]$sourceTreeState.fingerprint) {
        throw "Gateway source tree changed during packaging. Rebuild before publishing a new VersionId."
    }
    Assert-BuildProvenance `
        -GatewayRoot $gatewayRoot `
        -SourceTreeState $publishSourceTreeState `
        -ArtifactRelativePaths $artifactRelativePaths | Out-Null

    # Directory.Move is a same-volume atomic publish. It fails when another
    # process won the destination race, preserving the prior release.
    [System.IO.Directory]::Move($staging, $destination)
    $published = $true
    try {
        $evidenceProvenanceSnapshot = Copy-ImmutableEvidenceFile `
            -Source (Join-Path $destination "gateway-build-provenance.json") `
            -Destination $evidenceProvenancePath
    } catch {
        $evidencePublicationError = $_
        try {
            Remove-NewlyPublishedRelease -Destination $destination `
                -ReleaseRoot $releaseRootFull
            $published = $false
        } catch {
            throw "Evidence provenance publication failed, and release rollback also failed. Evidence error: $($evidencePublicationError.Exception.Message) Rollback error: $($_.Exception.Message)"
        }
        throw "Evidence provenance publication failed; the new release was rolled back. $($evidencePublicationError.Exception.Message)"
    }
    Write-Output "[gateway-package] published $destination"
    Write-Output "[gateway-package] files=$($checksumFiles.Count) checksums=$(Join-Path $destination 'checksums.sha256')"
    Write-Output "[gateway-package] evidence provenance $evidenceProvenanceSnapshot"
} finally {
    if (-not $published -and (Test-Path -LiteralPath $staging)) {
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
    }
}
