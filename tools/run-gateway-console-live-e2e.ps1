[CmdletBinding()]
param(
  [ValidateSet("desktop-chromium", "mobile-chromium")]
  [string] $Project = "desktop-chromium",

  [switch] $InstallBrowsers,

  [switch] $AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$UiRoot = Join-Path $RepoRoot "apps\desktop"
$RuntimeRoot = Join-Path $RepoRoot ".runtime\console-live-e2e"
$RunId = Get-Date -Format "yyyyMMdd-HHmmss"
$SessionRoot = Join-Path $RuntimeRoot $RunId
$LogsRoot = Join-Path $SessionRoot "logs"
$GatewayStateRoot = Join-Path $SessionRoot "gateway-state"
$RoutesPath = Join-Path $SessionRoot "routes.yaml"
$ResultsRoot = Join-Path $SessionRoot "test-results"
$ArtifactsRoot = Join-Path $SessionRoot "artifacts"
$GatewayBinary = Join-Path $RepoRoot "target\debug\neuro-gateway.exe"
$LiveSpecPath = "e2e/console.live.spec.ts"
$RedisImage = "redis:7-alpine"
$RedisNamespace = "default"

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
function Get-Utf8Sha256Hex {
  param([Parameter(Mandatory = $true)][string] $Text)

  $sha256 = [System.Security.Cryptography.SHA256]::Create()
  try {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Text)
    return [System.BitConverter]::ToString($sha256.ComputeHash($bytes)).Replace("-", "").ToLowerInvariant()
  } finally {
    $sha256.Dispose()
  }
}
function ConvertTo-JsonLiteral {
  param($Value)

  if ($null -eq $Value) {
    return "null"
  }
  return $Value | ConvertTo-Json -Depth 20 -Compress
}

function New-SeedRevisionMetadataJson {
  param([Parameter(Mandatory = $true)] $RevisionMetadata)

  $id = ConvertTo-JsonLiteral ([string]$RevisionMetadata.id)
  $sequence = ConvertTo-JsonLiteral ([int64]$RevisionMetadata.sequence)
  $parent = ConvertTo-JsonLiteral $RevisionMetadata.parent
  $actor = ConvertTo-JsonLiteral ([string]$RevisionMetadata.actor)
  $timestamp = ConvertTo-JsonLiteral ([string]$RevisionMetadata.timestamp)
  $documentDigest = ConvertTo-JsonLiteral ([string]$RevisionMetadata.documentDigest)
  $yamlDigest = ConvertTo-JsonLiteral ([string]$RevisionMetadata.yamlDigest)
  $message = ConvertTo-JsonLiteral $RevisionMetadata.message
  return "{`"id`":$id,`"sequence`":$sequence,`"parent`":$parent,`"actor`":$actor,`"timestamp`":$timestamp,`"documentDigest`":$documentDigest,`"yamlDigest`":$yamlDigest,`"message`":$message}"
}

function Install-SeedRouteRevisionArchive {
  param(
    [Parameter(Mandatory = $true)][string] $StateRoot,
    [Parameter(Mandatory = $true)][string] $RevisionId,
    [Parameter(Mandatory = $true)] $RevisionMetadata,
    [Parameter(Mandatory = $true)][string] $CanonicalDocumentJson,
    [Parameter(Mandatory = $true)][string] $CanonicalRoutesYaml
  )

  $metadataDocumentDigest = [string]$RevisionMetadata.documentDigest
  $metadataYamlDigest = [string]$RevisionMetadata.yamlDigest
  $documentDigest = Get-Utf8Sha256Hex -Text $CanonicalDocumentJson
  $yamlDigest = Get-Utf8Sha256Hex -Text $CanonicalRoutesYaml
  if ($metadataDocumentDigest -ne $documentDigest) {
    throw "Seed revision document digest mismatch: metadata=$metadataDocumentDigest generated=$documentDigest"
  }
  if ($metadataYamlDigest -ne $yamlDigest) {
    throw "Seed revision YAML digest mismatch: metadata=$metadataYamlDigest generated=$yamlDigest"
  }

  $archiveRoot = Join-Path $StateRoot "console\revisions\$RevisionId"
  $null = Ensure-Directory -Path $archiveRoot
  $metadataJson = New-SeedRevisionMetadataJson -RevisionMetadata $RevisionMetadata
  [System.IO.File]::WriteAllText(
    (Join-Path $archiveRoot "document.json"),
    $CanonicalDocumentJson,
    [System.Text.UTF8Encoding]::new($false)
  )
  [System.IO.File]::WriteAllText(
    (Join-Path $archiveRoot "routes.yaml"),
    $CanonicalRoutesYaml,
    [System.Text.UTF8Encoding]::new($false)
  )
  [System.IO.File]::WriteAllText(
    (Join-Path $archiveRoot "metadata.json"),
    $metadataJson,
    [System.Text.UTF8Encoding]::new($false)
  )
  Write-LiveLog "installed seed revision archive revision=$RevisionId"
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

function Start-LiveOpenAiCompatibleUpstream {
  param(
    [Parameter(Mandatory = $true)][int] $Port,
    [Parameter(Mandatory = $true)][string] $ScriptPath,
    [Parameter(Mandatory = $true)][string] $RequestLogPath,
    [Parameter(Mandatory = $true)][string] $StdoutPath,
    [Parameter(Mandatory = $true)][string] $StderrPath
  )

  $scriptLines = @(
    'import http from "node:http";',
    'import fs from "node:fs";',
    '',
    'const port = Number.parseInt(process.argv[2] ?? "", 10);',
    'const requestLogPath = process.argv[3];',
    'if (!Number.isInteger(port) || port <= 0 || !requestLogPath) {',
    '  console.error("usage: node openai-compatible-upstream.mjs <port> <request-log-path>");',
    '  process.exit(2);',
    '}',
    '',
    'function sendJson(response, statusCode, payload) {',
    '  const body = typeof payload === "string" ? payload : JSON.stringify(payload);',
    '  response.writeHead(statusCode, {',
    '    "content-type": "application/json; charset=utf-8",',
    '    "content-length": Buffer.byteLength(body),',
    '  });',
    '  response.end(body);',
    '}',
    '',
    'function appendRequestLog(entry) {',
    '  fs.appendFileSync(requestLogPath, `${JSON.stringify(entry)}\n`, "utf8");',
    '}',
    '',
    'const server = http.createServer((request, response) => {',
    '  const host = request.headers.host ?? `127.0.0.1:${port}`;',
    '  const url = new URL(request.url ?? "/", `http://${host}`);',
    '',
    '  if (request.method === "GET" && url.pathname === "/healthz") {',
    '    sendJson(response, 200, { status: "ok" });',
    '    return;',
    '  }',
    '',
    '  if (request.method !== "POST" || url.pathname !== "/v1/chat/completions") {',
    '    sendJson(response, 404, { error: { message: "not found" } });',
    '    return;',
    '  }',
    '',
    '  let body = "";',
    '  request.setEncoding("utf8");',
    '  request.on("data", (chunk) => {',
    '    body += chunk;',
    '  });',
    '  request.on("error", (error) => {',
    '    sendJson(response, 500, { error: { message: error.message } });',
    '  });',
    '  request.on("end", () => {',
    '    let parsed;',
    '    try {',
    '      parsed = body.length > 0 ? JSON.parse(body) : {};',
    '    } catch (error) {',
    '      sendJson(response, 400, { error: { message: `invalid JSON: ${error.message}` } });',
    '      return;',
    '    }',
    '',
    '    const requestedModel = typeof parsed.model === "string" ? parsed.model.trim() : "";',
    '    const model = requestedModel.length > 0 ? requestedModel : "unknown-model";',
    '    appendRequestLog({',
    '      timestamp: new Date().toISOString(),',
    '      method: request.method,',
    '      path: url.pathname,',
    '      authorization: request.headers.authorization ?? null,',
    '      model,',
    '      body: parsed,',
    '    });',
    '',
    '    sendJson(response, 200, {',
    '      id: "chatcmpl-live-fixture",',
    '      object: "chat.completion",',
    '      created: 1720000000,',
    '      model,',
    '      choices: [',
    '        {',
    '          index: 0,',
    '          message: {',
    '            role: "assistant",',
    '            content: `live upstream ok: ${model}`,',
    '          },',
    '          finish_reason: "stop",',
    '        },',
    '      ],',
    '      usage: {',
    '        prompt_tokens: 1,',
    '        completion_tokens: 1,',
    '        total_tokens: 2,',
    '      },',
    '    });',
    '  });',
    '});',
    '',
    'server.listen(port, "127.0.0.1", () => {',
    '  console.log(JSON.stringify({ status: "ready", port }));',
    '});',
    '',
    'function shutdown() {',
    '  server.close(() => process.exit(0));',
    '}',
    '',
    'process.on("SIGTERM", shutdown);',
    'process.on("SIGINT", shutdown);'
  )
  [System.IO.File]::WriteAllText(
    $ScriptPath,
    ($scriptLines -join "`n") + "`n",
    [System.Text.UTF8Encoding]::new($false)
  )

  try {
    $nodeExecutable = Resolve-Executable -Command "node.exe"
  } catch {
    $nodeExecutable = Resolve-Executable -Command "node"
  }
  $process = Start-Process `
    -FilePath $nodeExecutable `
    -ArgumentList @($ScriptPath, [string]$Port, $RequestLogPath) `
    -WorkingDirectory (Split-Path -Parent $ScriptPath) `
    -RedirectStandardOutput $StdoutPath `
    -RedirectStandardError $StderrPath `
    -PassThru `
    -WindowStyle Hidden

  return [pscustomobject]@{
    Process = $process
    BaseUrl = "http://127.0.0.1:$Port"
    ScriptPath = $ScriptPath
    RequestLogPath = $RequestLogPath
    StdoutPath = $StdoutPath
    StderrPath = $StderrPath
  }
}

function Wait-ForLiveOpenAiCompatibleUpstream {
  param(
    [Parameter(Mandatory = $true)] $Process,
    [Parameter(Mandatory = $true)][string] $BaseUrl,
    [Parameter(Mandatory = $true)][string] $StderrPath,
    [ValidateRange(1, 60)][int] $TimeoutSeconds = 15
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
      throw "OpenAI-compatible fixture upstream exited before readiness: $stderrTail"
    }

    $health = Invoke-SmokeHttpRequest -Method "GET" -Uri "$BaseUrl/healthz" -RetryCount 3
    if ($health.StatusCode -eq 200) {
      break
    }
    Start-Sleep -Milliseconds 250
  }
  Assert-StatusCode -Name "fixture /healthz" -Expected 200 -Response $health
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

if (-not (Test-Path -LiteralPath $UiRoot -PathType Container)) {
  throw "UI workspace is missing: $UiRoot"
}

$null = Ensure-Directory -Path $RuntimeRoot
$null = Ensure-Directory -Path $SessionRoot
$null = Ensure-Directory -Path $LogsRoot
$null = Ensure-Directory -Path $GatewayStateRoot
$null = Ensure-Directory -Path $ResultsRoot
$null = Ensure-Directory -Path $ArtifactsRoot

$buildLog = Join-Path $LogsRoot "build.log"
$playwrightLog = Join-Path $LogsRoot "playwright.log"
$gatewayStdout = Join-Path $LogsRoot "gateway.stdout.log"
$gatewayStderr = Join-Path $LogsRoot "gateway.stderr.log"
$upstreamScript = Join-Path $SessionRoot "openai-compatible-upstream.mjs"
$upstreamRequests = Join-Path $LogsRoot "openai-compatible-upstream.requests.jsonl"
$upstreamStdout = Join-Path $LogsRoot "openai-compatible-upstream.stdout.log"
$upstreamStderr = Join-Path $LogsRoot "openai-compatible-upstream.stderr.log"

$SeedRouteDocument = [ordered]@{
  providers = @(
    [ordered]@{
      id = "managed-provider"
      preset = "openai"
      base_url = "https://api.primary.example.com"
      api_key = "sk-test"
      supported_models = @("gpt-5.4")
    }
  )
  model_routes = @(
    [ordered]@{
      pattern = "gpt-5.4"
      provider_ids = @("managed-provider")
      priority = 10
    }
  )
  aliases = [ordered]@{
    answer = "gpt-5.4"
  }
}

$routesYaml = @'
providers:
  - id: managed-provider
    preset: openai
    base_url: "https://api.primary.example.com"
    api_key: "sk-test"
    supported_models: [gpt-5.4]
model_routes:
  - pattern: "gpt-5.4"
    provider_ids: [managed-provider]
    priority: 10
aliases:
  answer: gpt-5.4
'@

$SeedCanonicalDocumentJson = '{"aliases":{"answer":"gpt-5.4"},"model_routes":[{"pattern":"gpt-5.4","priority":10,"provider_ids":["managed-provider"]}],"providers":[{"account_name":null,"adapter":null,"api_key":"sk-test","audio_speech_path":null,"audio_transcriptions_path":null,"auth_token":null,"balance_path":null,"base_url":"https://api.primary.example.com","chat_completions_path":null,"completions_path":null,"credentials":[],"default_model":null,"embeddings_path":null,"endpoint_execution_modes":null,"execution_mode":null,"expires_at":null,"extra_body":{},"fetch_path":null,"fetch_urls_field":null,"headers":{},"id":"managed-provider","keepalive":null,"label":null,"messages_path":null,"model_map":{},"preset":"openai","protocol_family":null,"protocol_profile":null,"research_path":null,"responses_path":null,"runtime_state_object_key":null,"search_path":null,"search_query_field":null,"session_auth":null,"supported_models":["gpt-5.4"]}]}'
$SeedCanonicalRoutesYaml = (@(
  'aliases:',
  '  answer: gpt-5.4',
  'model_routes:',
  '- pattern: gpt-5.4',
  '  priority: 10',
  '  provider_ids:',
  '  - managed-provider',
  'providers:',
  '- account_name: null',
  '  adapter: null',
  '  api_key: sk-test',
  '  audio_speech_path: null',
  '  audio_transcriptions_path: null',
  '  auth_token: null',
  '  balance_path: null',
  '  base_url: https://api.primary.example.com',
  '  chat_completions_path: null',
  '  completions_path: null',
  '  credentials: []',
  '  default_model: null',
  '  embeddings_path: null',
  '  endpoint_execution_modes: null',
  '  execution_mode: null',
  '  expires_at: null',
  '  extra_body: {}',
  '  fetch_path: null',
  '  fetch_urls_field: null',
  '  headers: {}',
  '  id: managed-provider',
  '  keepalive: null',
  '  label: null',
  '  messages_path: null',
  '  model_map: {}',
  '  preset: openai',
  '  protocol_family: null',
  '  protocol_profile: null',
  '  research_path: null',
  '  responses_path: null',
  '  runtime_state_object_key: null',
  '  search_path: null',
  '  search_query_field: null',
  '  session_auth: null',
  '  supported_models:',
  '  - gpt-5.4'
) -join "`n") + "`n"
[System.IO.File]::WriteAllText(
  $RoutesPath,
  $routesYaml.Replace("`r`n", "`n") + "`n",
  [System.Text.UTF8Encoding]::new($false)
)

if ($InstallBrowsers) {
  Invoke-LoggedCommand -LogPath $buildLog -Command @(
    "npx.cmd",
    "--prefix",
    $UiRoot,
    "playwright",
    "install",
    "chromium"
  )
}

Invoke-LoggedCommand -LogPath $buildLog -Command @(
  "npm.cmd",
  "run",
  "build:web",
  "--prefix",
  $UiRoot
)

Invoke-LoggedCommand -LogPath $buildLog -Command @(
  "cargo",
  "build",
  "--locked",
  "--bin",
  "neuro-gateway"
)

if (-not (Test-Path -LiteralPath $GatewayBinary -PathType Leaf)) {
  throw "Expected debug gateway binary was not produced: target\debug\neuro-gateway.exe"
}

$dockerExecutable = (Get-Command docker -ErrorAction Stop | Select-Object -First 1).Source
$temporaryRedis = $null
$gatewayProcess = $null
$upstreamProcess = $null
$previousEnvironment = @{}
$environmentKeys = @(
  "GATEWAY_RUNTIME_ROLE",
  "PORT",
  "GATEWAY_REDIS_URL",
  "GATEWAY_API_KEY",
  "GATEWAY_API_KEY_SECRET",
  "GATEWAY_MANAGEMENT_TOKEN",
  "GATEWAY_PUBLIC_BASE_URL",
  "GATEWAY_ROUTES_FILE",
  "GATEWAY_CONSOLE_REDIS_NAMESPACE",
  "GATEWAY_STATE_DIR",
  "RUST_LOG",
  "GATEWAY_UI_BASE_URL",
  "PLAYWRIGHT_SKIP_WEBSERVER",
  "GATEWAY_LIVE_MANAGEMENT_TOKEN",
  "GATEWAY_LIVE_API_BASE_URL",
  "GATEWAY_LIVE_API_TOKEN",
  "GATEWAY_LIVE_EXPECT_PROVIDER_ID",
  "GATEWAY_LIVE_EXPECT_MODEL",
  "GATEWAY_LIVE_EXPECT_ADDED_PROVIDER_ID",
  "GATEWAY_LIVE_EXPECT_ADDED_MODEL",
  "GATEWAY_LIVE_EXPECT_RESTORED_MODEL",
  "GATEWAY_LIVE_EXPECT_REMOVED_MODEL",
  "GATEWAY_LIVE_UPSTREAM_BASE_URL",
  "GATEWAY_LIVE_CHAT_MODEL",
  "GATEWAY_LIVE_CHAT_EXPECT_TEXT",
  "PLAYWRIGHT_HTML_OUTPUT_DIR"
)
foreach ($key in $environmentKeys) {
  $previousEnvironment[$key] = [System.Environment]::GetEnvironmentVariable($key, "Process")
}

try {
  $temporaryRedis = Start-TemporaryRedis -DockerExecutable $dockerExecutable -StartupTimeoutSeconds 30
  Write-LiveLog "started disposable Redis container=$($temporaryRedis.ContainerName) hostPort=$($temporaryRedis.HostPort)"

  $port = Get-FreeTcpPort
  $baseUrl = "http://127.0.0.1:$port"
  $managementToken = "gateway-live-management-$([guid]::NewGuid().ToString('N'))"
  $apiToken = "gateway-live-api-$([guid]::NewGuid().ToString('N'))"
  $apiHeaders = @{ Authorization = "Bearer $apiToken" }
  $chatModel = "gpt-live-e2e"
  $chatExpectedText = "live upstream ok: $chatModel"

  $upstreamPort = Get-FreeTcpPort
  $upstreamFixture = Start-LiveOpenAiCompatibleUpstream `
    -Port $upstreamPort `
    -ScriptPath $upstreamScript `
    -RequestLogPath $upstreamRequests `
    -StdoutPath $upstreamStdout `
    -StderrPath $upstreamStderr
  $upstreamProcess = $upstreamFixture.Process
  Wait-ForLiveOpenAiCompatibleUpstream `
    -Process $upstreamProcess `
    -BaseUrl $upstreamFixture.BaseUrl `
    -StderrPath $upstreamStderr
  Write-LiveLog "started deterministic OpenAI-compatible upstream pid=$($upstreamProcess.Id) baseUrl=$($upstreamFixture.BaseUrl)"

  [System.Environment]::SetEnvironmentVariable("GATEWAY_RUNTIME_ROLE", "standalone", "Process")
  [System.Environment]::SetEnvironmentVariable("PORT", [string]$port, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_REDIS_URL", $temporaryRedis.Url, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_API_KEY", $apiToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_API_KEY_SECRET", "gateway-live-secret-$([guid]::NewGuid().ToString('N'))", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_MANAGEMENT_TOKEN", $managementToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_PUBLIC_BASE_URL", $baseUrl, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_ROUTES_FILE", $RoutesPath, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_STATE_DIR", $GatewayStateRoot, "Process")
  [System.Environment]::SetEnvironmentVariable("RUST_LOG", "info", "Process")

  $gatewayProcess = Start-LiveGatewayProcess `
    -BinaryPath $GatewayBinary `
    -WorkingDirectory $RepoRoot `
    -StdoutPath $gatewayStdout `
    -StderrPath $gatewayStderr
  Write-LiveLog "started Gateway pid=$($gatewayProcess.Id) port=$port"

  Wait-ForGatewayReady -Process $gatewayProcess -BaseUrl $baseUrl -StderrPath $gatewayStderr

  $uiShellResponse = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/ui/" -RetryCount 3
  Assert-StatusCode -Name "/ui/" -Expected 200 -Response $uiShellResponse
  if ($uiShellResponse.Content -notmatch "Neuro Gateway") {
    throw "Gateway /ui/ did not return the embedded browser console shell."
  }

  $models = Invoke-SmokeHttpRequest -Method "GET" -Uri "$baseUrl/v1/models" -Headers $apiHeaders -RetryCount 3
  Assert-StatusCode -Name "/v1/models" -Expected 200 -Response $models
  if ($models.Content -notmatch "gpt-5\.4") {
    throw "/v1/models did not contain the expected gpt-5.4 entry."
  }

  $routeConfigResponse = Invoke-SmokeHttpRequest `
    -Method "GET" `
    -Uri "$baseUrl/v1/internal/gateway/console/route-config" `
    -Headers @{ "x-management-token" = $managementToken } `
    -RetryCount 3
  Assert-StatusCode -Name "/console/route-config" -Expected 200 -Response $routeConfigResponse
  $routeConfigPayload = $routeConfigResponse.Content | ConvertFrom-Json
  $revisionId = [string]$routeConfigPayload.routeConfig.revision.id
  if ([string]::IsNullOrWhiteSpace($revisionId)) {
    throw "Live console route-config response did not include an active revision id."
  }
  $revisionKey = "gw:console:route-config:$($RedisNamespace):revisions:$revisionId"
  $activeRevisionKey = "gw:console:route-config:$($RedisNamespace):active_revision"
  $activeDocumentKey = "gw:console:route-config:$($RedisNamespace):active_document"
  $revisionPayload = @{
    metadata = $routeConfigPayload.routeConfig.revision
    document = $SeedRouteDocument
  } | ConvertTo-Json -Depth 100 -Compress
  $activeDocumentPayload = $SeedRouteDocument | ConvertTo-Json -Depth 100 -Compress
  Set-RedisString -Port $temporaryRedis.HostPort -Key $revisionKey -Value $revisionPayload
  Set-RedisString -Port $temporaryRedis.HostPort -Key $activeRevisionKey -Value $revisionId
  Set-RedisString -Port $temporaryRedis.HostPort -Key $activeDocumentKey -Value $activeDocumentPayload
  $activeRevisionReadback = Get-RedisString -Port $temporaryRedis.HostPort -Key $activeRevisionKey
  if ($activeRevisionReadback.Trim() -ne $revisionId) {
    throw "Redis seed readback for active revision did not match: expected '$revisionId', actual '$activeRevisionReadback'"
  }
  Install-SeedRouteRevisionArchive `
    -StateRoot $GatewayStateRoot `
    -RevisionId $revisionId `
    -RevisionMetadata $routeConfigPayload.routeConfig.revision `
    -CanonicalDocumentJson $SeedCanonicalDocumentJson `
    -CanonicalRoutesYaml $SeedCanonicalRoutesYaml

  Stop-Process -Id $gatewayProcess.Id -Force -ErrorAction SilentlyContinue
  $gatewayProcess.WaitForExit(5000) | Out-Null
  $gatewayProcess = Start-LiveGatewayProcess `
    -BinaryPath $GatewayBinary `
    -WorkingDirectory $RepoRoot `
    -StdoutPath $gatewayStdout `
    -StderrPath $gatewayStderr
  Write-LiveLog "restarted Gateway pid=$($gatewayProcess.Id) after Redis seeding"
  Wait-ForGatewayReady -Process $gatewayProcess -BaseUrl $baseUrl -StderrPath $gatewayStderr

  $redisBackedRouteConfig = Invoke-SmokeHttpRequest `
    -Method "GET" `
    -Uri "$baseUrl/v1/internal/gateway/console/route-config" `
    -Headers @{ "x-management-token" = $managementToken } `
    -RetryCount 3
  Assert-StatusCode -Name "/console/route-config after Redis seed" -Expected 200 -Response $redisBackedRouteConfig
  $redisBackedPayload = $redisBackedRouteConfig.Content | ConvertFrom-Json
  if ([string]$redisBackedPayload.routeConfig.source -ne "redis") {
    throw "Gateway did not adopt the seeded Redis revision state before live browser mutation."
  }

  [System.Environment]::SetEnvironmentVariable("GATEWAY_UI_BASE_URL", "$baseUrl/ui/", "Process")
  [System.Environment]::SetEnvironmentVariable("PLAYWRIGHT_SKIP_WEBSERVER", "1", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_MANAGEMENT_TOKEN", $managementToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_API_BASE_URL", $baseUrl, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_API_TOKEN", $apiToken, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_PROVIDER_ID", "managed-provider", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_MODEL", "gpt-5.4", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_ADDED_PROVIDER_ID", "backup-provider", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_ADDED_MODEL", "gpt-5.4-mini", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_RESTORED_MODEL", "gpt-5.4", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_EXPECT_REMOVED_MODEL", "gpt-5.4-mini", "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_UPSTREAM_BASE_URL", $upstreamFixture.BaseUrl, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_CHAT_MODEL", $chatModel, "Process")
  [System.Environment]::SetEnvironmentVariable("GATEWAY_LIVE_CHAT_EXPECT_TEXT", $chatExpectedText, "Process")
  [System.Environment]::SetEnvironmentVariable("PLAYWRIGHT_HTML_OUTPUT_DIR", $ArtifactsRoot, "Process")

  Invoke-LoggedCommand -LogPath $playwrightLog -Command @(
    "npm.cmd",
    "run",
    "e2e",
    "--prefix",
    $UiRoot,
    "--",
    "--project=$Project",
    "--output",
    $ResultsRoot,
    $LiveSpecPath
  )

  if (-not (Test-Path -LiteralPath $upstreamRequests -PathType Leaf)) {
    throw "OpenAI-compatible fixture upstream did not record any chat completion requests."
  }
  $upstreamRequestLog = [System.IO.File]::ReadAllText($upstreamRequests, [System.Text.Encoding]::UTF8)
  if ($upstreamRequestLog -notmatch [regex]::Escape("/v1/chat/completions") -or
      $upstreamRequestLog -notmatch [regex]::Escape($chatModel)) {
    throw "OpenAI-compatible fixture upstream request log did not contain the expected chat completion model $chatModel."
  }

  $payload = [pscustomobject]@{
    status = "pass"
    project = $Project
    runtimeRoot = $SessionRoot
    gatewayBaseUrl = $baseUrl
    redisUrl = $temporaryRedis.Url
    routesPath = $RoutesPath
    gatewayBinary = "target\debug\neuro-gateway.exe"
    playwrightSpec = $LiveSpecPath
    resultsRoot = $ResultsRoot
    artifactsRoot = $ArtifactsRoot
    logs = [pscustomobject]@{
      build = $buildLog
      playwright = $playwrightLog
      stdout = $gatewayStdout
      stderr = $gatewayStderr
      upstreamStdout = $upstreamStdout
      upstreamStderr = $upstreamStderr
      upstreamRequests = $upstreamRequests
    }
  }

  if ($AsJson) {
    $payload | ConvertTo-Json -Depth 5
  } else {
    Write-LiveLog "status=pass project=$Project"
    Write-LiveLog "runtimeRoot=$SessionRoot"
  }
} finally {
  if ($null -ne $gatewayProcess -and -not $gatewayProcess.HasExited) {
    Stop-Process -Id $gatewayProcess.Id -Force -ErrorAction SilentlyContinue
    $gatewayProcess.WaitForExit(5000) | Out-Null
  }
  if ($null -ne $upstreamProcess -and -not $upstreamProcess.HasExited) {
    Stop-Process -Id $upstreamProcess.Id -Force -ErrorAction SilentlyContinue
    $upstreamProcess.WaitForExit(5000) | Out-Null
  }
  if ($null -ne $temporaryRedis) {
    Stop-TemporaryRedis -DockerExecutable $dockerExecutable -ContainerName $temporaryRedis.ContainerName
  }
  foreach ($key in $environmentKeys) {
    [System.Environment]::SetEnvironmentVariable($key, $previousEnvironment[$key], "Process")
  }
}
