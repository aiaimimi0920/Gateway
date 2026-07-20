param(
  [switch]$AllowLiveProviderCalls,
  [string]$GatewayBaseUrl,
  [string]$GatewayApiKey,
  [string]$TargetsPath,
  [string]$ArtifactRoot,
  [switch]$AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$GatewayRoot = Split-Path -Parent $PSScriptRoot

if ([string]::IsNullOrWhiteSpace($GatewayApiKey)) {
  $GatewayApiKey = [string]$env:GATEWAY_CANARY_API_KEY
}

if ([string]::IsNullOrWhiteSpace($ArtifactRoot)) {
  $ArtifactRoot = Join-Path $GatewayRoot ("target\live-provider-canary-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
}
New-Item -ItemType Directory -Force -Path $ArtifactRoot | Out-Null

$SummaryPath = Join-Path $ArtifactRoot "summary.json"

function Redact-Secret {
  param([string]$Value)

  if ([string]::IsNullOrWhiteSpace($Value)) {
    return ""
  }
  if ($Value.Length -le 8) {
    return "[redacted]"
  }
  return ($Value.Substring(0, 4) + "..." + $Value.Substring($Value.Length - 4))
}

function Classify-CanaryFailure {
  param(
    [Nullable[int]]$StatusCode,
    [string]$Body
  )

  $text = if ($null -eq $Body) { "" } else { $Body.ToLowerInvariant() }
  if ($StatusCode -eq 429 -or $text.Contains("quota") -or $text.Contains("rate limit")) {
    return "quota_or_rate_limit"
  }
  if (
    $StatusCode -eq 401 -or
    $StatusCode -eq 403 -or
    $text.Contains("unauthorized") -or
    $text.Contains("invalid_api_key") -or
    $text.Contains("invalid api key") -or
    $text.Contains("invalid token") -or
    $text.Contains("expired token") -or
    $text.Contains("authentication token")
  ) {
    return "credential_or_auth"
  }
  if ($StatusCode -ge 500) {
    return "upstream_or_gateway_server"
  }
  if ($StatusCode -ge 400) {
    return "request_or_route_contract"
  }
  return "network_or_unknown"
}

function Write-Summary {
  param([object]$Payload)

  $Payload | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $SummaryPath -Encoding UTF8
  if ($AsJson) {
    $Payload | ConvertTo-Json -Depth 8
  } else {
    Write-Host "[gateway-live-provider-canary] status=$($Payload.status) summary=$SummaryPath"
    if ($Payload.reason) {
      Write-Host "reason=$($Payload.reason)"
    }
  }
}

function Read-CanaryTargets {
  param([string]$Path)

  if ([string]::IsNullOrWhiteSpace($Path)) {
    return @()
  }
  if (-not (Test-Path -LiteralPath $Path)) {
    throw "TargetsPath does not exist: $Path"
  }
  $raw = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
  if ([string]::IsNullOrWhiteSpace($raw)) {
    return @()
  }
  $parsed = $raw | ConvertFrom-Json
  if ($parsed -is [array]) {
    return @($parsed)
  }
  if ($parsed.PSObject.Properties.Name -contains "targets") {
    return @($parsed.targets)
  }
  throw "TargetsPath must contain either a JSON array or an object with a targets array"
}

function Invoke-CanaryHttpRequest {
  param(
    [string]$Method,
    [string]$Uri,
    [string]$Body,
    [string]$GatewayApiKey
  )

  $curl = if (Get-Command "curl.exe" -ErrorAction SilentlyContinue) { "curl.exe" } else { "curl" }
  $requestId = [guid]::NewGuid().ToString("N")
  $bodyPath = Join-Path $ArtifactRoot "canary.$requestId.body"
  $headersPath = Join-Path $ArtifactRoot "canary.$requestId.headers"
  $stderrPath = Join-Path $ArtifactRoot "canary.$requestId.curl.stderr"
  $requestBodyPath = $null
  $authHeaderPath = Join-Path ([System.IO.Path]::GetTempPath()) ("gateway-live-canary-auth-{0}.txt" -f $requestId)
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText(
    $authHeaderPath,
    ("Authorization: Bearer " + $GatewayApiKey),
    $utf8NoBom
  )
  $curlArgs = @(
    "--silent",
    "--show-error",
    "--noproxy",
    "*",
    "--max-time",
    "60",
    "--dump-header",
    $headersPath,
    "--output",
    $bodyPath,
    "--write-out",
    "%{http_code}",
    "--header",
    ("@" + $authHeaderPath),
    "--header",
    ("X-Request-Id: " + $requestId),
    "--header",
    "X-Neuro-Gateway-Route-Proof-Request: v1"
  )
  $curlArgs += "--request"
  $curlArgs += $Method
  if ($PSBoundParameters.ContainsKey("Body")) {
    $requestBodyPath = Join-Path ([System.IO.Path]::GetTempPath()) ("gateway-live-canary-body-{0}.json" -f $requestId)
    [System.IO.File]::WriteAllText($requestBodyPath, $Body, $utf8NoBom)
    $curlArgs += "--header"
    $curlArgs += "Content-Type: application/json"
    $curlArgs += "--data-binary"
    $curlArgs += ("@" + $requestBodyPath)
  }
  $curlArgs += $Uri

  $previousErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    $statusText = & $curl @curlArgs 2> $stderrPath
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
  } finally {
    if (-not [string]::IsNullOrWhiteSpace($requestBodyPath)) {
      Remove-Item -LiteralPath $requestBodyPath -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $authHeaderPath -Force -ErrorAction SilentlyContinue
    $ErrorActionPreference = $previousErrorActionPreference
  }
  $content = if (Test-Path -LiteralPath $bodyPath) {
    Get-Content -LiteralPath $bodyPath -Raw -Encoding UTF8
  } else {
    ""
  }
  if ($exitCode -ne 0) {
    $stderrText = if (Test-Path -LiteralPath $stderrPath) {
      Get-Content -LiteralPath $stderrPath -Raw -Encoding UTF8
    } else {
      ""
    }
    return [pscustomobject]@{
      statusCode = $null
      content = $content
      curlExitCode = $exitCode
      curlError = $stderrText.Trim()
      requestId = $requestId
      headersPath = $headersPath
      bodyPath = $bodyPath
    }
  }

  $statusCandidate = (($statusText | Out-String).Trim() -split "\s+")[-1]
  [pscustomobject]@{
    statusCode = if ($statusCandidate -match '^\d{3}$') { [int]$statusCandidate } else { $null }
    content = $content
    curlExitCode = $exitCode
    curlError = ""
    requestId = $requestId
    headersPath = $headersPath
    bodyPath = $bodyPath
  }
}

function Get-CanaryTargetProperty {
  param(
    [object]$Target,
    [string]$Name
  )

  if ($null -eq $Target) {
    return $null
  }
  $property = $Target.PSObject.Properties[$Name]
  if ($null -eq $property) {
    return $null
  }
  return $property.Value
}

function Read-CanaryResponseHeaders {
  param([string]$Path)

  $headers = @{}
  if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path)) {
    return $headers
  }

  foreach ($line in (Get-Content -LiteralPath $Path -Encoding UTF8)) {
    if ($line -match '^HTTP/\d(?:\.\d)?\s+\d{3}') {
      $headers = @{}
      continue
    }
    if ([string]::IsNullOrWhiteSpace($line)) {
      continue
    }
    $separator = $line.IndexOf(":")
    if ($separator -le 0) {
      continue
    }
    $name = $line.Substring(0, $separator).Trim().ToLowerInvariant()
    $value = $line.Substring($separator + 1).Trim()
    if (-not [string]::IsNullOrWhiteSpace($name)) {
      $headers[$name] = $value
    }
  }
  return $headers
}

function Get-CanaryResponseHeader {
  param(
    [hashtable]$Headers,
    [string]$Name
  )

  if ($null -eq $Headers) {
    return ""
  }
  $key = $Name.ToLowerInvariant()
  if ($Headers.ContainsKey($key)) {
    return [string]$Headers[$key]
  }
  return ""
}

function Test-SafeProviderLine {
  param([string]$Value)

  return -not [string]::IsNullOrWhiteSpace($Value) -and
    $Value.Length -le 128 -and
    $Value -match '^[A-Za-z0-9][A-Za-z0-9._-]*$'
}

function Invoke-CanaryTarget {
  param([object]$Target)

  $endpoint = [string](Get-CanaryTargetProperty -Target $Target -Name "endpoint")
  if ([string]::IsNullOrWhiteSpace($endpoint)) {
    $endpoint = "/v1/chat/completions"
  }
  $method = [string](Get-CanaryTargetProperty -Target $Target -Name "method")
  if ([string]::IsNullOrWhiteSpace($method)) {
    $method = "POST"
  }
  $method = $method.Trim().ToUpperInvariant()
  $model = [string](Get-CanaryTargetProperty -Target $Target -Name "model")
  $declaredProviderLine = [string](Get-CanaryTargetProperty -Target $Target -Name "provider_line")
  $expectedProviderLine = [string](Get-CanaryTargetProperty -Target $Target -Name "expected_provider_line")
  if ([string]::IsNullOrWhiteSpace($expectedProviderLine)) {
    $expectedProviderLine = $declaredProviderLine
  }
  $body = if ($Target.PSObject.Properties.Name -contains "body") {
    $Target.body | ConvertTo-Json -Depth 8 -Compress
  } else {
    @{ model = $model; messages = @(@{ role = "user"; content = "gateway live canary" }); max_tokens = 1 } | ConvertTo-Json -Depth 8 -Compress
  }

  if ([string]::IsNullOrWhiteSpace($expectedProviderLine)) {
    return [pscustomobject]@{
      name = [string](Get-CanaryTargetProperty -Target $Target -Name "name")
      provider_line = $declaredProviderLine
      credential_source = [string](Get-CanaryTargetProperty -Target $Target -Name "credential_source")
      endpoint = $endpoint
      method = $method
      model = $model
      expected_provider_line = ""
      observed_provider_line = $null
      request_id = ""
      route_proof = $null
      http_status = $null
      status = "fail"
      failure_classification = "route_proof_missing_or_mismatch"
      body_path = $null
      headers_path = $null
      timestamp = (Get-Date).ToString("o")
    }
  }

  $uri = $GatewayBaseUrl.TrimEnd("/") + $endpoint
  $requestArguments = @{
    Method = $method
    Uri = $uri
    GatewayApiKey = $GatewayApiKey
  }
  if (-not [string]::Equals($method, "GET", [System.StringComparison]::OrdinalIgnoreCase)) {
    $requestArguments["Body"] = $body
  }
  $response = Invoke-CanaryHttpRequest @requestArguments
  $responseHeaders = Read-CanaryResponseHeaders -Path $response.headersPath
  $observedProviderLine = Get-CanaryResponseHeader `
    -Headers $responseHeaders `
    -Name "x-neuro-gateway-provider-line"
  $proofVersion = Get-CanaryResponseHeader `
    -Headers $responseHeaders `
    -Name "x-neuro-gateway-route-proof"
  $observedRequestId = Get-CanaryResponseHeader `
    -Headers $responseHeaders `
    -Name "x-request-id"
  $httpFailed = $null -eq $response.statusCode -or $response.statusCode -lt 200 -or $response.statusCode -ge 300
  $proofAvailable = $proofVersion -eq "v1" -and (Test-SafeProviderLine -Value $observedProviderLine)
  $proofMatches = $proofAvailable -and
    $observedRequestId -eq [string]$response.requestId -and
    $observedProviderLine -eq $expectedProviderLine
  $classification = if ($httpFailed) {
    Classify-CanaryFailure -StatusCode $response.statusCode -Body (($response.content + "`n" + $response.curlError))
  } elseif (-not $proofMatches) {
    "route_proof_missing_or_mismatch"
  } else {
    "success"
  }

  $routeProof = $null
  if ($proofAvailable) {
    $routeProof = [ordered]@{
      source = "gateway_response_headers_v1"
      provider_line = $observedProviderLine
      request_id = $observedRequestId
    }
  }

  [pscustomobject]@{
    name = [string](Get-CanaryTargetProperty -Target $Target -Name "name")
    provider_line = $declaredProviderLine
    credential_source = [string](Get-CanaryTargetProperty -Target $Target -Name "credential_source")
    endpoint = $endpoint
    method = $method
    model = $model
    expected_provider_line = $expectedProviderLine
    observed_provider_line = if ($proofAvailable) { $observedProviderLine } else { $null }
    request_id = $observedRequestId
    route_proof = $routeProof
    http_status = $response.statusCode
    status = if ($classification -eq "success") { "pass" } else { "fail" }
    failure_classification = if ($classification -eq "success") { $null } else { $classification }
    body_path = $response.bodyPath
    headers_path = $response.headersPath
    timestamp = (Get-Date).ToString("o")
  }
}

$targets = @()
$status = "skipped"
$reason = $null

try {
  $targets = @(Read-CanaryTargets -Path $TargetsPath)
  if (-not $AllowLiveProviderCalls) {
    $reason = "Live provider calls are disabled by default; pass -AllowLiveProviderCalls with -GatewayBaseUrl, -GatewayApiKey, and -TargetsPath to run."
    $payload = [pscustomobject]@{
      status = $status
      reason = $reason
      allowLiveProviderCalls = $false
      gatewayBaseUrl = $GatewayBaseUrl
      gatewayApiKey = Redact-Secret -Value $GatewayApiKey
      targetsPath = $TargetsPath
      targets = @()
      summaryPath = $SummaryPath
    }
    Write-Summary -Payload $payload
    exit 0
  }

  if ([string]::IsNullOrWhiteSpace($GatewayBaseUrl)) {
    throw "-GatewayBaseUrl is required when -AllowLiveProviderCalls is set"
  }
  if ([string]::IsNullOrWhiteSpace($GatewayApiKey)) {
    throw "-GatewayApiKey is required when -AllowLiveProviderCalls is set"
  }
  if ([string]::IsNullOrWhiteSpace($TargetsPath)) {
    throw "-TargetsPath is required when -AllowLiveProviderCalls is set"
  }
  if ($targets.Count -eq 0) {
    throw "TargetsPath did not provide any canary targets"
  }

  $results = @()
  foreach ($target in $targets) {
    $results += Invoke-CanaryTarget -Target $target
  }
  $failed = @($results | Where-Object { $_.status -ne "pass" })
  $status = if ($failed.Count -eq 0) { "pass" } else { "fail" }
  $payload = [pscustomobject]@{
    status = $status
    reason = $null
    allowLiveProviderCalls = $true
    gatewayBaseUrl = $GatewayBaseUrl
    gatewayApiKey = Redact-Secret -Value $GatewayApiKey
    targetsPath = $TargetsPath
    targets = $results
    summaryPath = $SummaryPath
  }
  Write-Summary -Payload $payload
  if ($status -ne "pass") {
    exit 1
  }
} catch {
  $payload = [pscustomobject]@{
    status = "fail"
    reason = $_.Exception.Message
    allowLiveProviderCalls = [bool]$AllowLiveProviderCalls
    gatewayBaseUrl = $GatewayBaseUrl
    gatewayApiKey = Redact-Secret -Value $GatewayApiKey
    targetsPath = $TargetsPath
    targets = @()
    summaryPath = $SummaryPath
  }
  Write-Summary -Payload $payload
  exit 1
}
