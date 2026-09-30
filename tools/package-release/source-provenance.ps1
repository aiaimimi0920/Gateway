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
                    Get-Item -LiteralPath $fullPath -Force
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
