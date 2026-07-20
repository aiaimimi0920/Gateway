[CmdletBinding()]
param(
    [string]$LineId = "",
    [switch]$All,
    [switch]$SkipCargo,
    [switch]$LibOnly,
    [string]$SharedCargoTargetDir = "",
    [string]$EvidenceRoot = "",
    [string]$ArtifactRoot = "",
    [string]$InventoryPath = "",
    [string]$Timestamp = "",
    [switch]$AllowLiveProviderCalls,
    [string]$GatewayBaseUrl = "",
    [string]$GatewayApiKey = "",
    [string]$TargetsPath = "",
    [string]$LiveCanaryPath = "",
    [switch]$AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$GatewayRoot = Split-Path -Parent $PSScriptRoot
$ManifestRoot = Join-Path $GatewayRoot "manifests\lines"
$ManifestValidator = Join-Path $GatewayRoot "tools\validate-gateway-line-manifests.py"
$InventoryGenerator = Join-Path $GatewayRoot "tools\generate-gateway-provider-inventory.py"
$InventoryValidator = Join-Path $GatewayRoot "tools\validate-gateway-provider-evidence.py"
$LineVerifier = Join-Path $GatewayRoot "tools\verify-gateway-line.ps1"
$LiveCanary = if ([string]::IsNullOrWhiteSpace($LiveCanaryPath)) {
    Join-Path $GatewayRoot "scripts\invoke-gateway-live-provider-canary.ps1"
} else {
    [System.IO.Path]::GetFullPath($LiveCanaryPath)
}
$Mode = if ($AllowLiveProviderCalls) { "live" } else { "offline" }

$RepresentativeLineIds = @(
    "anthropic-messages-official-model-api",
    "chatgpt-web-reverse",
    "freebuff-web-reverse-api",
    "jina-search-official-vendor-api",
    "suno-web-reverse-api",
    "xfyun-native-websocket-official-vendor-api"
)

function Get-OptionalProperty {
    param(
        [object]$Object,
        [string]$Name
    )

    if ($null -eq $Object) {
        return $null
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Write-Utf8NoBom {
    param(
        [string]$Path,
        [string]$Content
    )

    $parent = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function ConvertTo-JsonText {
    param([object]$Payload)

    return (($Payload | ConvertTo-Json -Depth 20) + "`n")
}

function Write-RunnerResult {
    param([object]$Payload)

    if ($AsJson) {
        Write-Output ($Payload | ConvertTo-Json -Depth 12)
        return
    }
    Write-Output (
        "[gateway-provider-evidence] status={0} mode={1} evidence={2}" -f `
            $Payload.status,
            $Payload.mode,
            $Payload.evidencePath
    )
    if (-not [string]::IsNullOrWhiteSpace([string]$Payload.reason)) {
        Write-Output ("reason={0}" -f $Payload.reason)
    }
}

function Resolve-PythonExecutable {
    foreach ($candidate in @("python", "python3")) {
        $command = Get-Command $candidate -ErrorAction SilentlyContinue
        if ($null -ne $command) {
            return [string]$command.Source
        }
    }
    throw "Python is required to generate and validate Gateway provider evidence."
}

function Resolve-PowerShellExecutable {
    try {
        $current = (Get-Process -Id $PID -ErrorAction Stop).Path
        if (-not [string]::IsNullOrWhiteSpace($current)) {
            return $current
        }
    } catch {
        # Fall through to PATH lookup when the host process path is unavailable.
    }

    foreach ($candidate in @("pwsh", "powershell")) {
        $command = Get-Command $candidate -ErrorAction SilentlyContinue
        if ($null -ne $command) {
            return [string]$command.Source
        }
    }
    throw "PowerShell is required to run focused Gateway line verification."
}

function Invoke-CapturedCommand {
    param(
        [string]$Executable,
        [string[]]$CommandArguments,
        [string]$StdoutPath,
        [string]$StderrPath
    )

    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $Executable @CommandArguments 1> $StdoutPath 2> $StderrPath
        $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    return [pscustomobject]@{
        exitCode = $exitCode
        stdoutPath = $StdoutPath
        stderrPath = $StderrPath
    }
}

function Read-JsonFile {
    param([string]$Path)

    $raw = Get-Content -LiteralPath $Path -Raw -Encoding UTF8
    if ([string]::IsNullOrWhiteSpace($raw)) {
        throw "JSON output is empty: $Path"
    }
    return ($raw | ConvertFrom-Json)
}

function Get-LineManifests {
    $lines = @()
    foreach ($path in Get-ChildItem -LiteralPath $ManifestRoot -Recurse -File -Filter "*.json" | Sort-Object FullName) {
        $manifest = Get-Content -LiteralPath $path.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
        $lines += [pscustomobject]@{
            Id = [string]$manifest.id
            Path = $path.FullName
            Manifest = $manifest
        }
    }
    return @($lines | Sort-Object Id)
}

function Read-LiveTargets {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "TargetsPath does not exist: $Path"
    }
    $parsed = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($parsed -is [array]) {
        return @($parsed)
    }
    $targets = Get-OptionalProperty -Object $parsed -Name "targets"
    if ($null -ne $targets) {
        return @($targets)
    }
    throw "TargetsPath must contain either a JSON array or an object with a targets array."
}

function Select-LineManifests {
    param(
        [array]$Lines,
        [array]$LiveTargets
    )

    if ($All -and -not [string]::IsNullOrWhiteSpace($LineId)) {
        throw "LineId cannot be combined with -All."
    }

    $requestedIds = @()
    if ($All) {
        $requestedIds = @($Lines | ForEach-Object { $_.Id })
    } elseif (-not [string]::IsNullOrWhiteSpace($LineId)) {
        $requestedIds = @(
            $LineId -split "[,;]" |
                ForEach-Object { $_.Trim() } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        )
    } elseif ($AllowLiveProviderCalls) {
        $requestedIds = @(
            $LiveTargets |
                ForEach-Object { [string](Get-OptionalProperty -Object $_ -Name "provider_line") } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
                Sort-Object -Unique
        )
    } else {
        $requestedIds = $RepresentativeLineIds
    }

    if ($requestedIds.Count -eq 0) {
        throw "No Gateway provider lines were selected. Use -LineId, -All, or provide live targets with provider_line labels."
    }

    $byId = @{}
    foreach ($line in $Lines) {
        $byId[$line.Id] = $line
    }
    $selected = @()
    foreach ($requestedId in $requestedIds | Select-Object -Unique) {
        if (-not $byId.ContainsKey($requestedId)) {
            throw "Gateway line not found: $requestedId"
        }
        $selected += $byId[$requestedId]
    }
    return @($selected | Sort-Object Id)
}

function Get-VerificationCategory {
    param([object]$Line)

    $manifest = $Line.Manifest
    $profile = [string]$manifest.identity.protocolProfile
    $families = @($manifest.capabilities.families | ForEach-Object { [string]$_ })
    $executionMode = [string]$manifest.identity.executionMode
    if ($profile -match "websocket" -or $Line.Id -match "websocket") {
        return "websocket"
    }
    if ($families -contains "search") {
        return "search"
    }
    if (
        $families -contains "music" -or
        $families -contains "video" -or
        $families -contains "videos"
    ) {
        return "media"
    }
    if ($executionMode -eq "browser_backed") {
        return "browser_backed"
    }
    if ($executionMode -eq "direct_http_replay") {
        return "direct_http_replay"
    }
    return "official_http"
}

function Get-EndpointFamily {
    param([object]$Line)

    $category = Get-VerificationCategory -Line $Line
    if ($category -in @("websocket", "search", "media")) {
        return $category
    }
    $families = @($Line.Manifest.capabilities.families | ForEach-Object { [string]$_ })
    if ($families.Count -gt 0) {
        return $families[0]
    }
    return "unknown"
}

function ConvertTo-SafeLabel {
    param(
        [string]$Value,
        [string]$Fallback
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $Fallback
    }
    $candidate = $Value.Trim()
    if (
        $candidate.Length -gt 128 -or
        $candidate -match "(?i)(bearer\s+[a-z0-9._~+/=-]+|basic\s+[a-z0-9+/=]{8,}|sk-[a-z0-9]|akia[0-9a-z]{16}|AIza[0-9a-z_-]{20,}|ya29\.[a-z0-9._-]+|eyJ[a-z0-9_-]{8}|(?:cookie|set-cookie)\s*[:=]|(?:session(?:id|_token)?|refresh_token|api[_-]?key|access[_-]?token|authorization|token|secret|password)\s*[=:])"
    ) {
        return $Fallback
    }
    return $candidate
}

function New-SafeLiveTargetSummary {
    param([object]$Target)

    if ($null -eq $Target) {
        return $null
    }
    $summary = [ordered]@{}
    foreach ($name in @(
        "name", "provider_line", "endpoint", "model", "execution_mode",
        "fallback_reason", "remote_executor_ready", "local_browser_ready",
        "session_material_ready", "credential_refresh_ready", "lease_cooling_ready",
        "model_health_ready"
    )) {
        $value = Get-OptionalProperty -Object $Target -Name $name
        if ($null -ne $value) {
            if ($name -eq "fallback_reason") {
                $summary[$name] = ConvertTo-SafeLabel -Value ([string]$value) -Fallback "operator-classified"
            } elseif ($value -is [bool]) {
                $summary[$name] = [bool]$value
            } else {
                $summary[$name] = ConvertTo-SafeLabel -Value ([string]$value) -Fallback "unavailable"
            }
        }
    }
    $method = [string](Get-OptionalProperty -Object $Target -Name "method")
    if ([string]::IsNullOrWhiteSpace($method)) {
        $method = "POST"
    }
    $method = $method.Trim().ToUpperInvariant()
    if ($method -notin @("GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS")) {
        $method = "UNSPECIFIED"
    }
    $summary["method"] = $method
    $summary["credential_source"] = ConvertTo-SafeLabel `
        -Value ([string](Get-OptionalProperty -Object $Target -Name "credential_source")) `
        -Fallback "operator:configured"
    return $summary
}

# Project only the versioned route identity needed for evidence. Provider
# account IDs, credential IDs, labels, credential refs, access IDs, and all
# authentication material are deliberately excluded from durable records.
function New-SafeProviderProof {
    param(
        [object]$TargetResult,
        [string]$LineId,
        [string]$ArtifactPath = ""
    )

    $resultProviderLine = [string](Get-OptionalProperty -Object $TargetResult -Name "provider_line")
    $observedLine = [string](Get-OptionalProperty -Object $TargetResult -Name "observed_provider_line")
    $rawProof = Get-OptionalProperty -Object $TargetResult -Name "route_proof"
    $rawProofIsObject = $rawProof -is [pscustomobject] -or $rawProof -is [System.Collections.IDictionary]
    if (
        $resultProviderLine -cne $LineId -or
        [string]::IsNullOrWhiteSpace($observedLine) -or
        $observedLine -cne $LineId -or
        $null -eq $rawProof -or
        -not $rawProofIsObject
    ) {
        return $null
    }
    $proofSource = Get-OptionalProperty -Object $rawProof -Name "source"
    $source = [string]$proofSource
    if ($source -cne "gateway_response_headers_v1") {
        return $null
    }
    $proofProviderLine = [string](Get-OptionalProperty -Object $rawProof -Name "provider_line")
    if (
        $proofProviderLine -cne $LineId
    ) {
        return $null
    }
    $proofRequestId = [string](Get-OptionalProperty -Object $rawProof -Name "request_id")
    $resultRequestId = [string](Get-OptionalProperty -Object $TargetResult -Name "request_id")
    if (
        [string]::IsNullOrWhiteSpace($proofRequestId) -or
        [string]::IsNullOrWhiteSpace($resultRequestId) -or
        $proofRequestId -cne $resultRequestId
    ) {
        return $null
    }
    $httpStatusValue = Get-OptionalProperty -Object $TargetResult -Name "http_status"
    [int]$httpStatus = 0
    if (
        $null -eq $httpStatusValue -or
        -not [int]::TryParse([string]$httpStatusValue, [ref]$httpStatus) -or
        $httpStatus -lt 200 -or
        $httpStatus -ge 300
    ) {
        return $null
    }
    $proof = [ordered]@{
        providerLine = $LineId
        source = $source
    }
    foreach ($entry in @(
        @{ source = "endpoint"; target = "endpoint" },
        @{ source = "method"; target = "method" },
        @{ source = "http_status"; target = "httpStatus" },
        @{ source = "request_id"; target = "requestId" }
    )) {
        $value = Get-OptionalProperty -Object $TargetResult -Name $entry.source
        if ($null -ne $value -and -not [string]::IsNullOrWhiteSpace([string]$value)) {
            if ($entry.target -eq "httpStatus") {
                $proof[$entry.target] = [int]$value
            } else {
                $proof[$entry.target] = ConvertTo-SafeLabel -Value ([string]$value) -Fallback "unavailable"
            }
        }
    }
    if (-not [string]::IsNullOrWhiteSpace($ArtifactPath)) {
        $proof.artifactPath = $ArtifactPath
    }
    return $proof
}

function Get-ManifestCredentialLabel {
    param([object]$Line)

    $materialKinds = @(
        $Line.Manifest.credentials.materialKinds |
            ForEach-Object { [string]$_ } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    )
    if ($materialKinds.Count -eq 0) {
        return "manifest:none"
    }
    return ("manifest:" + (($materialKinds | Sort-Object -Unique) -join "+"))
}

function Get-ReadinessValue {
    param(
        [object]$Target,
        [string]$PropertyName,
        [bool]$Required
    )

    if (-not $Required) {
        return "not_required"
    }
    $value = Get-OptionalProperty -Object $Target -Name $PropertyName
    if ($null -eq $value) {
        return "not_checked"
    }
    if ($value -is [bool]) {
        return $(if ($value) { "ready" } else { "unavailable" })
    }
    $text = [string]$value
    if ([string]::IsNullOrWhiteSpace($text)) {
        return "not_checked"
    }
    return (ConvertTo-SafeLabel -Value $text -Fallback "not_checked")
}

function New-ReadinessDiagnostics {
    param(
        [object]$Line,
        [object]$Target
    )

    $executionMode = [string]$Line.Manifest.identity.executionMode
    $materialKinds = @($Line.Manifest.credentials.materialKinds | ForEach-Object { [string]$_ })
    $browserRequired = $executionMode -eq "browser_backed"
    $sessionRequired = $materialKinds -contains "session_auth" -or $materialKinds -contains "browser_state"
    $refreshRequired = $sessionRequired -or $materialKinds -contains "bearer_token"

    return [ordered]@{
        remoteExecutor = [ordered]@{
            required = $browserRequired
            status = Get-ReadinessValue -Target $Target -PropertyName "remote_executor_ready" -Required $browserRequired
        }
        localBrowserFallback = [ordered]@{
            required = $browserRequired
            status = Get-ReadinessValue -Target $Target -PropertyName "local_browser_ready" -Required $browserRequired
        }
        sessionMaterial = [ordered]@{
            required = $sessionRequired
            status = Get-ReadinessValue -Target $Target -PropertyName "session_material_ready" -Required $sessionRequired
        }
        credentialRefresh = [ordered]@{
            required = $refreshRequired
            status = Get-ReadinessValue -Target $Target -PropertyName "credential_refresh_ready" -Required $refreshRequired
        }
        leaseCooling = [ordered]@{
            required = $true
            status = Get-ReadinessValue -Target $Target -PropertyName "lease_cooling_ready" -Required $true
        }
        modelHealth = [ordered]@{
            required = $true
            status = Get-ReadinessValue -Target $Target -PropertyName "model_health_ready" -Required $true
        }
    }
}

function New-FallbackEvidence {
    param(
        [object]$Line,
        [object]$Target
    )

    $declaredMode = [string]$Line.Manifest.identity.executionMode
    $observedMode = [string](Get-OptionalProperty -Object $Target -Name "execution_mode")
    if ([string]::IsNullOrWhiteSpace($observedMode)) {
        $observedMode = $declaredMode
    }
    $reason = [string](Get-OptionalProperty -Object $Target -Name "fallback_reason")
    $used = -not [string]::Equals(
        $declaredMode,
        $observedMode,
        [System.StringComparison]::OrdinalIgnoreCase
    )

    return [ordered]@{
        declaredMode = $declaredMode
        observedMode = $observedMode
        used = $used
        reason = if ([string]::IsNullOrWhiteSpace($reason)) { $null } else { ConvertTo-SafeLabel -Value $reason -Fallback "operator-classified" }
        evidenceRequired = $true
    }
}

function New-Classification {
    param(
        [string]$FailureClass,
        [string]$FailureCode,
        [string]$Message
    )

    return [ordered]@{
        failureClass = $FailureClass
        failureCode = $FailureCode
        message = $Message
    }
}

function Assert-SecretFreeJson {
    param([string]$Json)

    $secretValues = @()
    if (-not [string]::IsNullOrWhiteSpace($GatewayApiKey)) {
        $secretValues += $GatewayApiKey
    }
    foreach ($entry in Get-ChildItem Env:) {
        if (
            $entry.Name -match "(?i)(api[_-]?key|access[_-]?token|authorization|cookie|jwt|password|private[_-]?key|refresh[_-]?token|secret|session[_-]?token)" -and
            -not [string]::IsNullOrWhiteSpace([string]$entry.Value) -and
            ([string]$entry.Value).Length -ge 8
        ) {
            $secretValues += [string]$entry.Value
        }
    }
    foreach ($secret in $secretValues | Sort-Object -Unique) {
        if ($Json.Contains($secret)) {
            throw "Refusing to persist provider evidence containing secret material."
        }
    }
    if ($Json -match "(?i)(bearer\s+[a-z0-9._~+/=-]{8,}|basic\s+[a-z0-9+/=]{8,}|akia[0-9a-z]{16}|AIza[0-9a-z_-]{20,}|ya29\.[a-z0-9._-]+|sk-[a-z0-9._-]{8,}|(?:cookie|set-cookie)\s*[:=]|(?:session(?:id|_token)?|refresh_token|api[_-]?key|access[_-]?token|authorization|token|secret|password)\s*[=:])") {
        throw "Refusing to persist provider evidence containing recognizable secret material."
    }
}

function Get-PowerShellArguments {
    param(
        [string]$ScriptPath,
        [string[]]$ScriptArguments
    )

    $arguments = @("-NoProfile")
    if ($env:OS -eq "Windows_NT") {
        $arguments += @("-ExecutionPolicy", "Bypass")
    }
    $arguments += @("-File", $ScriptPath)
    $arguments += $ScriptArguments
    return $arguments
}

function Get-OutputPath {
    param([string]$Path)

    return ([System.IO.Path]::GetFullPath($Path).Replace("\", "/"))
}

function Get-GatewayRelativeArtifactPath {
    param([string]$Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $rootPath = ([System.IO.Path]::GetFullPath($GatewayRoot)).TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
    $comparison = [System.StringComparison]::OrdinalIgnoreCase
    if (-not $fullPath.StartsWith($rootPath, $comparison)) {
        return $null
    }
    $relative = $fullPath.Substring($rootPath.Length).Replace("\", "/")
    if ([string]::IsNullOrWhiteSpace($relative) -or $relative.StartsWith("../") -or $relative.Contains("/../")) {
        return $null
    }
    return $relative
}

$EvidencePath = $null
$ResolvedInventoryPath = $null
$RunArtifactRoot = $null
$LiveCanaryInfo = $null

try {
    if ([string]::IsNullOrWhiteSpace($GatewayApiKey)) {
        $GatewayApiKey = [string]$env:GATEWAY_CANARY_API_KEY
    }
    if ($AllowLiveProviderCalls) {
        if ([string]::IsNullOrWhiteSpace($GatewayBaseUrl)) {
            throw "-GatewayBaseUrl is required when -AllowLiveProviderCalls is set."
        }
        if ([string]::IsNullOrWhiteSpace($GatewayApiKey)) {
            throw "-GatewayApiKey is required when -AllowLiveProviderCalls is set."
        }
        if ([string]::IsNullOrWhiteSpace($TargetsPath)) {
            throw "-TargetsPath is required when -AllowLiveProviderCalls is set."
        }
    }

    $PythonExecutable = Resolve-PythonExecutable
    $PowerShellExecutable = Resolve-PowerShellExecutable
    $observedAt = if ([string]::IsNullOrWhiteSpace($Timestamp)) {
        [DateTimeOffset]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    } else {
        ([DateTimeOffset]::Parse($Timestamp)).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    }
    $runId = [DateTimeOffset]::UtcNow.ToString("yyyyMMddTHHmmssfffZ") + "-" + [guid]::NewGuid().ToString("N").Substring(0, 8)

    if ([string]::IsNullOrWhiteSpace($EvidenceRoot)) {
        $EvidenceRoot = Join-Path $GatewayRoot "docs\evidence"
    }
    if ([string]::IsNullOrWhiteSpace($ArtifactRoot)) {
        $ArtifactRoot = Join-Path $GatewayRoot "target\provider-evidence"
    }
    if ([string]::IsNullOrWhiteSpace($InventoryPath)) {
        $InventoryPath = Join-Path $GatewayRoot "docs\provider-inventory.json"
    }

    $resolvedEvidenceRoot = [System.IO.Path]::GetFullPath($EvidenceRoot)
    $resolvedArtifactRoot = [System.IO.Path]::GetFullPath($ArtifactRoot)
    $ResolvedInventoryPath = [System.IO.Path]::GetFullPath($InventoryPath)
    $RunArtifactRoot = Join-Path $resolvedArtifactRoot $runId
    $EvidencePath = Join-Path $resolvedEvidenceRoot ("gateway-line-evidence-{0}.json" -f $runId)
    New-Item -ItemType Directory -Force -Path $resolvedEvidenceRoot | Out-Null
    New-Item -ItemType Directory -Force -Path $RunArtifactRoot | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ResolvedInventoryPath) | Out-Null

    $liveTargets = if ($AllowLiveProviderCalls) { Read-LiveTargets -Path $TargetsPath } else { @() }
    $lines = Get-LineManifests
    $selectedLines = Select-LineManifests -Lines $lines -LiveTargets $liveTargets

    $manifestStdout = Join-Path $RunArtifactRoot "manifest-validator.json"
    $manifestStderr = Join-Path $RunArtifactRoot "manifest-validator.stderr.log"
    $manifestResult = Invoke-CapturedCommand `
        -Executable $PythonExecutable `
        -CommandArguments @($ManifestValidator, "--as-json") `
        -StdoutPath $manifestStdout `
        -StderrPath $manifestStderr
    if ($manifestResult.exitCode -ne 0) {
        $message = (Get-Content -LiteralPath $manifestStderr -Raw -ErrorAction SilentlyContinue)
        throw "Gateway manifest validation failed: $message"
    }

    $offlineRecords = @()
    foreach ($line in $selectedLines) {
        $safeId = $line.Id -replace "[^a-zA-Z0-9._-]", "_"
        $lineStdout = Join-Path $RunArtifactRoot ("line-{0}.stdout.log" -f $safeId)
        $lineStderr = Join-Path $RunArtifactRoot ("line-{0}.stderr.log" -f $safeId)
        $verifierArguments = @("-LineId", $line.Id)
        if ($SkipCargo) {
            $verifierArguments += "-SkipCargo"
        }
        if ($LibOnly) {
            $verifierArguments += "-LibOnly"
        }
        if (-not [string]::IsNullOrWhiteSpace($SharedCargoTargetDir)) {
            $verifierArguments += @("-SharedCargoTargetDir", [System.IO.Path]::GetFullPath($SharedCargoTargetDir))
        }
        $lineCommandArguments = Get-PowerShellArguments -ScriptPath $LineVerifier -ScriptArguments $verifierArguments
        $lineResult = Invoke-CapturedCommand `
            -Executable $PowerShellExecutable `
            -CommandArguments $lineCommandArguments `
            -StdoutPath $lineStdout `
            -StderrPath $lineStderr

        $linePassed = $lineResult.exitCode -eq 0
        $state = if ($linePassed -and -not $SkipCargo) { "fixture_passed" } else { "metadata_only" }
        $classification = if ($linePassed) {
            if ($SkipCargo) {
                New-Classification `
                    -FailureClass "none" `
                    -FailureCode "offline_metadata_only" `
                    -Message "Manifest and focused line metadata validation passed; Cargo execution was explicitly skipped, so no compile or fixture claim is made."
            } else {
                New-Classification `
                    -FailureClass "none" `
                    -FailureCode "offline_focused_fixture_passed" `
                    -Message "Focused manifest and Cargo verification passed without live provider calls."
            }
        } else {
            New-Classification `
                -FailureClass "gateway_verification" `
                -FailureCode "offline_focused_verification_failed" `
                -Message "Focused Gateway line verification failed; inspect the recorded stdout and stderr artifacts."
        }

        $offlineArtifactPaths = @(
            @(
                (Get-GatewayRelativeArtifactPath -Path $line.Path)
                (Get-GatewayRelativeArtifactPath -Path $lineStdout)
                (Get-GatewayRelativeArtifactPath -Path $lineStderr)
            ) | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) }
        )

        $offlineRecords += [ordered]@{
            lineId = $line.Id
            providerLine = $line.Id
            state = $state
            timestamp = $observedAt
            observedAt = $observedAt
            mode = "offline"
            status = if ($linePassed) { "pass" } else { "fail" }
            executionMode = [string]$line.Manifest.identity.executionMode
            verificationCategory = Get-VerificationCategory -Line $line
            endpointFamily = Get-EndpointFamily -Line $line
            endpointFamilies = @($line.Manifest.capabilities.families | ForEach-Object { [string]$_ })
            credentialSource = Get-ManifestCredentialLabel -Line $line
            classification = $classification
            failureClass = $classification.failureClass
            failureCode = $classification.failureCode
            message = $classification.message
            readiness = New-ReadinessDiagnostics -Line $line -Target $null
            fallback = New-FallbackEvidence -Line $line -Target $null
            artifactPaths = $offlineArtifactPaths
        }
    }

    $records = $offlineRecords
    if ($AllowLiveProviderCalls) {
        $selectedIds = @($selectedLines | ForEach-Object { $_.Id })
        $filteredTargets = @(
            $liveTargets | Where-Object {
                $selectedIds -contains [string](Get-OptionalProperty -Object $_ -Name "provider_line")
            }
        )
        $canaryArtifactRoot = Join-Path $RunArtifactRoot "live-canary"
        New-Item -ItemType Directory -Force -Path $canaryArtifactRoot | Out-Null
        $canaryStdout = Join-Path $RunArtifactRoot "live-canary.stdout.json"
        $canaryStderr = Join-Path $RunArtifactRoot "live-canary.stderr.log"
        $canaryPayload = $null
        $canaryExitCode = 0

        $canaryTargets = @($filteredTargets)
        if ($canaryTargets.Count -gt 0) {
            # The canary requires a targets file. Keep the unredacted request
            # body in a short-lived temp file and remove it before evidence is
            # assembled; durable artifacts receive only the safe projection.
            $filteredTargetsPath = Join-Path `
                ([System.IO.Path]::GetTempPath()) `
                ("gateway-live-canary-targets-{0}.json" -f [guid]::NewGuid().ToString("N"))
            try {
                Write-Utf8NoBom `
                    -Path $filteredTargetsPath `
                    -Content (ConvertTo-JsonText -Payload ([ordered]@{ targets = $canaryTargets }))
                $canaryArguments = @(
                    "-AllowLiveProviderCalls",
                    "-GatewayBaseUrl", $GatewayBaseUrl,
                    "-TargetsPath", $filteredTargetsPath,
                    "-ArtifactRoot", $canaryArtifactRoot,
                    "-AsJson"
                )
                $canaryCommandArguments = Get-PowerShellArguments -ScriptPath $LiveCanary -ScriptArguments $canaryArguments
                $previousCanaryApiKey = [System.Environment]::GetEnvironmentVariable(
                    "GATEWAY_CANARY_API_KEY",
                    [System.EnvironmentVariableTarget]::Process
                )
                try {
                    $env:GATEWAY_CANARY_API_KEY = $GatewayApiKey
                    $canaryResult = Invoke-CapturedCommand `
                        -Executable $PowerShellExecutable `
                        -CommandArguments $canaryCommandArguments `
                        -StdoutPath $canaryStdout `
                        -StderrPath $canaryStderr
                } finally {
                    if ($null -eq $previousCanaryApiKey) {
                        Remove-Item Env:GATEWAY_CANARY_API_KEY -ErrorAction SilentlyContinue
                    } else {
                        $env:GATEWAY_CANARY_API_KEY = $previousCanaryApiKey
                    }
                }
                $canaryExitCode = $canaryResult.exitCode
                try {
                    $canaryPayload = Read-JsonFile -Path $canaryStdout
                } catch {
                    $canaryPayload = [pscustomobject]@{
                        status = "fail"
                        reason = "Live canary did not emit readable JSON: $($_.Exception.Message)"
                        targets = @()
                        summaryPath = $null
                    }
                }
            } finally {
                Remove-Item -LiteralPath $filteredTargetsPath -Force -ErrorAction SilentlyContinue
            }
        } else {
            Write-Utf8NoBom -Path $canaryStdout -Content ""
            Write-Utf8NoBom -Path $canaryStderr -Content ""
            $canaryPayload = [pscustomobject]@{
                status = "skipped"
                reason = "No selected provider line had a live canary target."
                targets = @()
                summaryPath = $null
            }
        }

        $LiveCanaryInfo = [ordered]@{
            status = [string]$canaryPayload.status
            exitCode = $canaryExitCode
            summaryPath = Get-OptionalProperty -Object $canaryPayload -Name "summaryPath"
            stdoutPath = Get-OutputPath -Path $canaryStdout
            stderrPath = Get-OutputPath -Path $canaryStderr
        }

        $records = @()
        foreach ($line in $selectedLines) {
            $offlineRecord = @($offlineRecords | Where-Object { $_.lineId -eq $line.Id })[0]
            $sourceTarget = @(
                $filteredTargets | Where-Object {
                    [string](Get-OptionalProperty -Object $_ -Name "provider_line") -eq $line.Id
                }
            ) | Select-Object -First 1
            $targetResults = @(
                @($canaryPayload.targets) | Where-Object {
                    [string](Get-OptionalProperty -Object $_ -Name "provider_line") -eq $line.Id
                }
            )
            $failedTarget = @($targetResults | Where-Object { [string]$_.status -ne "pass" } | Select-Object -First 1)
            $canaryExecutionFailed = $canaryExitCode -ne 0 -or [string]$canaryPayload.status -ne "pass"
            $fallback = New-FallbackEvidence -Line $line -Target $sourceTarget
            $targetSummary = New-SafeLiveTargetSummary -Target $sourceTarget
            $credentialLabel = ConvertTo-SafeLabel `
                -Value ([string](Get-OptionalProperty -Object $sourceTarget -Name "credential_source")) `
                -Fallback "operator:configured"
            $routeProof = $null
            $observedProvider = $null

            if ($targetResults.Count -eq 0 -and $canaryExecutionFailed) {
                $state = "external_gate"
                $status = "fail"
                $classification = New-Classification `
                    -FailureClass "canary" `
                    -FailureCode "live_canary_execution_failed" `
                    -Message "The live canary tool failed before it produced a provider result for this line; this is not evidence of a missing credential."
            } elseif ($targetResults.Count -eq 0 -and $null -eq $sourceTarget) {
                $state = "credential_missing"
                $status = "fail"
                $classification = New-Classification `
                    -FailureClass "credential" `
                    -FailureCode "live_target_or_credential_missing" `
                    -Message "No live target with a credential source label was available for this provider line."
            } elseif ($targetResults.Count -eq 0) {
                $state = "external_gate"
                $status = "fail"
                $classification = New-Classification `
                    -FailureClass "canary" `
                    -FailureCode "live_canary_target_result_missing" `
                    -Message "The live canary did not return a result for the configured target."
            } elseif ($failedTarget.Count -eq 0) {
                $proofArtifactPath = $null
                $headersPath = [string](Get-OptionalProperty -Object $targetResults[0] -Name "headers_path")
                if (-not [string]::IsNullOrWhiteSpace($headersPath)) {
                    $proofArtifactPath = Get-GatewayRelativeArtifactPath -Path $headersPath
                }
                $routeProof = New-SafeProviderProof `
                    -TargetResult $targetResults[0] `
                    -LineId $line.Id `
                    -ArtifactPath ([string]$proofArtifactPath)
                if ($null -eq $routeProof) {
                    $state = "external_gate"
                    $status = "fail"
                    $classification = New-Classification `
                        -FailureClass "route_proof" `
                        -FailureCode "live_canary_missing_route_proof" `
                        -Message "The canary returned success without an observed provider line and route proof, so live_passed was not claimed."
                } else {
                    $observedProvider = [ordered]@{
                        providerLine = $line.Id
                        source = $routeProof.source
                    }
                    $state = "live_passed"
                    $status = "pass"
                    $classification = New-Classification `
                        -FailureClass "none" `
                        -FailureCode "live_canary_passed" `
                        -Message "The explicitly enabled live provider canary passed with matching observed provider and route proof."
                }
            } else {
                $canaryClass = [string]$failedTarget[0].failure_classification
                switch ($canaryClass) {
                    "credential_or_auth" {
                        $state = "credential_missing"
                        $failureClass = "credential"
                    }
                    "quota_or_rate_limit" {
                        $state = "external_gate"
                        $failureClass = "quota"
                    }
                    "request_or_route_contract" {
                        $state = "external_gate"
                        $failureClass = "upstream_protocol"
                    }
                    "upstream_or_gateway_server" {
                        $state = "external_gate"
                        $failureClass = "upstream"
                    }
                    "network_or_unknown" {
                        $state = "external_gate"
                        $failureClass = "network"
                    }
                    "route_proof_missing_or_mismatch" {
                        $state = "external_gate"
                        $failureClass = "route_proof"
                    }
                    default {
                        $state = "external_gate"
                        $failureClass = "external"
                    }
                }
                $status = "fail"
                $classification = New-Classification `
                    -FailureClass $failureClass `
                    -FailureCode ("live_canary_{0}" -f $(if ([string]::IsNullOrWhiteSpace($canaryClass)) { "unknown" } else { $canaryClass })) `
                    -Message "The live canary reached a classified external or provider gate; inspect canary artifacts."
            }

            if ($fallback.used -and [string]::IsNullOrWhiteSpace([string]$fallback.reason)) {
                $state = "external_gate"
                $status = "fail"
                $classification = New-Classification `
                    -FailureClass "execution_mode" `
                    -FailureCode "silent_fallback_blocked" `
                    -Message "Observed execution mode differed from the manifest without a fallback reason."
            }

            $artifactPaths = @($offlineRecord.artifactPaths)
            $artifactPaths += Get-GatewayRelativeArtifactPath -Path $canaryStdout
            $artifactPaths += Get-GatewayRelativeArtifactPath -Path $canaryStderr
            $summaryPath = Get-OptionalProperty -Object $canaryPayload -Name "summaryPath"
            if (-not [string]::IsNullOrWhiteSpace([string]$summaryPath)) {
                $artifactPaths += Get-GatewayRelativeArtifactPath -Path ([string]$summaryPath)
            }
            foreach ($targetResult in $targetResults) {
                foreach ($propertyName in @("body_path", "headers_path")) {
                    $artifactPath = [string](Get-OptionalProperty -Object $targetResult -Name $propertyName)
                    if (-not [string]::IsNullOrWhiteSpace($artifactPath)) {
                        $artifactPaths += Get-GatewayRelativeArtifactPath -Path $artifactPath
                    }
                }
            }
            $artifactPaths = @(
                $artifactPaths |
                    Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) } |
                    Sort-Object -Unique
            )

            $record = [ordered]@{
                lineId = $line.Id
                providerLine = $line.Id
                state = $state
                timestamp = $observedAt
                observedAt = $observedAt
                mode = "live"
                status = $status
                executionMode = [string]$line.Manifest.identity.executionMode
                verificationCategory = Get-VerificationCategory -Line $line
                endpointFamily = Get-EndpointFamily -Line $line
                endpointFamilies = @($line.Manifest.capabilities.families | ForEach-Object { [string]$_ })
                credentialSource = $credentialLabel
                classification = $classification
                failureClass = $classification.failureClass
                failureCode = $classification.failureCode
                message = $classification.message
                readiness = New-ReadinessDiagnostics -Line $line -Target $sourceTarget
                fallback = $fallback
                targetSummary = $targetSummary
                artifactPaths = $artifactPaths
            }
            if ($null -ne $routeProof) {
                $record["routeProof"] = $routeProof
                $record["observedProvider"] = $observedProvider
            }
            $records += $record
        }
    }

    $evidencePayload = [ordered]@{
        schemaVersion = "gateway-line-evidence/v1"
        runId = $runId
        generatedAt = $observedAt
        mode = $Mode
        allowLiveProviderCalls = [bool]$AllowLiveProviderCalls
        selection = [ordered]@{
            all = [bool]$All
            lineIds = @($selectedLines | ForEach-Object { $_.Id })
            cargoSkipped = [bool]$SkipCargo
            libOnly = [bool]$LibOnly
        }
        records = $records
        artifacts = [ordered]@{
            manifestValidation = Get-GatewayRelativeArtifactPath -Path $manifestStdout
            manifestValidationStderr = Get-GatewayRelativeArtifactPath -Path $manifestStderr
            inventory = Get-GatewayRelativeArtifactPath -Path $ResolvedInventoryPath
            inventoryGeneration = Get-GatewayRelativeArtifactPath -Path (Join-Path $RunArtifactRoot "inventory-generator.json")
            inventoryValidation = Get-GatewayRelativeArtifactPath -Path (Join-Path $RunArtifactRoot "inventory-validator.json")
        }
    }
    $evidenceJson = ConvertTo-JsonText -Payload $evidencePayload
    Assert-SecretFreeJson -Json $evidenceJson
    Write-Utf8NoBom -Path $EvidencePath -Content $evidenceJson

    $inventoryGeneratorStdout = Join-Path $RunArtifactRoot "inventory-generator.json"
    $inventoryGeneratorStderr = Join-Path $RunArtifactRoot "inventory-generator.stderr.log"
    $inventoryGeneratorResult = Invoke-CapturedCommand `
        -Executable $PythonExecutable `
        -CommandArguments @(
            $InventoryGenerator,
            "--output", $ResolvedInventoryPath,
            "--timestamp", $observedAt,
            "--evidence", $EvidencePath,
            "--as-json"
        ) `
        -StdoutPath $inventoryGeneratorStdout `
        -StderrPath $inventoryGeneratorStderr
    if ($inventoryGeneratorResult.exitCode -ne 0) {
        $message = Get-Content -LiteralPath $inventoryGeneratorStderr -Raw -ErrorAction SilentlyContinue
        throw "Provider inventory generation failed: $message"
    }

    $inventoryValidatorStdout = Join-Path $RunArtifactRoot "inventory-validator.json"
    $inventoryValidatorStderr = Join-Path $RunArtifactRoot "inventory-validator.stderr.log"
    $inventoryValidatorResult = Invoke-CapturedCommand `
        -Executable $PythonExecutable `
        -CommandArguments @(
            $InventoryValidator,
            "--inventory", $ResolvedInventoryPath,
            "--as-json"
        ) `
        -StdoutPath $inventoryValidatorStdout `
        -StderrPath $inventoryValidatorStderr
    if ($inventoryValidatorResult.exitCode -ne 0) {
        $message = Get-Content -LiteralPath $inventoryValidatorStderr -Raw -ErrorAction SilentlyContinue
        throw "Provider inventory validation failed: $message"
    }

    $inventoryJson = Get-Content -LiteralPath $ResolvedInventoryPath -Raw -Encoding UTF8
    Assert-SecretFreeJson -Json $inventoryJson

    $failedRecords = @($records | Where-Object { [string]$_.status -ne "pass" })
    $overallStatus = if ($failedRecords.Count -eq 0) { "pass" } else { "fail" }
    $summary = [ordered]@{
        status = $overallStatus
        reason = if ($overallStatus -eq "pass") { $null } else { "$($failedRecords.Count) provider evidence record(s) require attention." }
        mode = $Mode
        allowLiveProviderCalls = [bool]$AllowLiveProviderCalls
        evidencePath = Get-OutputPath -Path $EvidencePath
        inventoryPath = Get-OutputPath -Path $ResolvedInventoryPath
        artifactRoot = Get-OutputPath -Path $RunArtifactRoot
        recordCount = $records.Count
        liveCanary = $LiveCanaryInfo
    }
    Write-RunnerResult -Payload $summary
    if ($overallStatus -ne "pass") {
        exit 1
    }
} catch {
    $failure = [ordered]@{
        status = "fail"
        reason = $_.Exception.Message
        mode = $Mode
        allowLiveProviderCalls = [bool]$AllowLiveProviderCalls
        evidencePath = if ($null -eq $EvidencePath) { $null } else { Get-OutputPath -Path $EvidencePath }
        inventoryPath = if ($null -eq $ResolvedInventoryPath) { $null } else { Get-OutputPath -Path $ResolvedInventoryPath }
        artifactRoot = if ($null -eq $RunArtifactRoot) { $null } else { Get-OutputPath -Path $RunArtifactRoot }
        recordCount = 0
        liveCanary = $LiveCanaryInfo
    }
    Write-RunnerResult -Payload $failure
    exit 1
}
