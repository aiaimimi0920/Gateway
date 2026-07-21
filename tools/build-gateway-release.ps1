[CmdletBinding()]
param(
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$desktopRoot = Join-Path $repoRoot "apps\\desktop"
$maxNpmCiAttempts = 3

$commands = @(
    'cargo build --locked --release --bin neuro-gateway',
    'npm ci (retry up to 3 attempts on transient Windows file locks)',
    'npm run typecheck',
    'npm run tauri -- build --no-bundle'
)

if ($DryRun) {
    $commands | ForEach-Object { Write-Output $_ }
    exit 0
}

function Test-RetryableNpmCiFailure {
    param([string]$Output)

    return ($Output -match "EPERM|EBUSY|ENOTEMPTY")
}

function Invoke-GatewayReleaseStep {
    param(
        [string]$Name,
        [scriptblock]$Action
    )

    $startedAt = Get-Date
    Write-Output "[gateway-release] BEGIN $Name"
    try {
        & $Action
        $elapsed = (Get-Date) - $startedAt
        Write-Output "[gateway-release] END $Name ($([Math]::Round($elapsed.TotalSeconds, 2))s)"
    } catch {
        $elapsed = (Get-Date) - $startedAt
        Write-Output "[gateway-release] FAIL $Name ($([Math]::Round($elapsed.TotalSeconds, 2))s)"
        throw
    }
}

function Invoke-NpmCiWithRetry {
    param([int]$MaxAttempts = $maxNpmCiAttempts)

    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        Write-Output "[gateway-release] npm ci attempt $attempt/$MaxAttempts"

        $npmOutput = @(& npm ci 2>&1)
        $npmExitCode = $LASTEXITCODE
        $npmOutput | ForEach-Object { Write-Output $_ }

        if ($npmExitCode -eq 0) {
            return
        }

        $combinedOutput = ($npmOutput | ForEach-Object { $_.ToString() }) -join "`n"
        if (($attempt -lt $MaxAttempts) -and (Test-RetryableNpmCiFailure -Output $combinedOutput)) {
            $delaySeconds = [Math]::Min(10, 2 * $attempt)
            Write-Warning "npm ci failed with a transient Windows file-lock style error; retrying in $delaySeconds second(s)."
            Start-Sleep -Seconds $delaySeconds
            continue
        }

        throw "npm ci failed for Gateway desktop UI after $attempt attempt(s)"
    }
}

function Write-ArtifactHashSummary {
    param([string[]]$Paths)

    foreach ($path in $Paths) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing build artifact: $path"
        }

        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        Write-Output "[gateway-release] artifact $($item.Name) bytes=$($item.Length) sha256=$hash"
    }
}

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseRoot = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/")
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    if ([string]::Equals($baseRoot, $pathFull.TrimEnd("\", "/"), [System.StringComparison]::OrdinalIgnoreCase)) {
        return "."
    }
    $baseFull = $baseRoot + [System.IO.Path]::DirectorySeparatorChar
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Get-SourceTreeState {
    param([Parameter(Mandatory = $true)][string]$Root)

    $excludedDirectoryNames = @(".git", "target", "node_modules", ".runtime", "output")
    $excludedRootDirectoryNames = @("release")
    $gitRoot = $null
    $cursor = Get-Item -LiteralPath $Root
    while ($null -ne $cursor) {
        if (Test-Path -LiteralPath (Join-Path $cursor.FullName ".git")) {
            $gitRoot = $cursor.FullName
            break
        }
        $cursor = $cursor.Parent
    }
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
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $fingerprintBytes = $sha256.ComputeHash(
            [System.Text.Encoding]::UTF8.GetBytes([string]::Concat($sortedRecords))
        )
    } finally {
        $sha256.Dispose()
    }

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
        fingerprint = ([System.BitConverter]::ToString($fingerprintBytes)).Replace("-", "").ToLowerInvariant()
        fileCount = $sortedRecords.Count
        dirty = $dirty
    }
}

function Write-BuildProvenance {
    param(
        [Parameter(Mandatory = $true)][string]$GatewayRoot,
        [Parameter(Mandatory = $true)][object]$SourceTreeState,
        [Parameter(Mandatory = $true)][string[]]$ArtifactPaths
    )

    $records = @()
    foreach ($path in $ArtifactPaths) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing build artifact while writing provenance: $path"
        }
        $item = Get-Item -LiteralPath $path
        $records += [ordered]@{
            path = Get-RelativeUnixPath -BasePath $GatewayRoot -Path $path
            bytes = [int64]$item.Length
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }

    $payload = [ordered]@{
        schemaVersion = 1
        app = "Gateway"
        builtAt = [DateTimeOffset]::UtcNow.ToString("o")
        sourceTreeFingerprint = $SourceTreeState.fingerprint
        sourceTreeDirty = $SourceTreeState.dirty
        sourceTree = [ordered]@{
            algorithm = $SourceTreeState.algorithm
            fingerprint = $SourceTreeState.fingerprint
            fileCount = $SourceTreeState.fileCount
            dirty = $SourceTreeState.dirty
        }
        artifacts = @($records)
    }
    $provenancePath = Join-Path $GatewayRoot "target\release\gateway-build-provenance.json"
    $parent = Split-Path -Parent $provenancePath
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [System.IO.File]::WriteAllText(
        $provenancePath,
        (($payload | ConvertTo-Json -Depth 10) + [Environment]::NewLine),
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "[gateway-release] build provenance $provenancePath sourceTreeFingerprint=$($SourceTreeState.fingerprint)"
}

Push-Location -LiteralPath $repoRoot
try {
    Invoke-GatewayReleaseStep -Name "build headless neuro-gateway" -Action {
        & cargo build --locked --release --bin neuro-gateway
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed for neuro-gateway"
        }
    }

    Push-Location -LiteralPath $desktopRoot
    try {
        Invoke-GatewayReleaseStep -Name "install desktop dependencies" -Action {
            Invoke-NpmCiWithRetry -MaxAttempts $maxNpmCiAttempts
        }

        Invoke-GatewayReleaseStep -Name "typecheck desktop UI" -Action {
            & npm run typecheck
            if ($LASTEXITCODE -ne 0) {
                throw "npm typecheck failed for Gateway desktop UI"
            }
        }

        Invoke-GatewayReleaseStep -Name "build desktop Tauri shell" -Action {
            & npm run tauri -- build --no-bundle
            if ($LASTEXITCODE -ne 0) {
                throw "tauri build failed for Gateway desktop UI"
            }
        }
    } finally {
        Pop-Location
    }

    $headlessExe = Join-Path $repoRoot "target\\release\\neuro-gateway.exe"
    $uiExe = Join-Path $repoRoot "apps\\desktop\\src-tauri\\target\\release\\neuro-gateway-ui.exe"

    if (-not (Test-Path -LiteralPath $headlessExe -PathType Leaf)) {
        throw "Missing headless build artifact: $headlessExe"
    }
    if (-not (Test-Path -LiteralPath $uiExe -PathType Leaf)) {
        throw "Missing UI build artifact: $uiExe"
    }

    Write-ArtifactHashSummary -Paths @($headlessExe, $uiExe)
    $sourceTreeState = Get-SourceTreeState -Root $repoRoot
    Write-BuildProvenance `
        -GatewayRoot $repoRoot `
        -SourceTreeState $sourceTreeState `
        -ArtifactPaths @($headlessExe, $uiExe)
} finally {
    Pop-Location
}
