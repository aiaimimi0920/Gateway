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
. (Join-Path $PSScriptRoot "package-release/artifact-copy.ps1")
. (Join-Path $PSScriptRoot "package-release/source-provenance.ps1")

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
Assert-ArtifactPathWithoutLinks -Root $gatewayRoot -Path $evidenceProvenancePath

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

$headlessSource = Join-Path $gatewayRoot "target\release\gateway.exe"
$uiSource = Join-Path $gatewayRoot "apps\desktop\src-tauri\target\release\gateway-ui.exe"
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
$deploySource = Join-Path $gatewayRoot "deploy"
$toolsSource = Join-Path $gatewayRoot "tools"
$deployPayloadRelativePaths = @(
    ".env.example",
    "README.md",
    "docker-compose.local.yml",
    "docker-compose.yml",
    "docker-deploy.sh",
    "docker-entrypoint.sh",
    # The compose files provision their own PostgreSQL and seed it from these
    # files on first boot; without the schema the console's live concurrency,
    # cost and success-rate panels answer 503 "PostgreSQL 尚未配置", and without
    # the bootstrap rows every relay request fails with
    # 404 "AI gateway project 不存在".
    "postgres\initdb\001-gateway-schema.sql",
    "postgres\initdb\002-gateway-bootstrap.sql",
    "postgres\initdb\003-gateway-standalone-constraints.sql"
)
foreach ($requiredDirectory in @($manifestsSource, $scriptsSource, $docsSource, $deploySource, $toolsSource)) {
    if (-not (Test-Path -LiteralPath $requiredDirectory -PathType Container)) {
        throw "Required Gateway support directory is missing: $requiredDirectory"
    }
}

$artifactRelativePaths = @(
    "target/release/gateway.exe",
    "apps/desktop/src-tauri/target/release/gateway-ui.exe"
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
        -DestinationRelativePath "gateway.exe"
    $exeRecords += New-ArtifactRecord -PackageRoot $staging -Path $headlessDestination -Kind "exe"

    $uiDestination = Copy-PayloadFile `
        -Source $uiSource `
        -PackageRoot $staging `
        -DestinationRelativePath "gateway-ui.exe"
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

    foreach ($deployRelativePath in $deployPayloadRelativePaths) {
        $deployDestination = Copy-PayloadFile `
            -Source (Join-Path $deploySource $deployRelativePath) `
            -PackageRoot $staging `
            -DestinationRelativePath (Join-Path "deploy" $deployRelativePath)
        $supportRecords += New-ArtifactRecord `
            -PackageRoot $staging `
            -Path $deployDestination `
            -Kind "docker-deploy"
    }

    Copy-FilteredTree `
        -Source $toolsSource `
        -Destination (Join-Path $staging "tools") `
        -ExcludedRootDirectories @("line-evidence") `
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
            executables = @("gateway.exe", "gateway-ui.exe")
            environmentTemplate = ".env.example"
            buildProvenance = "gateway-build-provenance.json"
            routes = @($routePaths)
            manifests = "manifests/"
            scripts = "scripts/"
            docs = "docs/"
            deploy = "deploy/"
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
            -Destination $evidenceProvenancePath `
            -TrustedRoot $gatewayRoot
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
