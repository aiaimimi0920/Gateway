# Publication ownership spans artifact recovery, writes and owner-checked cleanup.
# Keep the named mutex held until the matching lock record has been removed.

function Read-PublicationLockStreamText {
    param([Parameter(Mandatory = $true)][System.IO.FileStream]$Stream)

    $originalPosition = $Stream.Position
    try {
        $Stream.Position = 0
        $bytes = New-Object byte[] ([int]$Stream.Length)
        $offset = 0
        while ($offset -lt $bytes.Length) {
            $read = $Stream.Read($bytes, $offset, $bytes.Length - $offset)
            if ($read -le 0) {
                throw "Publication lock owner record ended unexpectedly."
            }
            $offset += $read
        }
        return [System.Text.UTF8Encoding]::new($false).GetString($bytes)
    }
    finally {
        $Stream.Position = $originalPosition
    }
}

function Read-PublicationLockOwner {
    param([Parameter(Mandatory = $true)][string]$LockPath)

    $reader = $null
    try {
        $reader = [System.IO.File]::Open(
            $LockPath,
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::Read,
            ([System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
        )
        return Read-PublicationLockStreamText -Stream $reader
    }
    catch {
        return "<owner unavailable while publication lock is active>"
    }
    finally {
        if ($null -ne $reader) {
            $reader.Dispose()
        }
    }
}

function Get-PublicationMutexName {
    param([Parameter(Mandatory = $true)][string]$LockPath)

    $normalizedPath = [System.IO.Path]::GetFullPath($LockPath).ToLowerInvariant()
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = $sha256.ComputeHash(
            [System.Text.UTF8Encoding]::new($false).GetBytes($normalizedPath)
        )
    }
    finally {
        $sha256.Dispose()
    }
    $hex = [System.BitConverter]::ToString($digest).Replace("-", "").ToLowerInvariant()
    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        return "Global\GatewayReleasePublication-$hex"
    }
    return "GatewayReleasePublication-$hex"
}

function Enter-PublicationLock {
    param(
        [Parameter(Mandatory = $true)][string]$LockPath,
        [Parameter(Mandatory = $true)][string]$ArtifactName
    )

    $timeoutSeconds = 600
    $configuredTimeout = [Environment]::GetEnvironmentVariable(
        "GATEWAY_RELEASE_PUBLICATION_LOCK_TIMEOUT_SECONDS"
    )
    if (-not [string]::IsNullOrWhiteSpace($configuredTimeout)) {
        $parsedTimeout = 0
        if (-not [int]::TryParse($configuredTimeout, [ref]$parsedTimeout) -or $parsedTimeout -le 0) {
            throw "GATEWAY_RELEASE_PUBLICATION_LOCK_TIMEOUT_SECONDS must be a positive integer."
        }
        $timeoutSeconds = $parsedTimeout
    }

    $mutexName = Get-PublicationMutexName -LockPath $LockPath
    $mutex = [System.Threading.Mutex]::new($false, $mutexName)
    $mutexHeld = $false
    $stream = $null
    try {
        try {
            $mutexHeld = $mutex.WaitOne([int]($timeoutSeconds * 1000))
        }
        catch [System.Threading.AbandonedMutexException] {
            $mutexHeld = $true
        }
        if (-not $mutexHeld) {
            $owner = Read-PublicationLockOwner -LockPath $LockPath
            throw "Timed out waiting for publication lock: $LockPath owner=$owner"
        }

        $ownerToken = [guid]::NewGuid().ToString("N")
        $startedUtc = [DateTime]::UtcNow.ToString("o")
        $ownerRecord = [ordered]@{
            schemaVersion = 1
            ownerToken = $ownerToken
            processId = $PID
            machineName = [Environment]::MachineName
            startedUtc = $startedUtc
            artifactName = $ArtifactName
            state = "active"
        } | ConvertTo-Json -Compress
        $ownerRecord += [Environment]::NewLine
        $releaseRecord = [ordered]@{
            schemaVersion = 1
            ownerToken = $ownerToken
            processId = $PID
            machineName = [Environment]::MachineName
            startedUtc = $startedUtc
            artifactName = $ArtifactName
            state = "releasing"
        } | ConvertTo-Json -Compress
        $releaseRecord += [Environment]::NewLine

        $stream = [System.IO.File]::Open(
            $LockPath,
            [System.IO.FileMode]::Create,
            [System.IO.FileAccess]::ReadWrite,
            [System.IO.FileShare]::Read
        )
        $ownerBytes = [System.Text.UTF8Encoding]::new($false).GetBytes($ownerRecord)
        $stream.Write($ownerBytes, 0, $ownerBytes.Length)
        $stream.Flush($true)

        return [pscustomobject]@{
            Path = $LockPath
            ArtifactName = $ArtifactName
            OwnerToken = $ownerToken
            OwnerRecord = $ownerRecord
            ReleaseRecord = $releaseRecord
            Stream = $stream
            Mutex = $mutex
            MutexName = $mutexName
        }
    }
    catch {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
        if ($mutexHeld) {
            $mutex.ReleaseMutex()
        }
        $mutex.Dispose()
        throw
    }
}

function Exit-PublicationLock {
    param([Parameter(Mandatory = $true)]$Lock)

    $stream = $Lock.Stream
    $streamDisposed = $false
    try {
        $currentOwner = Read-PublicationLockStreamText -Stream $stream
        if ($currentOwner -cne $Lock.OwnerRecord) {
            throw "Refusing to remove a publication lock whose owner record changed: $($Lock.Path)"
        }

        $releaseBytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Lock.ReleaseRecord)
        $stream.SetLength(0)
        $stream.Position = 0
        $stream.Write($releaseBytes, 0, $releaseBytes.Length)
        $stream.Flush($true)
        $stream.Dispose()
        $streamDisposed = $true

        if (-not (Test-Path -LiteralPath $Lock.Path -PathType Leaf)) {
            throw "Publication lock owner record disappeared before cleanup: $($Lock.Path)"
        }
        $persistedOwner = [System.IO.File]::ReadAllText(
            $Lock.Path,
            [System.Text.UTF8Encoding]::new($false)
        )
        if ($persistedOwner -cne $Lock.ReleaseRecord) {
            throw "Refusing to delete a replacement publication lock owner record: $($Lock.Path)"
        }
        [System.IO.File]::Delete($Lock.Path)
    }
    finally {
        if (-not $streamDisposed) {
            $stream.Dispose()
        }
        try {
            $Lock.Mutex.ReleaseMutex()
        }
        finally {
            $Lock.Mutex.Dispose()
        }
    }
}
