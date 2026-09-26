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
$GatewayBinary = Join-Path $RepoRoot "target\debug\gateway.exe"
$LiveSpecPath = "e2e/console.live.spec.ts"
$RedisImage = "redis:7-alpine"
$RedisNamespace = "default"

. (Join-Path $PSScriptRoot "console-live-e2e\runtime.ps1")
. (Join-Path $PSScriptRoot "console-live-e2e\redis.ps1")
. (Join-Path $PSScriptRoot "console-live-e2e\revision-seed.ps1")
. (Join-Path $PSScriptRoot "console-live-e2e\upstream.ps1")

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
  "gateway"
)

if (-not (Test-Path -LiteralPath $GatewayBinary -PathType Leaf)) {
  throw "Expected debug gateway binary was not produced: target\debug\gateway.exe"
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
    gatewayBinary = "target\debug\gateway.exe"
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
