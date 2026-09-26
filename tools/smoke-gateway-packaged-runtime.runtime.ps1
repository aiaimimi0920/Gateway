# Native command, disposable Redis, and HTTP transport owners for packaged smoke.

function Get-EvidenceRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseFull = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Get-FreeTcpPort {
    $listener = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )
    $listener.Start()
    try {
        return ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    } finally {
        $listener.Stop()
    }
}

function Resolve-NativeExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if (Test-Path -LiteralPath $Command -PathType Leaf) {
        return [System.IO.Path]::GetFullPath($Command)
    }
    $resolved = @(Get-Command $Command -All -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandType -eq "Application" } |
        Select-Object -First 1)
    if ($resolved.Count -eq 0) {
        $resolved = @(Get-Command $Command -ErrorAction SilentlyContinue | Select-Object -First 1)
    }
    if ($resolved.Count -eq 0 -or $null -eq $resolved[0]) {
        throw "$Name executable was not found. Supply an explicit command path."
    }
    return $resolved[0].Source
}

function Invoke-NativeCommandCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $captureRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        "gateway-packaged-smoke-{0}" -f [guid]::NewGuid().ToString("N")
    )
    $stdoutPath = Join-Path $captureRoot "stdout.log"
    $stderrPath = Join-Path $captureRoot "stderr.log"
    New-Item -ItemType Directory -Path $captureRoot -Force | Out-Null

    try {
        $startInfo = @{
            FilePath = $Command
            ArgumentList = $Arguments
            RedirectStandardOutput = $stdoutPath
            RedirectStandardError = $stderrPath
            PassThru = $true
            Wait = $true
            WindowStyle = "Hidden"
        }
        $process = Start-Process @startInfo
        $stdoutLines = if (Test-Path -LiteralPath $stdoutPath -PathType Leaf) {
            @(Get-Content -LiteralPath $stdoutPath -Encoding UTF8)
        } else {
            @()
        }
        $stderrLines = if (Test-Path -LiteralPath $stderrPath -PathType Leaf) {
            @(Get-Content -LiteralPath $stderrPath -Encoding UTF8)
        } else {
            @()
        }

        return [pscustomobject]@{
            ExitCode = [int]$process.ExitCode
            Text = (($stdoutLines + $stderrLines) | ForEach-Object { [string]$_ }) -join "`n"
        }
    } finally {
        if (Test-Path -LiteralPath $captureRoot) {
            Remove-Item -LiteralPath $captureRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Invoke-Docker {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation,
        [switch]$AllowFailure
    )

    $result = Invoke-NativeCommandCapture -Command $DockerExecutable -Arguments $Arguments
    if ($result.ExitCode -ne 0 -and -not $AllowFailure) {
        $details = $result.Text.Trim()
        if ([string]::IsNullOrWhiteSpace($details)) {
            throw "$Operation failed with exit code $($result.ExitCode)."
        }
        throw "$Operation failed with exit code $($result.ExitCode): $details"
    }
    return [pscustomobject]@{
        ExitCode = $result.ExitCode
        Text = $result.Text.Trim()
    }
}

function Stop-TemporaryRedis {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string]$ContainerName
    )

    $removed = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("rm", "--force", "--volumes", $ContainerName) `
        -Operation "remove disposable Redis container" `
        -AllowFailure
    if ($removed.ExitCode -eq 0) {
        return $true
    }

    $inspect = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("inspect", $ContainerName) `
        -Operation "inspect disposable Redis container" `
        -AllowFailure
    return $inspect.ExitCode -ne 0
}

function Start-TemporaryRedis {
    param(
        [Parameter(Mandatory = $true)][string]$DockerExecutable,
        [Parameter(Mandatory = $true)][string]$Image,
        [Parameter(Mandatory = $true)][int]$Database,
        [Parameter(Mandatory = $true)][int]$StartupTimeoutSeconds
    )

    $runId = [guid]::NewGuid().ToString("N")
    $containerName = "gateway-packaged-smoke-redis-$($runId.Substring(0, 12))"
    $containerStarted = $false
    try {
        $run = Invoke-Docker `
            -DockerExecutable $DockerExecutable `
            -Arguments @(
                "run",
                "--detach",
                "--name",
                $containerName,
                "--publish",
                "127.0.0.1::6379",
                "--label",
                "neuro.gateway.packaged-smoke=$runId",
                $Image
            ) `
            -Operation "start disposable Redis container"
        if ([string]::IsNullOrWhiteSpace($run.Text)) {
            throw "docker run did not return a disposable Redis container id."
        }
        $containerStarted = $true

        $portDeadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
        $hostPort = 0
        while ([DateTimeOffset]::UtcNow -lt $portDeadline) {
            $port = Invoke-Docker `
                -DockerExecutable $DockerExecutable `
                -Arguments @("port", $containerName, "6379/tcp") `
                -Operation "resolve disposable Redis host port" `
                -AllowFailure
            if ($port.ExitCode -eq 0 -and $port.Text -match ":(?<port>[0-9]+)\s*$") {
                $hostPort = [int]$Matches["port"]
                if ($hostPort -gt 0) {
                    break
                }
            }
            Start-Sleep -Milliseconds 200
        }
        if ($hostPort -le 0) {
            throw "Disposable Redis did not publish a host port before the timeout."
        }

        $pingDeadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
        while ([DateTimeOffset]::UtcNow -lt $pingDeadline) {
            $ping = Invoke-Docker `
                -DockerExecutable $DockerExecutable `
                -Arguments @(
                    "exec",
                    $containerName,
                    "redis-cli",
                    "-n",
                    [string]$Database,
                    "PING"
                ) `
                -Operation "ping disposable Redis" `
                -AllowFailure
            if ($ping.ExitCode -eq 0 -and $ping.Text.Trim() -eq "PONG") {
                return [pscustomobject]@{
                    ContainerName = $containerName
                    HostPort = $hostPort
                    Url = "redis://127.0.0.1:$hostPort/$Database"
                    Image = $Image
                }
            }
            Start-Sleep -Milliseconds 200
        }
        throw "Disposable Redis did not become ready before the timeout."
    } catch {
        if ($containerStarted) {
            $null = Stop-TemporaryRedis `
                -DockerExecutable $DockerExecutable `
                -ContainerName $containerName
        }
        throw
    }
}

function Invoke-SmokeHttpRequest {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Uri,
        [hashtable]$Headers = @{},
        [string]$Body = "",
        [string]$ContentType = "application/json",
        [ValidateRange(1, 10)][int]$RetryCount = 1
    )

    $arguments = @{
        Method = $Method
        Uri = $Uri
        Headers = $Headers
        UseBasicParsing = $true
        TimeoutSec = 5
    }
    if (-not [string]::IsNullOrEmpty($Body)) {
        $arguments.Body = $Body
        $arguments.ContentType = $ContentType
    }

    for ($attempt = 1; $attempt -le $RetryCount; $attempt++) {
        try {
            $response = Invoke-WebRequest @arguments
            return [pscustomobject]@{
                StatusCode = [int]$response.StatusCode
                Content = [string]$response.Content
                Headers = $response.Headers
                TransportError = $false
            }
        } catch {
            $statusCode = 0
            $headers = @{}
            $responseProperty = $_.Exception.PSObject.Properties['Response']
            if ($null -ne $responseProperty -and $null -ne $responseProperty.Value) {
                try { $statusCode = [int]$responseProperty.Value.StatusCode } catch { $statusCode = 0 }
                try { $headers = $responseProperty.Value.Headers } catch { $headers = @{} }
            }
            $errorDetailsProperty = $_.PSObject.Properties['ErrorDetails']
            $errorDetails = if ($null -ne $errorDetailsProperty) { $errorDetailsProperty.Value } else { $null }
            $errorDetailsMessage = $null
            if ($null -ne $errorDetails) {
                $messageProperty = $errorDetails.PSObject.Properties['Message']
                if ($null -ne $messageProperty) {
                    $errorDetailsMessage = [string]$messageProperty.Value
                }
            }
            $content = if (-not [string]::IsNullOrWhiteSpace($errorDetailsMessage)) {
                $errorDetailsMessage
            } else {
                $_.Exception.Message
            }
            $result = [pscustomobject]@{
                StatusCode = $statusCode
                Content = [string]$content
                Headers = $headers
                TransportError = ($statusCode -eq 0)
            }
            if ($result.TransportError -and $attempt -lt $RetryCount) {
                Start-Sleep -Milliseconds ([Math]::Min(500, 100 * $attempt))
                continue
            }
            return $result
        }
    }
    throw "HTTP smoke request did not produce a result"
}

function Assert-SmokeStatus {
    param(
        [string]$Name,
        [int]$Expected,
        [object]$Response
    )

    if ($Response.StatusCode -ne $Expected) {
        throw "$Name expected HTTP $Expected but received $($Response.StatusCode): $($Response.Content)"
    }
    Write-Smoke "$Name HTTP $Expected"
}

function Test-HeaderExists {
    param([object]$Headers, [string]$Name)

    if ($null -eq $Headers) {
        return $false
    }
    foreach ($key in $Headers.Keys) {
        if ([string]::Equals([string]$key, $Name, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $true
        }
    }
    return $false
}
