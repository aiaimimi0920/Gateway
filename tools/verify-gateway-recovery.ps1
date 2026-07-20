[CmdletBinding()]
param(
    [string]$WorkRoot = "",
    [string]$EvidencePath = "",
    [string]$DockerPath = "docker",
    [string]$RedisImage = "redis:7-alpine",
    [ValidateRange(0, 2147483647)][int]$RedisDatabase = 15,
    [ValidateRange(5, 300)][int]$StartupTimeoutSeconds = 30,
    [switch]$KeepArtifacts,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-RecoveryStatus {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Output "[gateway-recovery] $Message"
}

function Resolve-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.Path]::GetFullPath($Path)
}

function Test-PathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $rootFull = (Resolve-FullPath -Path $Root).TrimEnd("\", "/")
    $pathFull = (Resolve-FullPath -Path $Path).TrimEnd("\", "/")
    if ([string]::Equals($rootFull, $pathFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $true
    }
    $rootPrefix = $rootFull + [System.IO.Path]::DirectorySeparatorChar
    return $pathFull.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Assert-SafeCleanupPath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path
    )

    if (-not (Test-PathWithin -Root $Root -Path $Path)) {
        throw "Refusing to clean a path outside the recovery work root."
    }
    if ([string]::Equals(
            (Resolve-FullPath -Path $Root).TrimEnd("\", "/"),
            (Resolve-FullPath -Path $Path).TrimEnd("\", "/"),
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "Refusing to clean the recovery work root itself."
    }
}

function Resolve-NativeExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if (Test-Path -LiteralPath $Command -PathType Leaf) {
        return (Resolve-FullPath -Path $Command)
    }
    $resolved = Get-Command $Command -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $resolved) {
        throw "$Name executable was not found. Supply an explicit command path."
    }
    return $resolved.Source
}

function Invoke-Docker {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation,
        [switch]$AllowFailure
    )

    $output = & $DockerExecutable @Arguments 2>&1
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0 -and -not $AllowFailure) {
        throw "$Operation failed with exit code $exitCode."
    }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Text = (($output | ForEach-Object { [string]$_ }) -join "`n").Trim()
    }
}

function Invoke-Redis {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string]$Container,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation
    )

    $dockerArguments = @(
        "exec",
        $Container,
        "redis-cli",
        "-n",
        [string]$RedisDatabase
    ) + $Arguments
    return Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments $dockerArguments `
        -Operation $Operation
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [System.IO.File]::WriteAllText(
        $Path,
        $Value,
        [System.Text.UTF8Encoding]::new($false)
    )
}

$backupScript = Join-Path $PSScriptRoot "backup-gateway-state.ps1"
$restoreScript = Join-Path $PSScriptRoot "restore-gateway-state.ps1"
foreach ($script in @($backupScript, $restoreScript)) {
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "Required recovery script is missing: $script"
    }
}

$runId = [guid]::NewGuid().ToString("N")
$containerName = "gateway-recovery-$($runId.Substring(0, 12))"
$sourceNamespace = "gateway-recovery-source-$runId`:"
$destinationNamespace = "gateway-recovery-restored-$runId`:"

if ($DryRun) {
    Write-RecoveryStatus "dry-run would create disposable container=$containerName image=$RedisImage"
    Write-RecoveryStatus "dry-run would use isolated source and destination Redis namespaces"
    Write-RecoveryStatus "dry-run would exercise local object storage backup and restore"
    return
}

$docker = Resolve-NativeExecutable -Command $DockerPath -Name "docker"
$workParent = if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    Join-Path ([System.IO.Path]::GetTempPath()) "gateway-recovery-work"
} else {
    Resolve-FullPath -Path $WorkRoot
}
New-Item -ItemType Directory -Path $workParent -Force | Out-Null
$workPath = Resolve-FullPath -Path (Join-Path $workParent "gateway-recovery-$runId")
Assert-SafeCleanupPath -Root $workParent -Path $workPath
New-Item -ItemType Directory -Path $workPath | Out-Null

$evidenceFull = if ([string]::IsNullOrWhiteSpace($EvidencePath)) {
    Resolve-FullPath -Path (
        Join-Path ([System.IO.Path]::GetTempPath()) "gateway-recovery-evidence-$runId.json"
    )
} else {
    Resolve-FullPath -Path $EvidencePath
}

$objectSource = Join-Path $workPath "object-source"
$objectRestored = Join-Path $workPath "object-restored"
$backupPath = Join-Path $workPath "backup"
New-Item -ItemType Directory -Path (Join-Path $objectSource "profiles\tenant-a") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $objectSource "artifacts") -Force | Out-Null
Write-Utf8NoBom `
    -Path (Join-Path $objectSource "profiles\tenant-a\state.json") `
    -Value (([ordered]@{ runId = $runId; status = "ready" } | ConvertTo-Json) + "`n")
[System.IO.File]::WriteAllBytes(
    (Join-Path $objectSource "artifacts\payload.bin"),
    [System.Text.Encoding]::UTF8.GetBytes("gateway-recovery-object-$runId")
)

$containerStarted = $false
$verificationPassed = $false
$startedAt = [DateTimeOffset]::UtcNow
$failureMessage = $null
try {
    # docker run uses --network none and publishes no host port; only docker exec can reach it.
    Write-RecoveryStatus "docker run isolated Redis container=$containerName"
    $run = Invoke-Docker `
        -DockerExecutable $docker `
        -Arguments @(
            "run",
            "--detach",
            "--rm",
            "--name",
            $containerName,
            "--network",
            "none",
            $RedisImage
        ) `
        -Operation "docker run isolated Redis"
    if ([string]::IsNullOrWhiteSpace($run.Text)) {
        throw "docker run did not return a container id."
    }
    $containerStarted = $true

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $pingReady = $false
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $ping = Invoke-Docker `
            -DockerExecutable $docker `
            -Arguments @(
                "exec",
                $containerName,
                "redis-cli",
                "-n",
                [string]$RedisDatabase,
                "PING"
            ) `
            -Operation "Redis PING" `
            -AllowFailure
        if ($ping.ExitCode -eq 0 -and $ping.Text.Trim() -eq "PONG") {
            $pingReady = $true
            break
        }
        Start-Sleep -Milliseconds 200
    }
    if (-not $pingReady) {
        throw "Disposable Redis did not become ready before the timeout."
    }

    $sourceKeys = @(
        "$sourceNamespace`persistent",
        "$sourceNamespace`expiring",
        "$sourceNamespace`hash"
    )
    $null = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("SET", $sourceKeys[0], "persistent-$runId") `
        -Operation "seed persistent Redis key"
    $null = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("SET", $sourceKeys[1], "expiring-$runId", "PX", "120000") `
        -Operation "seed expiring Redis key"
    $null = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("HSET", $sourceKeys[2], "field", "hash-$runId") `
        -Operation "seed Redis hash"

    $backupOutput = & $backupScript `
        -DestinationPath $backupPath `
        -RedisContainer $containerName `
        -RedisDatabase $RedisDatabase `
        -RedisNamespace $sourceNamespace `
        -DockerPath $docker `
        -ObjectStoragePath $objectSource
    $backupOutput | ForEach-Object { Write-RecoveryStatus ([string]$_) }

    foreach ($key in $sourceKeys) {
        $null = Invoke-Redis `
            -DockerExecutable $docker `
            -Container $containerName `
            -Arguments @("DEL", $key) `
            -Operation "remove source Redis fixture"
    }

    $restoreOutput = & $restoreScript `
        -BackupPath $backupPath `
        -RedisContainer $containerName `
        -RedisDatabase $RedisDatabase `
        -RedisDestinationNamespace $destinationNamespace `
        -DockerPath $docker `
        -ObjectStorageDestinationPath $objectRestored
    $restoreOutput | ForEach-Object { Write-RecoveryStatus ([string]$_) }

    $destinationKeys = @(
        "$destinationNamespace`persistent",
        "$destinationNamespace`expiring",
        "$destinationNamespace`hash"
    )
    $persistent = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("--raw", "GET", $destinationKeys[0]) `
        -Operation "verify persistent Redis key"
    $expiring = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("--raw", "GET", $destinationKeys[1]) `
        -Operation "verify expiring Redis key"
    $hash = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("--raw", "HGET", $destinationKeys[2], "field") `
        -Operation "verify Redis hash"
    $ttl = Invoke-Redis `
        -DockerExecutable $docker `
        -Container $containerName `
        -Arguments @("--raw", "PTTL", $destinationKeys[1]) `
        -Operation "verify Redis TTL"

    if ($persistent.Text.Trim() -ne "persistent-$runId") {
        throw "Persistent Redis value did not survive recovery."
    }
    if ($expiring.Text.Trim() -ne "expiring-$runId") {
        throw "Expiring Redis value did not survive recovery."
    }
    if ($hash.Text.Trim() -ne "hash-$runId") {
        throw "Redis hash did not survive recovery."
    }
    $ttlMs = 0L
    if (-not [long]::TryParse($ttl.Text.Trim(), [ref]$ttlMs) -or $ttlMs -le 0) {
        throw "Redis TTL was not restored."
    }

    $objectSourceHashes = @{}
    foreach ($file in @(Get-ChildItem -LiteralPath $objectSource -Recurse -File)) {
        $relative = $file.FullName.Substring($objectSource.Length).TrimStart("\", "/")
        $objectSourceHashes[$relative] = (
            Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256
        ).Hash.ToLowerInvariant()
    }
    $objectRestoredHashes = @{}
    foreach ($file in @(Get-ChildItem -LiteralPath $objectRestored -Recurse -File)) {
        $relative = $file.FullName.Substring($objectRestored.Length).TrimStart("\", "/")
        $objectRestoredHashes[$relative] = (
            Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256
        ).Hash.ToLowerInvariant()
    }
    if ($objectSourceHashes.Count -ne $objectRestoredHashes.Count) {
        throw "Object storage file count changed during recovery."
    }
    foreach ($relative in $objectSourceHashes.Keys) {
        if (-not $objectRestoredHashes.ContainsKey($relative) -or
            $objectRestoredHashes[$relative] -ne $objectSourceHashes[$relative]) {
            throw "Object storage checksum mismatch after recovery."
        }
    }

    $verificationPassed = $true
    $evidence = [ordered]@{
        schemaVersion = 1
        kind = "gateway-recovery-verification"
        status = "pass"
        startedAt = $startedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        redis = [ordered]@{
            image = $RedisImage
            database = $RedisDatabase
            sourceNamespaceLength = $sourceNamespace.Length
            destinationNamespaceLength = $destinationNamespace.Length
            restoredKeyCount = $destinationKeys.Count
            expiringKeyTtlMs = $ttlMs
            network = "none"
            publishedPorts = 0
        }
        objectStorage = [ordered]@{
            restoredFileCount = $objectRestoredHashes.Count
        }
        backupManifestSha256 = (
            Get-FileHash -LiteralPath (Join-Path $backupPath "manifest.json") -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        backupChecksumsSha256 = (
            Get-FileHash -LiteralPath (Join-Path $backupPath "checksums.sha256") -Algorithm SHA256
        ).Hash.ToLowerInvariant()
    }
    Write-Utf8NoBom `
        -Path $evidenceFull `
        -Value (($evidence | ConvertTo-Json -Depth 8) + "`n")
    Write-RecoveryStatus "verification passed evidence=$evidenceFull"
} catch {
    $failureMessage = $_.Exception.Message
    $failureEvidence = [ordered]@{
        schemaVersion = 1
        kind = "gateway-recovery-verification"
        status = "fail"
        startedAt = $startedAt.ToString("o")
        completedAt = [DateTimeOffset]::UtcNow.ToString("o")
        error = $failureMessage
    }
    Write-Utf8NoBom `
        -Path $evidenceFull `
        -Value (($failureEvidence | ConvertTo-Json -Depth 4) + "`n")
    throw
} finally {
    if ($containerStarted) {
        $null = Invoke-Docker `
            -DockerExecutable $docker `
            -Arguments @("rm", "--force", $containerName) `
            -Operation "remove disposable Redis container" `
            -AllowFailure
    }
    if (-not $KeepArtifacts -and (Test-Path -LiteralPath $workPath)) {
        Assert-SafeCleanupPath -Root $workParent -Path $workPath
        Remove-Item -LiteralPath $workPath -Recurse -Force
    } elseif ($KeepArtifacts) {
        Write-RecoveryStatus "artifacts retained workPath=$workPath"
    }
}

if (-not $verificationPassed) {
    throw "Gateway recovery verification failed: $failureMessage"
}
