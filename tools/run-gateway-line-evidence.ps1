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

# Dot-source private functions to retain the runner's existing invocation scope.
. (Join-Path $PSScriptRoot "line-evidence\process.ps1")
. (Join-Path $PSScriptRoot "line-evidence\selection.ps1")
. (Join-Path $PSScriptRoot "line-evidence\projection.ps1")
. (Join-Path $PSScriptRoot "line-evidence\offline.ps1")

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

    $offlineRecords = @(Invoke-OfflineLineVerification `
        -SelectedLines $selectedLines `
        -RunArtifactRoot $RunArtifactRoot `
        -SkipCargo ([bool]$SkipCargo) `
        -LibOnly ([bool]$LibOnly) `
        -SharedCargoTargetDir $SharedCargoTargetDir `
        -LineVerifier $LineVerifier `
        -PowerShellExecutable $PowerShellExecutable `
        -ObservedAt $observedAt)

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
