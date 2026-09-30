[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BaseUrl,
    [Parameter(Mandatory = $true)][string[]]$CredentialPaths,
    [ValidateSet('free', 'plus', 'pro', 'pro20x', 'team-mother', 'team-child')][string]$Group = 'free'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$uri = [Uri]$BaseUrl
if (-not $uri.IsLoopback -or $uri.Scheme -ne 'http') { throw 'Use a loopback HTTP Gateway endpoint.' }
$token = $env:GATEWAY_CONSOLE_TOKEN
if ([string]::IsNullOrWhiteSpace($token)) { throw 'Set GATEWAY_CONSOLE_TOKEN in the process environment.' }
$headers = @{ Authorization = "Bearer $token" }
$root = $BaseUrl.TrimEnd('/') + '/v1/internal/gateway/console'
function Invoke-Console([string]$Method, [string]$Path, $Body) {
    $params = @{ Method = $Method; Uri = "$root$Path"; Headers = $headers; TimeoutSec = 90 }
    if ($null -ne $Body) {
        $params.ContentType = 'application/json; charset=utf-8'
        $params.Body = [Text.Encoding]::UTF8.GetBytes(($Body | ConvertTo-Json -Depth 100 -Compress))
    }
    try { Invoke-RestMethod @params } catch {
        # Validation diagnostics are useful; remove source secrets before reporting them.
        $status = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
        $detail = [string]$_.ErrorDetails.Message
        foreach ($credentialPath in $CredentialPaths) {
            $source = [IO.File]::ReadAllText($credentialPath) | ConvertFrom-Json
            foreach ($field in @('access_token', 'refresh_token', 'id_token')) {
                if ($source.PSObject.Properties[$field] -and $source.$field) { $detail = $detail.Replace([string]$source.$field, '[redacted]') }
            }
        }
        $detail = $detail.Replace($token, '[redacted]')
        throw "Gateway $Method $Path failed (HTTP $status): $detail"
    }
}
$config = (Invoke-Console GET '/route-config' $null).routeConfig
if (@($config.document.providers | Where-Object id -eq 'chatgpt').Count -gt 0) {
    throw 'ChatGPT pool already exists; refusing to replace its credentials.'
}
$patches = [Collections.Generic.List[object]]::new()
foreach ($secret in $config.secrets) {
    if ($secret.configured) { $patches.Add(@{ path = $secret.path; operation = 'keep' }) }
}
$credentials = [Collections.Generic.List[object]]::new()
$seen = [Collections.Generic.HashSet[string]]::new()
$allModels = [Collections.Generic.HashSet[string]]::new()
$providerIndex = @($config.document.providers).Count
foreach ($path in $CredentialPaths) {
    $raw = [IO.File]::ReadAllText([IO.Path]::GetFullPath($path), [Text.Encoding]::UTF8) | ConvertFrom-Json
    foreach ($field in @('type', 'access_token', 'account_id', 'email', 'refresh_token', 'expired')) {
        if (-not $raw.PSObject.Properties[$field] -or [string]::IsNullOrWhiteSpace([string]$raw.$field)) {
            throw "CPA credential is missing $field."
        }
    }
    if ($raw.type -ne 'codex') { throw 'Expected a CPA codex credential.' }
    if (-not $seen.Add([string]$raw.account_id)) { throw 'Duplicate ChatGPT account in import.' }
    $expiry = [DateTimeOffset]::Parse($raw.expired)
    $remaining = [Math]::Floor(($expiry - [DateTimeOffset]::UtcNow).TotalSeconds)
    if ($remaining -le 0) { throw 'Credential is expired; refresh it at its source before import.' }
    $id = 'chatgpt-' + ($raw.email -replace '[^a-zA-Z0-9_-]', '-')
    $index = $credentials.Count
    $catalogHeaders = @{
        Authorization = "Bearer $($raw.access_token)"
        'Chatgpt-Account-Id' = [string]$raw.account_id
        Originator = 'codex_cli_rs'
    }
    try {
        $catalog = Invoke-RestMethod -Uri 'https://chatgpt.com/backend-api/codex/models?client_version=0.154.0' `
            -Headers $catalogHeaders -UserAgent 'codex_cli_rs/0.154.0 (Mac OS 26.3.1; arm64) iTerm.app/3.6.9' -TimeoutSec 30
        $models = @($catalog.models | ForEach-Object { [string]$_.slug } | Where-Object { $_ } | Select-Object -Unique)
    } catch { throw 'Could not read the account model catalog; no credentials were imported.' }
    if ($models.Count -eq 0) { throw 'Account returned no models; no credentials were imported.' }
    foreach ($model in $models) { $null = $allModels.Add($model) }
    $credentials.Add([ordered]@{
        id = $id; account_name = [string]$raw.email
        credential_identity_category_id = $Group
        enabled = -not ($raw.PSObject.Properties['disabled'] -and $raw.disabled)
        api_key = $null; refresh_token = $null
        refresh_endpoint = 'https://auth.openai.com/oauth/token'
        refresh_client_id = 'app_EMoamEEZ73f0CkXaXp7hrann'
        expires_at = $expiry.ToUniversalTime().ToString('o')
        token_expires_in_secs = [long]$remaining
        headers = @{ 'Chatgpt-Account-Id' = [string]$raw.account_id }
        supported_models = $models
    })
    $prefix = "/providers/$providerIndex/credentials/$index"
    $patches.Add(@{ path = "$prefix/api_key"; operation = 'replace'; value = [string]$raw.access_token })
    $patches.Add(@{ path = "$prefix/refresh_token"; operation = 'replace'; value = [string]$raw.refresh_token })
}
$categories = @('free', 'plus', 'pro', 'pro20x', 'team-mother', 'team-child') | ForEach-Object {
    @{ id = $_; label = $_.Replace('-', ' '); pool_target_size = 30; auto_refill_enabled = $false; auto_prune_enabled = $false }
}
$provider = [ordered]@{
    id = 'chatgpt'; label = 'ChatGPT'; vendor_key = 'chatgpt_platform'; vendor_name = 'ChatGPT'
    preset = 'chatgpt-codex-oauth-official-api'; protocol_profile = 'chatgpt_codex_backend'
    base_url = 'https://chatgpt.com/backend-api/codex'
    default_model = if ($allModels.Contains('gpt-5.6-luna')) { 'gpt-5.6-luna' } else { @($allModels)[0] }
    supported_models = @($allModels); credential_identity_categories = @($categories)
    credentials = @($credentials.ToArray())
}
$config.document.providers = @($config.document.providers) + @($provider)
$grant = Invoke-Console POST '/session/confirm-secret-access' @{ token = $token }
$headers['x-secret-grant'] = $grant.grant
$body = @{ document = $config.document; secretPatches = @($patches.ToArray()) }
$null = Invoke-Console POST '/route-config/validate' $body
$body.expectedRevision = $config.revision.id
$body.message = 'Import CPA ChatGPT OAuth credentials into native subscription pool'
$result = Invoke-Console PUT '/route-config' $body
if (-not $result.committed) { throw 'Gateway did not confirm the import.' }
[ordered]@{ provider = 'chatgpt'; group = $Group; imported = $credentials.Count; revision = $result.routeConfig.revision.id } | ConvertTo-Json -Compress
