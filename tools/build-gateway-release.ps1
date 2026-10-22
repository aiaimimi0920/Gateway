[CmdletBinding()]
param(
    [switch]$DryRun,
    [ValidateSet("api", "web", "desktop", "all")][string]$Edition = "desktop",
    [switch]$UseInstalledDependencies
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$desktopRoot = Join-Path $repoRoot "apps\\desktop"
$maxNpmCiAttempts = 3
$defaultCargoBuildJobs = 1
$scriptsRoot = Join-Path $repoRoot "scripts"
$minimumNodeVersion = [version]"22.22.0"

. (Join-Path $PSScriptRoot "release-editions.ps1")
. (Join-Path $PSScriptRoot "package-release/source-provenance.ps1")
$editions = if ($Edition -eq "all") { @("api", "web", "desktop") } else { @($Edition) }

if ($DryRun) {
    foreach ($selectedEdition in $editions) { Get-GatewayEditionBuildPlan -Edition $selectedEdition }
    exit 0
}
if ($Edition -eq "all") {
    # Shared Cargo/frontend output has one coordinator and is never built in parallel.
    foreach ($selectedEdition in $editions) {
        & $PSCommandPath -Edition $selectedEdition -UseInstalledDependencies:$UseInstalledDependencies
    }
    return
}
$layout = Get-GatewayEditionLayout -Root $repoRoot -Edition $Edition

function Test-RetryableNpmCiFailure {
    param([string]$Output)

    return ($Output -match "EPERM|EBUSY|ENOTEMPTY")
}

function Initialize-GatewayBuildThrottle {
    param([int]$DefaultJobs = $defaultCargoBuildJobs)

    if ($DefaultJobs -ne 1) {
        throw "Gateway release builds require exactly one Cargo build job."
    }
    $env:CARGO_BUILD_JOBS = "1"
    $env:CARGO_INCREMENTAL = "0"
    Write-Output "[gateway-release] enforcing CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0"
    return $DefaultJobs
}

function Invoke-GatewayReleaseStep {
    param(
        [string]$Name,
        [scriptblock]$Action
    )

    $startedAt = Get-Date
    Write-Output "[gateway-release] BEGIN $Name"
    try {
        & $Action
        $elapsed = (Get-Date) - $startedAt
        Write-Output "[gateway-release] END $Name ($([Math]::Round($elapsed.TotalSeconds, 2))s)"
    } catch {
        $elapsed = (Get-Date) - $startedAt
        Write-Output "[gateway-release] FAIL $Name ($([Math]::Round($elapsed.TotalSeconds, 2))s)"
        throw
    }
}

function Resolve-NativeExecutable {
    param([Parameter(Mandatory = $true)][string]$Command)

    if (Test-Path -LiteralPath $Command -PathType Leaf) {
        return [System.IO.Path]::GetFullPath($Command)
    }

    $resolved = @(Get-Command $Command -All -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandType -eq "Application" } |
        Select-Object -First 1)
    if ($resolved.Count -eq 0) {
        $resolved = @(Get-Command $Command -ErrorAction SilentlyContinue | Select-Object -First 1)
    }
    if ($resolved.Count -eq 0) {
        throw "Native command was not found: $Command"
    }
    return $resolved[0].Source
}

function Write-NewCaptureLines {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][ref]$LastLineIndex
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return @()
    }

    try {
        $lines = @(Get-Content -LiteralPath $Path -Encoding UTF8 -ErrorAction Stop)
    } catch {
        return @()
    }

    if ($LastLineIndex.Value -ge $lines.Count) {
        return @()
    }

    $newLines = @($lines[$LastLineIndex.Value..($lines.Count - 1)])
    $LastLineIndex.Value = $lines.Count
    foreach ($line in $newLines) {
        [Console]::Out.WriteLine($line)
    }
    [Console]::Out.Flush()
    return $newLines
}

function Invoke-NativeCommandCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$WorkingDirectory
    )

    $resolvedCommand = Resolve-NativeExecutable -Command $Command
    $captureRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
        "gateway-release-{0}" -f [guid]::NewGuid().ToString("N")
    )
    $stdoutPath = Join-Path $captureRoot "stdout.log"
    $stderrPath = Join-Path $captureRoot "stderr.log"
    $stdoutLineIndex = 0
    $stderrLineIndex = 0
    New-Item -ItemType Directory -Path $captureRoot -Force | Out-Null

    try {
        $startInfo = @{
            FilePath = $resolvedCommand
            ArgumentList = $Arguments
            WorkingDirectory = $WorkingDirectory
            RedirectStandardOutput = $stdoutPath
            RedirectStandardError = $stderrPath
            PassThru = $true
            WindowStyle = "Hidden"
        }
        $process = Start-Process @startInfo
        # Retain the Windows process handle before polling HasExited. Otherwise
        # PowerShell can lose ExitCode and [int]$null would falsely report success.
        $null = $process.Handle
        $stdoutLines = @()
        $stderrLines = @()
        while (-not $process.HasExited) {
            $stdoutLines += @(Write-NewCaptureLines -Path $stdoutPath -LastLineIndex ([ref]$stdoutLineIndex))
            $stderrLines += @(Write-NewCaptureLines -Path $stderrPath -LastLineIndex ([ref]$stderrLineIndex))
            Start-Sleep -Milliseconds 200
        }
        $process.WaitForExit()
        if ($null -eq $process.ExitCode) {
            throw "Native build command completed without an exit code: $Command"
        }
        $stdoutLines += @(Write-NewCaptureLines -Path $stdoutPath -LastLineIndex ([ref]$stdoutLineIndex))
        $stderrLines += @(Write-NewCaptureLines -Path $stderrPath -LastLineIndex ([ref]$stderrLineIndex))

        return [pscustomobject]@{
            ExitCode = [int]$process.ExitCode
            Output = @($stdoutLines + $stderrLines)
        }
    } finally {
        if (Test-Path -LiteralPath $captureRoot) {
            Remove-Item -LiteralPath $captureRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Invoke-NpmCiWithRetry {
    param(
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$ComponentName,
        [int]$MaxAttempts = $maxNpmCiAttempts
    )

    if ($UseInstalledDependencies) {
        $installed = Invoke-NativeCommandCapture -Command "npm" `
            -Arguments @("ls", "--depth=0") -WorkingDirectory $WorkingDirectory
        if ($installed.ExitCode -ne 0) { throw "Installed dependencies are invalid for $ComponentName" }
        return
    }

    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        Write-Output "[gateway-release] npm ci for $ComponentName attempt $attempt/$MaxAttempts"

        $npmResult = Invoke-NativeCommandCapture `
            -Command "npm" `
            -Arguments @("ci", "--no-audit", "--no-fund") `
            -WorkingDirectory $WorkingDirectory
        $npmOutput = @($npmResult.Output)
        $npmExitCode = [int]$npmResult.ExitCode

        if ($npmExitCode -eq 0) {
            return
        }

        $combinedOutput = ($npmOutput | ForEach-Object { $_.ToString() }) -join "`n"
        if (($attempt -lt $MaxAttempts) -and (Test-RetryableNpmCiFailure -Output $combinedOutput)) {
            $delaySeconds = [Math]::Min(10, 2 * $attempt)
            Write-Warning "npm ci failed with a transient Windows file-lock style error; retrying in $delaySeconds second(s)."
            Start-Sleep -Seconds $delaySeconds
            continue
        }

        throw "npm ci failed for $ComponentName after $attempt attempt(s)"
    }
}

function Invoke-NpmProductionAudit {
    param(
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$ComponentName
    )

    $auditResult = Invoke-NativeCommandCapture `
        -Command "npm" `
        -Arguments @("run", "audit:prod") `
        -WorkingDirectory $WorkingDirectory
    $auditOutput = @($auditResult.Output)
    $auditExitCode = [int]$auditResult.ExitCode
    if ($auditExitCode -ne 0) {
        throw "npm production dependency audit failed for $ComponentName"
    }
}

function Assert-MinimumNodeVersion {
    param([version]$MinimumVersion = $minimumNodeVersion)

    $nodeResult = Invoke-NativeCommandCapture `
        -Command "node" `
        -Arguments @("-p", "process.versions.node") `
        -WorkingDirectory $repoRoot
    $nodeOutput = @($nodeResult.Output)
    if ([int]$nodeResult.ExitCode -ne 0) {
        throw "Unable to determine the installed Node.js version. Gateway release builds require Node.js >= $MinimumVersion."
    }

    $nodeVersionText = @($nodeOutput | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1)
    if ($nodeVersionText.Count -ne 1) {
        throw "Node.js did not report a version. Gateway release builds require Node.js >= $MinimumVersion."
    }

    try {
        $nodeVersion = [version]$nodeVersionText[0].Trim()
    } catch {
        throw "Unable to parse Node.js version '$($nodeVersionText[0])'. Gateway release builds require Node.js >= $MinimumVersion."
    }

    if ($nodeVersion -lt $MinimumVersion) {
        throw "Node.js >= $MinimumVersion is required for Gateway release builds; found $nodeVersion."
    }
    Write-Output "[gateway-release] using Node.js $nodeVersion (minimum $MinimumVersion)"
}

function Write-ArtifactHashSummary {
    param([string[]]$Paths)

    foreach ($path in $Paths) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing build artifact: $path"
        }

        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        Write-Output "[gateway-release] artifact $($item.Name) bytes=$($item.Length) sha256=$hash"
    }
}

function Get-RelativeUnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $baseRoot = [System.IO.Path]::GetFullPath($BasePath).TrimEnd("\", "/")
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    if ([string]::Equals($baseRoot, $pathFull.TrimEnd("\", "/"), [System.StringComparison]::OrdinalIgnoreCase)) {
        return "."
    }
    $baseFull = $baseRoot + [System.IO.Path]::DirectorySeparatorChar
    $baseUri = New-Object System.Uri($baseFull)
    $pathUri = New-Object System.Uri($pathFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("\", "/")
}

function Write-BuildProvenance {
    param(
        [Parameter(Mandatory = $true)][string]$GatewayRoot,
        [Parameter(Mandatory = $true)][object]$SourceTreeState,
        [Parameter(Mandatory = $true)][string[]]$ArtifactPaths
    )

    $records = @()
    foreach ($path in $ArtifactPaths) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing build artifact while writing provenance: $path"
        }
        $item = Get-Item -LiteralPath $path
        $records += [ordered]@{
            path = Get-RelativeUnixPath -BasePath $GatewayRoot -Path $path
            bytes = [int64]$item.Length
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }

    $payload = [ordered]@{
        schemaVersion = 1
        app = "Gateway"
        edition = $Edition
        capabilities = [ordered]@{ managementUi = $layout.WebUi; desktopUi = $layout.DesktopUi }
        builtAt = [DateTimeOffset]::UtcNow.ToString("o")
        sourceTreeFingerprint = $SourceTreeState.fingerprint
        sourceTreeDirty = $SourceTreeState.dirty
        sourceTree = [ordered]@{
            algorithm = $SourceTreeState.algorithm
            fingerprint = $SourceTreeState.fingerprint
            fileCount = $SourceTreeState.fileCount
            dirty = $SourceTreeState.dirty
        }
        artifacts = @($records)
    }
    $provenancePath = $layout.ProvenancePath
    $parent = Split-Path -Parent $provenancePath
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [System.IO.File]::WriteAllText(
        $provenancePath,
        (($payload | ConvertTo-Json -Depth 10) + [Environment]::NewLine),
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "[gateway-release] build provenance $provenancePath sourceTreeFingerprint=$($SourceTreeState.fingerprint)"
}

$previousCargoBuildJobs = $env:CARGO_BUILD_JOBS
$previousCargoIncremental = $env:CARGO_INCREMENTAL

Push-Location -LiteralPath $repoRoot
try {
    Assert-MinimumNodeVersion
    Initialize-GatewayBuildThrottle | Out-Null

    Invoke-GatewayReleaseStep -Name "install browser worker dependencies" -Action {
        Invoke-NpmCiWithRetry `
            -WorkingDirectory $scriptsRoot `
            -ComponentName "Gateway browser workers" `
            -MaxAttempts $maxNpmCiAttempts
    }

    Invoke-GatewayReleaseStep -Name "audit production browser worker dependencies" -Action {
        Invoke-NpmProductionAudit `
            -WorkingDirectory $scriptsRoot `
            -ComponentName "Gateway browser workers"
    }

    if ($layout.WebUi) {
    Push-Location -LiteralPath $desktopRoot
    try {
        Invoke-GatewayReleaseStep -Name "install desktop dependencies" -Action {
            Invoke-NpmCiWithRetry `
                -WorkingDirectory $desktopRoot `
                -ComponentName "Gateway desktop UI" `
                -MaxAttempts $maxNpmCiAttempts
        }

        Invoke-GatewayReleaseStep -Name "audit production desktop dependencies" -Action {
            Invoke-NpmProductionAudit `
                -WorkingDirectory $desktopRoot `
                -ComponentName "Gateway desktop UI"
        }

        Invoke-GatewayReleaseStep -Name "typecheck desktop UI" -Action {
            $typecheckResult = Invoke-NativeCommandCapture `
                -Command "npm" `
                -Arguments @("run", "typecheck") `
                -WorkingDirectory $desktopRoot
            $typecheckOutput = @($typecheckResult.Output)
            $typecheckExitCode = [int]$typecheckResult.ExitCode
            if ($typecheckExitCode -ne 0) {
                throw "npm typecheck failed for Gateway desktop UI"
            }
        }

        Invoke-GatewayReleaseStep -Name "build browser console web assets" -Action {
            $webBuildResult = Invoke-NativeCommandCapture `
                -Command "npm" `
                -Arguments @("run", "build:web") `
                -WorkingDirectory $desktopRoot
            $webBuildOutput = @($webBuildResult.Output)
            $webBuildExitCode = [int]$webBuildResult.ExitCode
            if ($webBuildExitCode -ne 0) {
                throw "npm build:web failed for Gateway browser console"
            }
        }

    } finally { Pop-Location }
    }

        Invoke-GatewayReleaseStep -Name "build headless gateway" -Action {
            $previousPrebuiltWebUi = $env:GATEWAY_PREBUILT_WEB_UI
            try {
                $env:GATEWAY_PREBUILT_WEB_UI = if ($layout.WebUi) { "1" } else { "0" }
                # Keep the large backend's release link within the local memory budget.
                $cargoArguments = @("build", "--locked", "--release", "--bin", "gateway", "--config", "profile.release.package.gateway.codegen-units=16")
                if (-not $layout.WebUi) {
                    $cargoArguments += @("--no-default-features", "--features", "runtime-defaults")
                }
                $cargoResult = Invoke-NativeCommandCapture `
                    -Command "cargo" `
                    -Arguments $cargoArguments `
                    -WorkingDirectory $repoRoot
                $cargoOutput = @($cargoResult.Output)
                $cargoExitCode = [int]$cargoResult.ExitCode
                if ($cargoExitCode -ne 0) {
                    throw "cargo build failed for gateway"
                }
            } finally {
                if ([string]::IsNullOrEmpty($previousPrebuiltWebUi)) {
                    Remove-Item Env:GATEWAY_PREBUILT_WEB_UI -ErrorAction SilentlyContinue
                } else {
                    $env:GATEWAY_PREBUILT_WEB_UI = $previousPrebuiltWebUi
                }
            }
        }

    if ($layout.DesktopUi) {
        Invoke-GatewayReleaseStep -Name "build desktop Tauri shell" -Action {
            $tauriResult = Invoke-NativeCommandCapture `
                -Command "npm" `
                -Arguments @("run", "tauri", "--", "build", "--no-bundle") `
                -WorkingDirectory $desktopRoot
            $tauriOutput = @($tauriResult.Output)
            $tauriExitCode = [int]$tauriResult.ExitCode
            if ($tauriExitCode -ne 0) {
                throw "tauri build failed for Gateway desktop UI"
            }
        }
    }

    $headlessExe = Join-Path $repoRoot "target\\release\\gateway.exe"
    $uiExe = Join-Path $repoRoot "apps\\desktop\\src-tauri\\target\\release\\gateway-ui.exe"

    if (-not (Test-Path -LiteralPath $headlessExe -PathType Leaf)) {
        throw "Missing headless build artifact: $headlessExe"
    }
    if ($layout.DesktopUi -and -not (Test-Path -LiteralPath $uiExe -PathType Leaf)) {
        throw "Missing UI build artifact: $uiExe"
    }

    if (-not $layout.DesktopUi) {
        New-Item -ItemType Directory -Path (Split-Path -Parent $layout.GatewayPath) -Force | Out-Null
        Copy-Item -LiteralPath $headlessExe -Destination $layout.GatewayPath
    }
    Write-ArtifactHashSummary -Paths $layout.ArtifactPaths
    $sourceTreeState = Get-SourceTreeState -Root $repoRoot
    Write-BuildProvenance `
        -GatewayRoot $repoRoot `
        -SourceTreeState $sourceTreeState `
        -ArtifactPaths $layout.ArtifactPaths
} finally {
    Pop-Location
    if ($null -eq $previousCargoBuildJobs) {
        Remove-Item Env:CARGO_BUILD_JOBS -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_BUILD_JOBS = $previousCargoBuildJobs
    }
    if ($null -eq $previousCargoIncremental) {
        Remove-Item Env:CARGO_INCREMENTAL -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_INCREMENTAL = $previousCargoIncremental
    }
}
