function Invoke-OfflineLineVerification {
    param(
        [object[]]$SelectedLines,
        [string]$RunArtifactRoot,
        [bool]$SkipCargo,
        [bool]$LibOnly,
        [string]$SharedCargoTargetDir,
        [string]$LineVerifier,
        [string]$PowerShellExecutable,
        [string]$ObservedAt
    )

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
    return $offlineRecords
}
