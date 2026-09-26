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
