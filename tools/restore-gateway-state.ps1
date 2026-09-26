[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BackupPath,
    [string]$RedisUrl = "",
    [string]$RedisContainer = "",
    [ValidateRange(0, 2147483647)][int]$RedisDatabase = 0,
    [string]$RedisDestinationNamespace = "",
    [string]$RedisCliPath = "redis-cli",
    [string]$DockerPath = "docker",
    [switch]$AllowSameRedisNamespace,
    [switch]$OverwriteRedisKeys,
    [string]$PostgresConnection = "",
    [string]$PostgresContainer = "",
    [string]$PsqlPath = "psql",
    [switch]$ConfirmPostgresRestore,
    [string]$ObjectStorageDestinationPath = "",
    [switch]$OverwriteObjectStorage,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "state-recovery/native-process.ps1")

function Write-RestoreStatus {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Output "[gateway-restore] $Message"
}

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

function Assert-NotFileSystemRoot {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $fullPath = Resolve-FullPath -Path $Path
    $root = [System.IO.Path]::GetPathRoot($fullPath)
    if ([string]::Equals(
            $fullPath.TrimEnd("\", "/"),
            $root.TrimEnd("\", "/"),
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw "$Name must not be a filesystem root: $fullPath"
    }
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

function Assert-PathUnderRoot {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path
    )

    if (-not (Test-PathWithin -Root $Root -Path $Path)) {
        throw "Backup payload path escapes the backup root."
    }
}

function Resolve-BackupPayloadPath {
    param(
        [Parameter(Mandatory = $true)][string]$BackupRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    if ([string]::IsNullOrWhiteSpace($RelativePath) -or
        [System.IO.Path]::IsPathRooted($RelativePath) -or
        $RelativePath.Contains("\") -or
        $RelativePath.Contains("`r") -or
        $RelativePath.Contains("`n")) {
        throw "Backup contains an unsafe relative payload path."
    }
    $parts = @($RelativePath -split "/")
    if ($parts -contains ".." -or $parts -contains "." -or $parts -contains "") {
        throw "Backup contains an unsafe relative payload path."
    }
    $path = $BackupRoot
    foreach ($part in $parts) {
        $path = Join-Path $path $part
    }
    $fullPath = Resolve-FullPath -Path $path
    Assert-PathUnderRoot -Root $BackupRoot -Path $fullPath
    return $fullPath
}

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseFull = (Resolve-FullPath -Path $BasePath).TrimEnd("\", "/") +
        [System.IO.Path]::DirectorySeparatorChar
    $pathFull = Resolve-FullPath -Path $Path
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString(
        $baseUri.MakeRelativeUri($pathUri).ToString()
    ).Replace("\", "/")
}


function Get-RedisInvocation {
    param(
        [Parameter(Mandatory = $true)][string[]]$CommandArguments,
        [switch]$WithInput
    )

    if (-not [string]::IsNullOrWhiteSpace($RedisContainer)) {
        $docker = Resolve-NativeExecutable -Command $DockerPath -Name "docker"
        $arguments = @("exec")
        if ($WithInput) {
            $arguments += "-i"
        }
        $arguments += @(
            $RedisContainer,
            "redis-cli",
            "-n",
            [string]$RedisDatabase
        )
        $arguments += $CommandArguments
        return [pscustomobject]@{ FilePath = $docker; Arguments = $arguments }
    }

    $redisCli = Resolve-NativeExecutable -Command $RedisCliPath -Name "redis-cli"
    $arguments = @("-u", $RedisUrl)
    $arguments += $CommandArguments
    return [pscustomobject]@{ FilePath = $redisCli; Arguments = $arguments }
}

function Get-PostgresInvocation {
    param(
        [Parameter(Mandatory = $true)][string]$ClientCommand,
        [Parameter(Mandatory = $true)][string]$ClientPath,
        [Parameter(Mandatory = $true)][string[]]$CommandArguments
    )

    if (-not [string]::IsNullOrWhiteSpace($PostgresContainer)) {
        $docker = Resolve-NativeExecutable -Command $DockerPath -Name "docker"
        return [pscustomobject]@{
            FilePath = $docker
            Arguments = @("exec", $PostgresContainer, $ClientCommand) + $CommandArguments
        }
    }

    $client = Resolve-NativeExecutable -Command $ClientPath -Name $ClientCommand
    return [pscustomobject]@{ FilePath = $client; Arguments = $CommandArguments }
}

function Invoke-RedisText {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation
    )

    $invocation = Get-RedisInvocation -CommandArguments $Arguments
    $output = & $invocation.FilePath @($invocation.Arguments) 2>$null
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        throw "$Operation failed with exit code $exitCode. Endpoint details were redacted."
    }
    return (($output | ForEach-Object { [string]$_ }) -join "`n").Trim()
}

function Restore-RedisPayload {
    param(
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][long]$TtlMs,
        [Parameter(Mandatory = $true)][byte[]]$Payload
    )

    $restoreTtl = if ($TtlMs -lt 0) { 0L } else { $TtlMs }
    $hexPayload = [System.BitConverter]::ToString($Payload).Replace("-", "").ToLowerInvariant()
    $hexBytes = [System.Text.Encoding]::ASCII.GetBytes($hexPayload)
    # Docker CLI on Windows can prefix redirected stdin with a UTF-8 BOM.
    $hexRestore = "local hex = ARGV[2]; local bom = string.char(239, 187, 191); if string.sub(hex, 1, 3) == bom then hex = string.sub(hex, 4) end; hex = (hex:gsub('%s', '')); if (#hex % 2) ~= 0 or string.find(hex, '[^0-9a-fA-F]') then return redis.error_reply('invalid gateway hex payload') end; local serialized = (hex:gsub('..', function(pair) return string.char(tonumber(pair, 16)) end)); return redis.call('RESTORE', KEYS[1], ARGV[1], serialized)"
    $invocation = Get-RedisInvocation `
        -CommandArguments @(
            "-X",
            "gateway_hex_payload",
            "EVAL",
            $hexRestore,
            "1",
            $Key,
            [string]$restoreTtl,
            "gateway_hex_payload"
        ) `
        -WithInput
    $result = Invoke-NativeCapture `
        -FilePath $invocation.FilePath `
        -Arguments $invocation.Arguments `
        -Operation "Redis RESTORE" `
        -InputBytes $hexBytes
    $response = $result.Text.Trim()
    if ($response -ne "OK") {
        throw "Redis RESTORE returned an unexpected response: $response"
    }
}

function Copy-LocalObjectTree {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [switch]$Overwrite
    )

    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    foreach ($item in @(Get-ChildItem -LiteralPath $Source -Force | Sort-Object Name)) {
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Object storage restore refuses reparse points: $($item.FullName)"
        }
        $target = Join-Path $Destination $item.Name
        if ($item.PSIsContainer) {
            Copy-LocalObjectTree -Source $item.FullName -Destination $target -Overwrite:$Overwrite
            continue
        }
        if ((Test-Path -LiteralPath $target) -and -not $Overwrite) {
            throw "Object storage destination contains a colliding file."
        }
        Copy-Item -LiteralPath $item.FullName -Destination $target -Force:$Overwrite
    }
}

function Assert-BackupChecksums {
    param([Parameter(Mandatory = $true)][string]$BackupRoot)

    $checksumPath = Join-Path $BackupRoot "checksums.sha256"
    if (-not (Test-Path -LiteralPath $checksumPath -PathType Leaf)) {
        throw "Backup checksum manifest is missing."
    }

    $expectedPaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($line in @(Get-Content -LiteralPath $checksumPath -Encoding ASCII)) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        if ($line -notmatch '^([0-9a-fA-F]{64})  (.+)$') {
            throw "Backup checksum manifest contains an invalid line."
        }
        $digest = $Matches[1].ToLowerInvariant()
        $relative = $Matches[2]
        if (-not $expectedPaths.Add($relative)) {
            throw "Backup checksum manifest contains a duplicate path."
        }
        $payload = Resolve-BackupPayloadPath -BackupRoot $BackupRoot -RelativePath $relative
        if (-not (Test-Path -LiteralPath $payload -PathType Leaf)) {
            throw "Backup checksum references a missing payload."
        }
        $actual = (Get-FileHash -LiteralPath $payload -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $digest) {
            throw "Backup checksum mismatch for a payload; restore was not started."
        }
    }

    $actualPaths = @(
        Get-ChildItem -LiteralPath $BackupRoot -Recurse -File |
            Where-Object { $_.Name -ne "checksums.sha256" } |
            ForEach-Object { Get-RelativeUnixPath -BasePath $BackupRoot -Path $_.FullName }
    )
    foreach ($relative in $actualPaths) {
        if (-not $expectedPaths.Contains($relative)) {
            throw "Backup contains a payload that is not covered by checksums."
        }
    }
    if ($expectedPaths.Count -ne $actualPaths.Count) {
        throw "Backup checksum coverage does not match the payload set."
    }
}

$backupFull = Resolve-FullPath -Path $BackupPath -RequireExisting
if (-not (Test-Path -LiteralPath $backupFull -PathType Container)) {
    throw "BackupPath must be a directory: $backupFull"
}
Assert-NotFileSystemRoot -Path $backupFull -Name "BackupPath"
Assert-BackupChecksums -BackupRoot $backupFull

$manifestPath = Join-Path $backupFull "manifest.json"
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Backup manifest is missing."
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or $manifest.kind -ne "gateway-state-backup") {
    throw "Unsupported Gateway backup manifest."
}

$redisRequested = -not [string]::IsNullOrWhiteSpace($RedisUrl) -or
    -not [string]::IsNullOrWhiteSpace($RedisContainer)
$postgresRequested = -not [string]::IsNullOrWhiteSpace($PostgresConnection)
$objectStorageRequested = -not [string]::IsNullOrWhiteSpace($ObjectStorageDestinationPath)
if (-not $redisRequested -and -not $postgresRequested -and -not $objectStorageRequested) {
    throw "No restore destination was selected. Supply explicit Redis, PostgreSQL, or object storage parameters."
}

if (-not [string]::IsNullOrWhiteSpace($RedisUrl) -and
    -not [string]::IsNullOrWhiteSpace($RedisContainer)) {
    throw "RedisUrl and RedisContainer are mutually exclusive."
}
if ($redisRequested) {
    if (-not [bool]$manifest.components.redis.included) {
        throw "The backup does not contain Redis state."
    }
    if ([string]::IsNullOrWhiteSpace($RedisDestinationNamespace)) {
        throw "RedisDestinationNamespace is required for Redis restore."
    }
    if ($RedisDestinationNamespace.IndexOfAny([char[]]"*?[]") -ge 0) {
        throw "RedisDestinationNamespace must be a literal prefix without glob characters."
    }
    if (-not $AllowSameRedisNamespace -and
        [string]::Equals(
            [string]$manifest.components.redis.namespace,
            $RedisDestinationNamespace,
            [System.StringComparison]::Ordinal
        )) {
        throw "Refusing to restore into the source Redis namespace. Use a different namespace or pass AllowSameRedisNamespace explicitly."
    }
    if (-not [string]::IsNullOrWhiteSpace($RedisContainer) -and
        $RedisContainer -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]*$') {
        throw "RedisContainer contains unsupported characters."
    }
}

if ($postgresRequested) {
    if (-not [bool]$manifest.components.postgresql.included) {
        throw "The backup does not contain PostgreSQL state."
    }
    if (-not $ConfirmPostgresRestore) {
        throw "PostgreSQL restore requires ConfirmPostgresRestore."
    }
}
if (-not [string]::IsNullOrWhiteSpace($PostgresContainer)) {
    if (-not $postgresRequested) {
        throw "PostgresContainer requires PostgresConnection."
    }
    if ($PostgresContainer -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]*$') {
        throw "PostgresContainer contains unsupported characters."
    }
}

$objectStorageDestinationFull = ""
if ($objectStorageRequested) {
    if (-not [bool]$manifest.components.objectStorage.included) {
        throw "The backup does not contain local object storage state."
    }
    $objectStorageDestinationFull = Resolve-FullPath -Path $ObjectStorageDestinationPath
    Assert-NotFileSystemRoot -Path $objectStorageDestinationFull -Name "ObjectStorageDestinationPath"
    if ((Test-PathWithin -Root $backupFull -Path $objectStorageDestinationFull) -or
        (Test-PathWithin -Root $objectStorageDestinationFull -Path $backupFull)) {
        throw "BackupPath and ObjectStorageDestinationPath must not overlap."
    }
    if (Test-Path -LiteralPath $objectStorageDestinationFull) {
        if (-not (Test-Path -LiteralPath $objectStorageDestinationFull -PathType Container)) {
            throw "ObjectStorageDestinationPath must be a directory."
        }
        $existingItems = @(Get-ChildItem -LiteralPath $objectStorageDestinationFull -Force)
        if ($existingItems.Count -gt 0 -and -not $OverwriteObjectStorage) {
            throw "ObjectStorageDestinationPath is not empty. Pass OverwriteObjectStorage to replace colliding files only."
        }
    }
}

if ($DryRun) {
    Write-RestoreStatus "dry-run backup=$backupFull"
    if ($redisRequested) {
        $mode = if ([string]::IsNullOrWhiteSpace($RedisContainer)) { "redis-cli URL" } else { "docker exec" }
        Write-RestoreStatus "dry-run Redis selected mode=$mode namespaceLength=$($RedisDestinationNamespace.Length) keys=$($manifest.components.redis.keyCount)"
    }
    if ($postgresRequested) {
        $mode = if ([string]::IsNullOrWhiteSpace($PostgresContainer)) { "psql executable" } else { "docker exec" }
        Write-RestoreStatus "dry-run PostgreSQL selected mode=$mode"
    }
    if ($objectStorageRequested) {
        Write-RestoreStatus "dry-run local object storage selected destination=$objectStorageDestinationFull"
    }
    return
}

$redisEntries = @()
if ($redisRequested) {
    $utf8Strict = [System.Text.UTF8Encoding]::new($false, $true)
    foreach ($entry in @($manifest.components.redis.entries)) {
        try {
            $suffixBytes = [Convert]::FromBase64String([string]$entry.keySuffixBase64)
            $suffix = $utf8Strict.GetString($suffixBytes)
        } catch {
            throw "Redis backup contains an invalid key suffix."
        }
        $targetKey = $RedisDestinationNamespace + $suffix
        $payloadPath = Resolve-BackupPayloadPath `
            -BackupRoot $backupFull `
            -RelativePath ([string]$entry.payload)
        $existsText = Invoke-RedisText `
            -Arguments @("--raw", "EXISTS", $targetKey) `
            -Operation "Redis EXISTS"
        $exists = 0L
        if (-not [long]::TryParse($existsText, [ref]$exists)) {
            throw "Redis EXISTS returned an unexpected response."
        }
        if ($exists -gt 0 -and -not $OverwriteRedisKeys) {
            throw "Redis destination namespace contains a colliding key. No Redis keys were restored."
        }
        $redisEntries += [pscustomobject]@{
            Key = $targetKey
            TtlMs = [long]$entry.ttlMs
            PayloadPath = $payloadPath
            Exists = $exists -gt 0
        }
    }
}

if ($redisRequested) {
    foreach ($entry in $redisEntries) {
        if ($entry.Exists -and $OverwriteRedisKeys) {
            $deleted = Invoke-RedisText `
                -Arguments @("--raw", "DEL", $entry.Key) `
                -Operation "Redis DEL for explicit overwrite"
            if ($deleted -notin @("0", "1")) {
                throw "Redis DEL returned an unexpected response."
            }
        }
        $payload = [System.IO.File]::ReadAllBytes($entry.PayloadPath)
        Restore-RedisPayload -Key $entry.Key -TtlMs $entry.TtlMs -Payload $payload
    }
    Write-RestoreStatus "restored Redis namespace keys=$($redisEntries.Count)"
}

if ($postgresRequested) {
    $postgresPayload = Resolve-BackupPayloadPath `
        -BackupRoot $backupFull `
        -RelativePath ([string]$manifest.components.postgresql.payload)
    $psqlArguments = @(
        "--dbname=$PostgresConnection",
        "--set=ON_ERROR_STOP=on"
    )
    if ([string]::IsNullOrWhiteSpace($PostgresContainer)) {
        $psqlInvocation = Get-PostgresInvocation `
            -ClientCommand "psql" `
            -ClientPath $PsqlPath `
            -CommandArguments ($psqlArguments + "--file=$postgresPayload")
        $null = Invoke-NativeCapture `
            -FilePath $psqlInvocation.FilePath `
            -Arguments $psqlInvocation.Arguments `
            -Operation "psql restore"
    } else {
        $docker = Resolve-NativeExecutable -Command $DockerPath -Name "docker"
        $remotePayload = "/tmp/gateway-restore-$([guid]::NewGuid().ToString('N')).sql"
        $copyCompleted = $false
        $restoreCompleted = $false
        $cleanupError = $null
        try {
            $null = Invoke-NativeCapture `
                -FilePath $docker `
                -Arguments @(
                    "cp",
                    $postgresPayload,
                    "${PostgresContainer}:$remotePayload"
                ) `
                -Operation "copy PostgreSQL restore payload"
            $copyCompleted = $true
            $psqlInvocation = Get-PostgresInvocation `
                -ClientCommand "psql" `
                -ClientPath $PsqlPath `
                -CommandArguments ($psqlArguments + "--file=$remotePayload")
            $null = Invoke-NativeCapture `
                -FilePath $psqlInvocation.FilePath `
                -Arguments $psqlInvocation.Arguments `
                -Operation "psql restore"
            $restoreCompleted = $true
        } finally {
            if ($copyCompleted) {
                try {
                    $null = Invoke-NativeCapture `
                        -FilePath $docker `
                        -Arguments @(
                            "exec",
                            $PostgresContainer,
                            "rm",
                            "-f",
                            $remotePayload
                        ) `
                        -Operation "remove PostgreSQL restore payload"
                } catch {
                    $cleanupError = $_.Exception
                }
            }
            if ($restoreCompleted -and $null -ne $cleanupError) {
                throw "PostgreSQL restore succeeded but temporary payload cleanup failed."
            }
        }
    }
    Write-RestoreStatus "restored PostgreSQL plain SQL dump"
}

if ($objectStorageRequested) {
    $objectStorageSource = Resolve-BackupPayloadPath `
        -BackupRoot $backupFull `
        -RelativePath ([string]$manifest.components.objectStorage.path)
    if (-not (Test-Path -LiteralPath $objectStorageSource -PathType Container)) {
        throw "Object storage backup payload is missing."
    }
    Copy-LocalObjectTree `
        -Source $objectStorageSource `
        -Destination $objectStorageDestinationFull `
        -Overwrite:$OverwriteObjectStorage
    Write-RestoreStatus "restored local object storage files=$($manifest.components.objectStorage.fileCount) destination=$objectStorageDestinationFull"
}

Write-RestoreStatus "completed backup=$backupFull"
