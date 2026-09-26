function Invoke-Docker {
  param(
    [Parameter(Mandatory = $true)][string] $DockerExecutable,
    [Parameter(Mandatory = $true)][string[]] $Arguments,
    [Parameter(Mandatory = $true)][string] $Operation,
    [switch] $AllowFailure
  )

  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    $output = & $DockerExecutable @Arguments 2>&1
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  if ($exitCode -ne 0 -and -not $AllowFailure) {
    $details = (($output | ForEach-Object { [string]$_ }) -join "`n").Trim()
    if ([string]::IsNullOrWhiteSpace($details)) {
      throw "$Operation failed with exit code $exitCode."
    }
    throw "$Operation failed with exit code ${exitCode}: $details"
  }

  return [pscustomobject]@{
    ExitCode = $exitCode
    Text = (($output | ForEach-Object { [string]$_ }) -join "`n").Trim()
  }
}

function Stop-TemporaryRedis {
  param(
    [Parameter(Mandatory = $true)][string] $DockerExecutable,
    [Parameter(Mandatory = $true)][string] $ContainerName
  )

  $null = Invoke-Docker `
    -DockerExecutable $DockerExecutable `
    -Arguments @("rm", "--force", $ContainerName) `
    -Operation "remove disposable Redis container" `
    -AllowFailure
}

function Read-RedisLine {
  param([Parameter(Mandatory = $true)][System.IO.Stream] $Stream)

  $buffer = New-Object System.Collections.Generic.List[byte]
  while ($true) {
    $value = $Stream.ReadByte()
    if ($value -lt 0) {
      throw "Redis connection closed while reading a RESP line."
    }
    if ($value -eq 13) {
      $lineFeed = $Stream.ReadByte()
      if ($lineFeed -ne 10) {
        throw "Redis RESP line did not terminate with LF."
      }
      return [System.Text.Encoding]::UTF8.GetString($buffer.ToArray())
    }
    $buffer.Add([byte]$value)
  }
}

function Invoke-RedisCommand {
  param(
    [Parameter(Mandatory = $true)][int] $Port,
    [Parameter(Mandatory = $true)][string[]] $Arguments
  )

  $client = [System.Net.Sockets.TcpClient]::new()
  try {
    $client.Connect("127.0.0.1", $Port)
    $stream = $client.GetStream()
    $stream.ReadTimeout = 5000
    $stream.WriteTimeout = 5000

    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append("*$($Arguments.Count)`r`n")
    foreach ($argument in $Arguments) {
      $argumentValue = [string]$argument
      $argumentByteCount = [System.Text.Encoding]::UTF8.GetByteCount($argumentValue)
      [void]$builder.Append("$" + $argumentByteCount + "`r`n")
      [void]$builder.Append($argumentValue)
      [void]$builder.Append("`r`n")
    }

    $payload = [System.Text.Encoding]::UTF8.GetBytes($builder.ToString())
    $stream.Write($payload, 0, $payload.Length)
    $stream.Flush()

    $firstLine = Read-RedisLine -Stream $stream
    if ([string]::IsNullOrEmpty($firstLine)) {
      throw "Redis returned an empty RESP line."
    }

    switch ($firstLine[0]) {
      '+' { return $firstLine.Substring(1) }
      '-' { throw "Redis command failed: $($firstLine.Substring(1))" }
      ':' { return [int64]$firstLine.Substring(1) }
      '$' {
        $length = [int]$firstLine.Substring(1)
        if ($length -lt 0) {
          return $null
        }
        $buffer = New-Object byte[] ($length + 2)
        $offset = 0
        while ($offset -lt $buffer.Length) {
          $read = $stream.Read($buffer, $offset, $buffer.Length - $offset)
          if ($read -le 0) {
            throw "Redis connection closed while reading a bulk string."
          }
          $offset += $read
        }
        return [System.Text.Encoding]::UTF8.GetString($buffer, 0, $length)
      }
      default {
        throw "Unsupported Redis RESP reply: $firstLine"
      }
    }
  } finally {
    $client.Dispose()
  }
}

function Set-RedisString {
  param(
    [Parameter(Mandatory = $true)][int] $Port,
    [Parameter(Mandatory = $true)][string] $Key,
    [Parameter(Mandatory = $true)][string] $Value
  )

  $result = Invoke-RedisCommand -Port $Port -Arguments @("SET", $Key, $Value)
  if ([string]$result -ne "OK") {
    throw "Redis seed for '$Key' did not return OK: $result"
  }
}

function Get-RedisString {
  param(
    [Parameter(Mandatory = $true)][int] $Port,
    [Parameter(Mandatory = $true)][string] $Key
  )

  return Invoke-RedisCommand -Port $Port -Arguments @("GET", $Key)
}

function Start-TemporaryRedis {
  param(
    [Parameter(Mandatory = $true)][string] $DockerExecutable,
    [Parameter(Mandatory = $true)][int] $StartupTimeoutSeconds
  )

  $containerName = "gateway-console-live-e2e-$([guid]::NewGuid().ToString('N').Substring(0, 12))"
  $containerStarted = $false
  try {
    $run = Invoke-Docker `
      -DockerExecutable $DockerExecutable `
      -Arguments @(
        "run",
        "--detach",
        "--rm",
        "--name",
        $containerName,
        "--publish",
        "127.0.0.1::6379",
        $RedisImage
      ) `
      -Operation "start disposable Redis container"
    if ([string]::IsNullOrWhiteSpace($run.Text)) {
      throw "docker run did not return a container id."
    }
    $containerStarted = $true

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $hostPort = 0
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
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

    while ([DateTimeOffset]::UtcNow -lt $deadline) {
      $ping = Invoke-Docker `
        -DockerExecutable $DockerExecutable `
        -Arguments @("exec", $containerName, "redis-cli", "PING") `
        -Operation "ping disposable Redis" `
        -AllowFailure
      if ($ping.ExitCode -eq 0 -and $ping.Text.Trim() -eq "PONG") {
        return [pscustomobject]@{
          ContainerName = $containerName
          HostPort = $hostPort
          Url = "redis://127.0.0.1:$hostPort/0"
        }
      }
      Start-Sleep -Milliseconds 200
    }

    throw "Disposable Redis did not become ready before the timeout."
  } catch {
    if ($containerStarted) {
      Stop-TemporaryRedis -DockerExecutable $DockerExecutable -ContainerName $containerName
    }
    throw
  }
}
