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

    if (-not ("GatewaySourceProvenanceHasher" -as [type])) {
        Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;

public static class GatewaySourceProvenanceHasher {
    private static string ToLowerHex(byte[] bytes) {
        char[] c = new char[bytes.Length * 2];
        const string hex = "0123456789abcdef";
        for (int i = 0; i < bytes.Length; i++) {
            byte b = bytes[i];
            c[i * 2] = hex[b >> 4];
            c[i * 2 + 1] = hex[b & 0x0F];
        }
        return new string(c);
    }

    public static string HashFileHex(string path) {
        using (var sha = SHA256.Create())
        using (var fs = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete, 65536, FileOptions.SequentialScan)) {
            return ToLowerHex(sha.ComputeHash(fs));
        }
    }

    public static Tuple<string, int> ComputeFingerprint(
        string root,
        string gitRoot,
        string[] candidatePaths,
        bool pathsAreGitRelative,
        string[] excludedDirs,
        string[] excludedRootDirs
    ) {
        string rootTrimmed = Path.GetFullPath(root).TrimEnd('\\', '/');
        string rootPrefix = rootTrimmed + Path.DirectorySeparatorChar;
        var dirSet = new HashSet<string>(excludedDirs ?? Array.Empty<string>(), StringComparer.OrdinalIgnoreCase);
        var rootDirSet = new HashSet<string>(excludedRootDirs ?? Array.Empty<string>(), StringComparer.OrdinalIgnoreCase);
        var work = new List<KeyValuePair<string, string>>(candidatePaths != null ? candidatePaths.Length : 0);

        if (candidatePaths != null) {
            foreach (string raw in candidatePaths) {
                if (string.IsNullOrWhiteSpace(raw)) continue;
                string fullPath = pathsAreGitRelative
                    ? Path.GetFullPath(Path.Combine(gitRoot, raw))
                    : Path.GetFullPath(raw);
                if (!File.Exists(fullPath)) continue;
                string relative;
                if (string.Equals(fullPath.TrimEnd('\\', '/'), rootTrimmed, StringComparison.OrdinalIgnoreCase)) {
                    relative = ".";
                } else if (fullPath.StartsWith(rootPrefix, StringComparison.OrdinalIgnoreCase)) {
                    relative = fullPath.Substring(rootPrefix.Length).Replace('\\', '/');
                } else {
                    continue;
                }
                string[] parts = relative.Split('/');
                bool skip = false;
                for (int i = 0; i < parts.Length; i++) {
                    string p = parts[i];
                    if (dirSet.Contains(p) || (i == 0 && rootDirSet.Contains(p)) || p.StartsWith("tmp-", StringComparison.OrdinalIgnoreCase)) {
                        skip = true;
                        break;
                    }
                }
                if (!skip) {
                    work.Add(new KeyValuePair<string, string>(fullPath, relative));
                }
            }
        }

        string[] records = new string[work.Count];
        var opts = new ParallelOptions { MaxDegreeOfParallelism = Math.Max(1, Math.Min(6, Environment.ProcessorCount)) };
        Parallel.For(0, work.Count, opts, idx => {
            var item = work[idx];
            long len = new FileInfo(item.Key).Length;
            string digest = HashFileHex(item.Key);
            records[idx] = string.Concat(item.Value, "\t", len.ToString(), "\t", digest, "\n");
        });

        Array.Sort(records, StringComparer.Ordinal);
        string payload = string.Concat(records);
        using (var sha = SHA256.Create()) {
            byte[] fp = sha.ComputeHash(Encoding.UTF8.GetBytes(payload));
            return Tuple.Create(ToLowerHex(fp), records.Length);
        }
    }
}
"@
    }

    $excludedDirectoryNames = @(".git", "target", "node_modules", ".runtime", "output")
    $excludedRootDirectoryNames = @("release")
    $gitRoot = Get-GitRepositoryRoot -Root $Root
    $gitCommand = Get-Command "git" -ErrorAction SilentlyContinue
    $algorithm = "sha256-file-list-v1"
    $fingerprint = $null
    $fileCount = 0
    if ($null -ne $gitRoot) {
        if ($null -eq $gitCommand) {
            throw "Unable to enumerate Gateway source files with Git because git is unavailable."
        }
        $pathSpec = Get-RelativeUnixPath -BasePath $gitRoot -Path $Root
        $gitPaths = @(& git -C $gitRoot -c core.quotepath=false ls-files --cached --others --exclude-standard -- $pathSpec 2>$null)
        if ($LASTEXITCODE -ne 0) {
            throw "Unable to enumerate Gateway source files with Git."
        }
        $algorithm = "sha256-git-source-list-v2"
        $computed = [GatewaySourceProvenanceHasher]::ComputeFingerprint(
            $Root,
            $gitRoot,
            [string[]]$gitPaths,
            $true,
            [string[]]$excludedDirectoryNames,
            [string[]]$excludedRootDirectoryNames
        )
        $fingerprint = $computed.Item1
        $fileCount = [int]$computed.Item2
    } else {
        $files = @(Get-ChildItem -LiteralPath $Root -Recurse -File -Force)
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

            $digest = [GatewaySourceProvenanceHasher]::HashFileHex($file.FullName)
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
        $fileCount = $sortedRecords.Count
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
        fingerprint = $fingerprint
        fileCount = $fileCount
        dirty = $dirty
    }
}

function Assert-BuildProvenance {
    param(
        [Parameter(Mandatory = $true)][string]$GatewayRoot,
        [Parameter(Mandatory = $true)][object]$SourceTreeState,
        [Parameter(Mandatory = $true)][string[]]$ArtifactRelativePaths,
        [ValidateSet("api", "web", "desktop")][string]$Edition = "desktop",
        [ValidateSet("release", "devtest")][string]$BuildKind = "release",
        [string]$ProvenanceRelativePath = "target/release/gateway-build-provenance.json"
    )

    $provenancePath = Join-Path $GatewayRoot $ProvenanceRelativePath
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
    $recordedKind = if ($provenance.PSObject.Properties.Name -contains "buildKind") { $provenance.buildKind } else { "release" }
    if ($recordedKind -ne $BuildKind) {
        throw "Gateway build kind mismatch: cannot package $recordedKind as $BuildKind."
    }
    # Older desktop receipts remain readable. A new edition must prove its exact
    # compile capability, even when a stale binary has an identical source tree.
    if ($provenance.PSObject.Properties.Name -contains "edition") {
        if ($provenance.edition -ne $Edition -or
            [bool]$provenance.capabilities.managementUi -ne ($Edition -ne "api") -or
            [bool]$provenance.capabilities.desktopUi -ne ($Edition -eq "desktop")) {
            throw "Gateway build provenance edition/capability mismatch. Rebuild the requested edition."
        }
    } elseif ($Edition -ne "desktop") {
        throw "Gateway build provenance has no edition. Rebuild the requested edition."
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
        $sha = [System.Security.Cryptography.SHA256]::Create()
        try {
            $stream = [System.IO.File]::Open($artifactPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
            try {
                $actualHash = ([System.BitConverter]::ToString($sha.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
            } finally {
                $stream.Dispose()
            }
        } finally {
            $sha.Dispose()
        }
        if ([int64]$matchingRecords[0].bytes -ne [int64]$artifact.Length -or
            [string]$matchingRecords[0].sha256 -ne $actualHash) {
            throw "Gateway build artifact no longer matches build provenance: $relativePath"
        }
    }

    return $provenance
}
