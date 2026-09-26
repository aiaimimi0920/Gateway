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
