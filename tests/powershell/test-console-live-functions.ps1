param(
  [Parameter(Mandatory = $true)][string] $ModuleRoot,
  [Parameter(Mandatory = $true)][string] $WorkRoot
)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
foreach ($name in @("runtime", "redis", "revision-seed", "upstream")) {
  . (Join-Path $ModuleRoot "$name.ps1")
}
$null = Ensure-Directory -Path $WorkRoot

$bytes = [System.Text.Encoding]::UTF8.GetBytes("+fixture`r`n")
$stream = [System.IO.MemoryStream]::new($bytes)
try {
  if ((Read-RedisLine -Stream $stream) -cne "+fixture") { throw "RESP line mismatch" }
} finally { $stream.Dispose() }

$document = '{"providers":[]}'
$yaml = "providers: []`n"
$metadata = [pscustomobject]@{
  id = "fixture-revision"; sequence = 1; parent = $null; actor = "fixture"
  timestamp = "2026-09-08T00:00:00Z"; message = "fixture"
  documentDigest = Get-Utf8Sha256Hex -Text $document
  yamlDigest = Get-Utf8Sha256Hex -Text $yaml
}
Install-SeedRouteRevisionArchive -StateRoot $WorkRoot -RevisionId $metadata.id `
  -RevisionMetadata $metadata -CanonicalDocumentJson $document -CanonicalRoutesYaml $yaml
$archive = Join-Path $WorkRoot "console/revisions/fixture-revision"
if ([IO.File]::ReadAllText((Join-Path $archive "document.json")) -cne $document) { throw "Document mismatch" }
if ([IO.File]::ReadAllText((Join-Path $archive "routes.yaml")) -cne $yaml) { throw "YAML mismatch" }
$stored = [IO.File]::ReadAllText((Join-Path $archive "metadata.json")) | ConvertFrom-Json
if ($stored.documentDigest -cne $metadata.documentDigest -or $stored.yamlDigest -cne $metadata.yamlDigest) {
  throw "Archived digests mismatch"
}
$rejected = $false
try {
  Install-SeedRouteRevisionArchive -StateRoot $WorkRoot -RevisionId "bad" `
    -RevisionMetadata $metadata -CanonicalDocumentJson '{}' -CanonicalRoutesYaml $yaml
} catch { $rejected = $_.Exception.Message -like "Seed revision document digest mismatch:*" }
if (-not $rejected -or (Test-Path (Join-Path $WorkRoot "console/revisions/bad"))) {
  throw "Invalid revision was not rejected before publication"
}

$fixture = $null
try {
  $fixture = Start-LiveOpenAiCompatibleUpstream -Port (Get-FreeTcpPort) `
    -ScriptPath (Join-Path $WorkRoot "fixture.mjs") -RequestLogPath (Join-Path $WorkRoot "requests.jsonl") `
    -StdoutPath (Join-Path $WorkRoot "stdout.log") -StderrPath (Join-Path $WorkRoot "stderr.log")
  Wait-ForLiveOpenAiCompatibleUpstream -Process $fixture.Process -BaseUrl $fixture.BaseUrl -StderrPath $fixture.StderrPath
  $response = Invoke-SmokeHttpRequest -Method POST -Uri "$($fixture.BaseUrl)/v1/chat/completions" `
    -Headers @{ Authorization = "Bearer fixture-only" } -Body '{"model":"fixture-model","messages":[]}'
  Assert-StatusCode -Name "fixture completion" -Expected 200 -Response $response
  $body = $response.Content | ConvertFrom-Json
  if ($body.choices[0].message.content -cne "live upstream ok: fixture-model") { throw "Completion mismatch" }
  $record = [IO.File]::ReadAllText($fixture.RequestLogPath) | ConvertFrom-Json
  if ($record.model -cne "fixture-model" -or $record.path -cne "/v1/chat/completions") { throw "Request log mismatch" }
} finally {
  if ($null -ne $fixture) {
    if (-not $fixture.Process.HasExited) {
      Stop-Process -Id $fixture.Process.Id -Force
      if (-not $fixture.Process.WaitForExit(5000)) { throw "Owned fixture process did not exit" }
    }
    $fixture.Process.Dispose()
  }
}
Write-Output "console-live-functions: pass"
