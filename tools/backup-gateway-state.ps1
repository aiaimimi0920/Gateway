[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DestinationPath,
    [string]$RedisUrl = "",
    [string]$RedisContainer = "",
    [ValidateRange(0, 2147483647)][int]$RedisDatabase = 0,
    [string]$RedisNamespace = "",
    [string]$RedisCliPath = "redis-cli",
    [string]$DockerPath = "docker",
    [string]$PostgresConnection = "",
    [string]$PostgresContainer = "",
    [string]$PgDumpPath = "pg_dump",
    [string]$ObjectStoragePath = "",
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-BackupStatus {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Output "[gateway-backup] $Message"
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
        throw "Refusing to write outside the expected root. Root=[$Root] Path=[$Path]"
    }
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

function Write-Ascii {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )

    [System.IO.File]::WriteAllText(
        $Path,
        $Value,
        [System.Text.ASCIIEncoding]::new()
    )
}

function Resolve-NativeExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if (Test-Path -LiteralPath $Command -PathType Leaf) {
        return (Resolve-FullPath -Path $Command -RequireExisting)
    }
    $resolved = Get-Command $Command -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $resolved) {
        throw "$Name executable was not found. Supply an explicit command path."
    }
    return $resolved.Source
}

function ConvertTo-ProcessArgument {
    param([AllowEmptyString()][string]$Value)

    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') {
        return $Value
    }

    $builder = [System.Text.StringBuilder]::new()
    [void]$builder.Append('"')
    $backslashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') {
            $backslashes += 1
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($backslashes * 2) + 1)))
            [void]$builder.Append('"')
            $backslashes = 0
            continue
        }
        if ($backslashes -gt 0) {
            [void]$builder.Append(('\' * $backslashes))
            $backslashes = 0
        }
        [void]$builder.Append($character)
    }
    if ($backslashes -gt 0) {
        [void]$builder.Append(('\' * ($backslashes * 2)))
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Invoke-NativeCapture {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation,
        [byte[]]$InputBytes = $null
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = (($Arguments | ForEach-Object {
        ConvertTo-ProcessArgument -Value ([string]$_)
    }) -join " ")
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.RedirectStandardInput = $null -ne $InputBytes

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "$Operation failed to start."
    }

    $output = [System.IO.MemoryStream]::new()
    $outputTask = $process.StandardOutput.BaseStream.CopyToAsync($output)
    $errorTask = $process.StandardError.ReadToEndAsync()
    if ($null -ne $InputBytes) {
        $process.StandardInput.BaseStream.Write($InputBytes, 0, $InputBytes.Length)
        $process.StandardInput.BaseStream.Flush()
        $process.StandardInput.Close()
    }

    $null = $process.WaitForExit()
    $null = $outputTask.GetAwaiter().GetResult()
    $null = $errorTask.GetAwaiter().GetResult()
    $exitCode = $process.ExitCode
    $bytes = $output.ToArray()
    $output.Dispose()
    $process.Dispose()

    if ($exitCode -ne 0) {
        throw "$Operation failed with exit code $exitCode. Endpoint details were redacted."
    }

    return [pscustomobject]@{
        Bytes = $bytes
        Text = [System.Text.Encoding]::UTF8.GetString($bytes)
    }
}

function Invoke-NativeToFile {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation,
        [Parameter(Mandatory = $true)][string]$OutputPath
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = (($Arguments | ForEach-Object {
        ConvertTo-ProcessArgument -Value ([string]$_)
    }) -join " ")
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $output = [System.IO.File]::Open(
        $OutputPath,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None
    )
    try {
        if (-not $process.Start()) {
            throw "$Operation failed to start."
        }
        $outputTask = $process.StandardOutput.BaseStream.CopyToAsync($output)
        $errorTask = $process.StandardError.ReadToEndAsync()
        $null = $process.WaitForExit()
        $null = $outputTask.GetAwaiter().GetResult()
        $null = $errorTask.GetAwaiter().GetResult()
        $exitCode = $process.ExitCode
    } finally {
        $output.Dispose()
        $process.Dispose()
    }

    if ($exitCode -ne 0) {
        throw "$Operation failed with exit code $exitCode. Endpoint details were redacted."
    }
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

function Get-RedisDumpBytes {
    param([Parameter(Mandatory = $true)][string]$Key)

    $hexEncoder = "local value = redis.call('DUMP', KEYS[1]); if not value then return false end; return (value:gsub('.', function(character) return string.format('%02x', string.byte(character)) end))"
    $hex = Invoke-RedisText `
        -Arguments @("--raw", "EVAL", $hexEncoder, "1", $Key) `
        -Operation "Redis DUMP"
    if ([string]::IsNullOrWhiteSpace($hex)) {
        throw "Redis DUMP returned no payload. The key may have changed during backup."
    }
    if (($hex.Length % 2) -ne 0 -or $hex -notmatch '^[0-9a-fA-F]+$') {
        throw "Redis DUMP hex transport returned an invalid payload."
    }
    $bytes = [byte[]]::new($hex.Length / 2)
    for ($index = 0; $index -lt $bytes.Length; $index += 1) {
        $bytes[$index] = [Convert]::ToByte($hex.Substring($index * 2, 2), 16)
    }
    return $bytes
}

function Copy-LocalObjectTree {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    foreach ($item in @(Get-ChildItem -LiteralPath $Source -Force | Sort-Object Name)) {
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Object storage backup refuses reparse points: $($item.FullName)"
        }
        $target = Join-Path $Destination $item.Name
        Assert-PathUnderRoot -Root $Destination -Path $target
        if ($item.PSIsContainer) {
            Copy-LocalObjectTree -Source $item.FullName -Destination $target
        } else {
            Copy-Item -LiteralPath $item.FullName -Destination $target
        }
    }
}

function Write-ChecksumManifest {
    param([Parameter(Mandatory = $true)][string]$BackupRoot)

    $lines = [System.Collections.Generic.List[string]]::new()
    $files = @(
        Get-ChildItem -LiteralPath $BackupRoot -Recurse -File |
            Where-Object { $_.Name -ne "checksums.sha256" } |
            Sort-Object FullName
    )
    foreach ($file in $files) {
        $relative = Get-RelativeUnixPath -BasePath $BackupRoot -Path $file.FullName
        if ($relative.Contains("`r") -or $relative.Contains("`n")) {
            throw "Backup paths containing newlines are not supported."
        }
        $digest = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $lines.Add("$digest  $relative") | Out-Null
    }
    $value = if ($lines.Count -eq 0) { "" } else { ($lines -join "`n") + "`n" }
    Write-Ascii -Path (Join-Path $BackupRoot "checksums.sha256") -Value $value
}

$redisRequested = -not [string]::IsNullOrWhiteSpace($RedisUrl) -or
    -not [string]::IsNullOrWhiteSpace($RedisContainer)
$postgresRequested = -not [string]::IsNullOrWhiteSpace($PostgresConnection)
$objectStorageRequested = -not [string]::IsNullOrWhiteSpace($ObjectStoragePath)

if (-not $redisRequested -and -not $postgresRequested -and -not $objectStorageRequested) {
    throw "No state source was selected. Supply Redis, PostgreSQL, or local object storage parameters."
}
if (-not [string]::IsNullOrWhiteSpace($RedisUrl) -and
    -not [string]::IsNullOrWhiteSpace($RedisContainer)) {
    throw "RedisUrl and RedisContainer are mutually exclusive."
}
if ($redisRequested) {
    if ([string]::IsNullOrWhiteSpace($RedisNamespace)) {
        throw "RedisNamespace is required for namespace-scoped Redis backup."
    }
    if ($RedisNamespace.IndexOfAny([char[]]"*?[]") -ge 0) {
        throw "RedisNamespace must be a literal prefix without glob characters."
    }
    if (-not [string]::IsNullOrWhiteSpace($RedisContainer) -and
        $RedisContainer -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]*$') {
        throw "RedisContainer contains unsupported characters."
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

$destinationFull = Resolve-FullPath -Path $DestinationPath
Assert-NotFileSystemRoot -Path $destinationFull -Name "DestinationPath"
if (Test-Path -LiteralPath $destinationFull) {
    throw "DestinationPath already exists. Use a new, explicit backup directory: $destinationFull"
}

$objectStorageFull = ""
if ($objectStorageRequested) {
    $objectStorageFull = Resolve-FullPath -Path $ObjectStoragePath -RequireExisting
    if (-not (Test-Path -LiteralPath $objectStorageFull -PathType Container)) {
        throw "ObjectStoragePath must be a directory: $objectStorageFull"
    }
    Assert-NotFileSystemRoot -Path $objectStorageFull -Name "ObjectStoragePath"
    if ((Test-PathWithin -Root $objectStorageFull -Path $destinationFull) -or
        (Test-PathWithin -Root $destinationFull -Path $objectStorageFull)) {
        throw "DestinationPath and ObjectStoragePath must not overlap."
    }
}

if ($DryRun) {
    Write-BackupStatus "dry-run destination=$destinationFull"
    if ($redisRequested) {
        $mode = if ([string]::IsNullOrWhiteSpace($RedisContainer)) { "redis-cli URL" } else { "docker exec" }
        Write-BackupStatus "dry-run Redis selected mode=$mode namespaceLength=$($RedisNamespace.Length)"
    }
    if ($postgresRequested) {
        $mode = if ([string]::IsNullOrWhiteSpace($PostgresContainer)) { "pg_dump executable" } else { "docker exec" }
        Write-BackupStatus "dry-run PostgreSQL selected mode=$mode"
    }
    if ($objectStorageRequested) {
        Write-BackupStatus "dry-run local object storage selected"
    }
    return
}

$destinationParent = Split-Path -Parent $destinationFull
if ([string]::IsNullOrWhiteSpace($destinationParent)) {
    throw "DestinationPath must have a parent directory."
}
New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
$stagingPath = Resolve-FullPath -Path (
    "$destinationFull.partial-$([guid]::NewGuid().ToString('N'))"
)
Assert-PathUnderRoot -Root $destinationParent -Path $stagingPath
New-Item -ItemType Directory -Path $stagingPath | Out-Null

$completed = $false
try {
    $redisManifest = [ordered]@{
        included = $false
        sourceMode = $null
        database = $null
        namespace = $null
        keyCount = 0
        entries = @()
    }
    if ($redisRequested) {
        $redisRoot = Join-Path $stagingPath "redis"
        New-Item -ItemType Directory -Path $redisRoot -Force | Out-Null
        # redis-cli SCAN is intentionally constrained to the literal namespace prefix.
        $scan = Invoke-RedisText `
            -Arguments @("--scan", "--pattern", "$RedisNamespace*") `
            -Operation "Redis SCAN"
        $keys = @(
            $scan -split "`r?`n" |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
                Sort-Object -Unique
        )
        $entries = [System.Collections.Generic.List[object]]::new()
        $index = 0
        foreach ($key in $keys) {
            if (-not $key.StartsWith($RedisNamespace, [System.StringComparison]::Ordinal)) {
                throw "Redis SCAN returned a key outside the requested namespace."
            }
            $ttlText = Invoke-RedisText `
                -Arguments @("--raw", "PTTL", $key) `
                -Operation "Redis PTTL"
            $ttlMs = 0L
            if (-not [long]::TryParse($ttlText, [ref]$ttlMs) -or $ttlMs -eq -2) {
                throw "Redis key changed while its TTL was being captured."
            }
            $dump = Get-RedisDumpBytes -Key $key
            $payloadName = "{0:D8}.dump" -f $index
            $payloadPath = Join-Path $redisRoot $payloadName
            [System.IO.File]::WriteAllBytes($payloadPath, $dump)
            $suffix = $key.Substring($RedisNamespace.Length)
            $entries.Add([ordered]@{
                keySuffixBase64 = [Convert]::ToBase64String(
                    [System.Text.Encoding]::UTF8.GetBytes($suffix)
                )
                ttlMs = $ttlMs
                payload = "redis/$payloadName"
                bytes = [int64]$dump.Length
                sha256 = (Get-FileHash -LiteralPath $payloadPath -Algorithm SHA256).Hash.ToLowerInvariant()
            }) | Out-Null
            $index += 1
        }
        $redisManifest = [ordered]@{
            included = $true
            sourceMode = if ([string]::IsNullOrWhiteSpace($RedisContainer)) { "url" } else { "container" }
            database = if ([string]::IsNullOrWhiteSpace($RedisContainer)) { $null } else { $RedisDatabase }
            namespace = $RedisNamespace
            keyCount = $entries.Count
            entries = @($entries)
        }
        Write-BackupStatus "captured Redis namespace keys=$($entries.Count)"
    }

    $postgresManifest = [ordered]@{
        included = $false
        format = $null
        payload = $null
        bytes = 0
    }
    if ($postgresRequested) {
        $postgresRoot = Join-Path $stagingPath "postgresql"
        New-Item -ItemType Directory -Path $postgresRoot -Force | Out-Null
        $postgresPayload = Join-Path $postgresRoot "gateway.sql"
        $pgDumpInvocation = Get-PostgresInvocation `
            -ClientCommand "pg_dump" `
            -ClientPath $PgDumpPath `
            -CommandArguments @(
                "--dbname=$PostgresConnection",
                "--format=plain",
                "--no-owner",
                "--no-privileges"
            )
        Invoke-NativeToFile `
            -FilePath $pgDumpInvocation.FilePath `
            -Arguments $pgDumpInvocation.Arguments `
            -Operation "pg_dump" `
            -OutputPath $postgresPayload
        if (-not (Test-Path -LiteralPath $postgresPayload -PathType Leaf)) {
            throw "pg_dump completed without producing the expected payload."
        }
        $postgresItem = Get-Item -LiteralPath $postgresPayload
        $postgresManifest = [ordered]@{
            included = $true
            format = "plain-sql"
            payload = "postgresql/gateway.sql"
            bytes = [int64]$postgresItem.Length
        }
        Write-BackupStatus "captured PostgreSQL plain SQL dump"
    }

    $objectStorageManifest = [ordered]@{
        included = $false
        format = $null
        path = $null
        fileCount = 0
        bytes = 0
    }
    if ($objectStorageRequested) {
        $objectStorageDestination = Join-Path $stagingPath "object-storage"
        Copy-LocalObjectTree `
            -Source $objectStorageFull `
            -Destination $objectStorageDestination
        $objectFiles = @(Get-ChildItem -LiteralPath $objectStorageDestination -Recurse -File)
        $objectBytes = ($objectFiles | Measure-Object -Property Length -Sum).Sum
        if ($null -eq $objectBytes) {
            $objectBytes = 0
        }
        $objectStorageManifest = [ordered]@{
            included = $true
            format = "local-directory"
            path = "object-storage"
            fileCount = $objectFiles.Count
            bytes = [int64]$objectBytes
        }
        Write-BackupStatus "captured local object storage files=$($objectFiles.Count)"
    }

    $manifest = [ordered]@{
        schemaVersion = 1
        kind = "gateway-state-backup"
        createdAt = [DateTimeOffset]::UtcNow.ToString("o")
        components = [ordered]@{
            redis = $redisManifest
            postgresql = $postgresManifest
            objectStorage = $objectStorageManifest
        }
    }
    Write-Utf8NoBom `
        -Path (Join-Path $stagingPath "manifest.json") `
        -Value (($manifest | ConvertTo-Json -Depth 12) + [Environment]::NewLine)
    Write-ChecksumManifest -BackupRoot $stagingPath

    Move-Item -LiteralPath $stagingPath -Destination $destinationFull
    $completed = $true
    Write-BackupStatus "completed destination=$destinationFull"
} finally {
    if (-not $completed -and (Test-Path -LiteralPath $stagingPath)) {
        Assert-PathUnderRoot -Root $destinationParent -Path $stagingPath
        Remove-Item -LiteralPath $stagingPath -Recurse -Force
    }
}
