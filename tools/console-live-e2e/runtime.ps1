function Write-LiveLog {
  param([Parameter(Mandatory = $true)][string] $Message)
  Write-Host "[gateway-console-live-e2e] $Message"
}

function Ensure-Directory {
  param([Parameter(Mandatory = $true)][string] $Path)
  New-Item -ItemType Directory -Force -Path $Path | Out-Null
  return [System.IO.Path]::GetFullPath($Path)
}

function Resolve-Executable {
  param([Parameter(Mandatory = $true)][string] $Command)

  if (Test-Path -LiteralPath $Command -PathType Leaf) {
    return [System.IO.Path]::GetFullPath($Command)
  }

  $resolved = Get-Command $Command -ErrorAction Stop | Select-Object -First 1
  if ($null -eq $resolved) {
    throw "Command not found: $Command"
  }
  return $resolved.Source
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

function Invoke-LoggedCommand {
  param(
    [Parameter(Mandatory = $true)][string] $LogPath,
    [Parameter(Mandatory = $true)][string[]] $Command,
    [string] $WorkingDirectory = $RepoRoot
  )

  Write-LiveLog ("running: " + ($Command -join " "))
  $stdoutCapture = Join-Path $env:TEMP ("gateway-live-e2e-stdout-" + [guid]::NewGuid().ToString("N") + ".log")
  $stderrCapture = Join-Path $env:TEMP ("gateway-live-e2e-stderr-" + [guid]::NewGuid().ToString("N") + ".log")
  try {
    $filePath = Resolve-Executable -Command $Command[0]
    $process = Start-Process `
      -FilePath $filePath `
      -ArgumentList @($Command | Select-Object -Skip 1) `
      -WorkingDirectory $WorkingDirectory `
      -RedirectStandardOutput $stdoutCapture `
      -RedirectStandardError $stderrCapture `
      -NoNewWindow `
      -PassThru `
      -Wait
    $exitCode = $process.ExitCode

    foreach ($capturePath in @($stdoutCapture, $stderrCapture)) {
      if (Test-Path -LiteralPath $capturePath) {
        Get-Content -LiteralPath $capturePath -Encoding UTF8 | Tee-Object -FilePath $LogPath -Append
      }
    }
  } finally {
    foreach ($capturePath in @($stdoutCapture, $stderrCapture)) {
      if (Test-Path -LiteralPath $capturePath) {
        Remove-Item -LiteralPath $capturePath -Force -ErrorAction SilentlyContinue
      }
    }
  }
  if ($exitCode -ne 0) {
    throw "Command failed with exit code ${exitCode}: $($Command -join ' ')"
  }
}

function Invoke-SmokeHttpRequest {
  param(
    [Parameter(Mandatory = $true)][string] $Method,
    [Parameter(Mandatory = $true)][string] $Uri,
    [hashtable] $Headers = @{},
    [string] $Body = "",
    [string] $ContentType = "application/json",
    [ValidateRange(1, 10)][int] $RetryCount = 1
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
      }
    } catch {
      $statusCode = 0
      $responseProperty = $_.Exception.PSObject.Properties["Response"]
      if ($null -ne $responseProperty -and $null -ne $responseProperty.Value) {
        try {
          $statusCode = [int]$responseProperty.Value.StatusCode
        } catch {
          $statusCode = 0
        }
      }
      if ($statusCode -eq 0 -and $attempt -lt $RetryCount) {
        Start-Sleep -Milliseconds 250
        continue
      }
      return [pscustomobject]@{
        StatusCode = $statusCode
        Content = $_.Exception.Message
        Headers = @{}
      }
    }
  }

  throw "HTTP smoke request did not produce a result."
}

function Start-LiveGatewayProcess {
  param(
    [Parameter(Mandatory = $true)][string] $BinaryPath,
    [Parameter(Mandatory = $true)][string] $WorkingDirectory,
    [Parameter(Mandatory = $true)][string] $StdoutPath,
    [Parameter(Mandatory = $true)][string] $StderrPath
  )

  return Start-Process `
    -FilePath $BinaryPath `
    -WorkingDirectory $WorkingDirectory `
    -RedirectStandardOutput $StdoutPath `
    -RedirectStandardError $StderrPath `
    -PassThru `
    -WindowStyle Hidden
}

function Wait-ForGatewayReady {
  param(
    [Parameter(Mandatory = $true)] $Process,
    [Parameter(Mandatory = $true)][string] $BaseUrl,
    [Parameter(Mandatory = $true)][string] $StderrPath,
    [ValidateRange(1, 120)][int] $TimeoutSeconds = 30
  )

  $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
  $health = $null
  while ([DateTimeOffset]::UtcNow -lt $deadline) {
    if ($Process.HasExited) {
      $stderrTail = if (Test-Path -LiteralPath $StderrPath) {
        (Get-Content -LiteralPath $StderrPath -Tail 50 -Encoding UTF8) -join "`n"
      } else {
        ""
      }
      throw "Gateway exited before readiness: $stderrTail"
    }

    $health = Invoke-SmokeHttpRequest -Method "GET" -Uri "$BaseUrl/healthz" -RetryCount 3
    if ($health.StatusCode -eq 200) {
      break
    }
    Start-Sleep -Milliseconds 250
  }
  Assert-StatusCode -Name "/healthz" -Expected 200 -Response $health
}

function Assert-StatusCode {
  param(
    [Parameter(Mandatory = $true)][string] $Name,
    [Parameter(Mandatory = $true)][int] $Expected,
    [Parameter(Mandatory = $true)] $Response
  )

  if ($Response.StatusCode -ne $Expected) {
    throw "$Name expected HTTP $Expected but received $($Response.StatusCode): $($Response.Content)"
  }
}
