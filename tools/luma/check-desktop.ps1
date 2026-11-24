[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ReleaseDir,
    [Parameter(Mandatory = $true)][string]$Output
)
# Reuse the package smoke owner; observe its isolated desktop sidecar over HTTP.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Write-Smoke([string]$Message) { Write-Host "[luma-desktop-smoke] $Message" }
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$release = (Resolve-Path -LiteralPath $ReleaseDir).Path
$evidence = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $evidence) { throw "Output must be new: $evidence" }
$ui = Join-Path $release 'gateway-ui.exe'
$backend = Join-Path $release 'gateway.exe'
function Get-OwnedProcesses {
    @(Get-CimInstance Win32_Process -Filter "Name = 'gateway.exe' OR Name = 'gateway-ui.exe'" |
        Where-Object { $_.ExecutablePath -eq $ui -or $_.ExecutablePath -eq $backend })
}
if (@(Get-OwnedProcesses).Count -ne 0) { throw 'This package is already running; use an idle package.' }
. (Join-Path $repo 'tools/smoke-gateway-packaged-runtime.runtime.ps1')
$port = Get-FreeTcpPort
$data = Join-Path $evidence 'data'
New-Item -ItemType Directory -Path $data | Out-Null
$encoding = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $data 'connection.json'),
    (@{ mode = 'local'; localPort = $port; serverUrl = '' } | ConvertTo-Json), $encoding)
$previous = @{}
foreach ($entry in @(Get-ChildItem Env:GATEWAY_*)) { $previous[$entry.Name] = $entry.Value }
foreach ($name in @('DATABASE_URL', 'PORT', 'TEMP', 'TMP', 'TMPDIR')) {
    $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$runner = $null
$observed = $null
$failure = $null
try {
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $null, 'Process') }
    $env:GATEWAY_DATA_DIR = $data
    $temporary = Join-Path $evidence 'temp'
    New-Item -ItemType Directory -Path $temporary | Out-Null
    $env:TEMP = $temporary
    $env:TMP = $temporary
    $env:TMPDIR = $temporary
    $script = Join-Path $repo 'tools/smoke-gateway-ui-release.ps1'
    $runner = Start-Process -FilePath 'powershell.exe' -WorkingDirectory $repo -WindowStyle Hidden -PassThru `
        -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$script`"", '-ReleaseDir', "`"$release`"", '-LaunchUi', '-LaunchSeconds', '30') `
        -RedirectStandardOutput (Join-Path $evidence 'command.log') `
        -RedirectStandardError (Join-Path $evidence 'stderr.log')
    $null = $runner.Handle
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    while (-not $runner.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        if ($null -eq $observed) {
            $owned = @(Get-OwnedProcesses)
            $child = @($owned | Where-Object { $_.ExecutablePath -eq $backend })
            $shell = @($owned | Where-Object { $_.ExecutablePath -eq $ui })
            if ($child.Count -eq 1 -and $shell.Count -eq 1 -and $child[0].ParentProcessId -eq $shell[0].ProcessId) {
                $ready = Invoke-SmokeHttpRequest -Method GET -Uri "http://127.0.0.1:$port/readyz" -RetryCount 1
                if ($ready.StatusCode -eq 200) {
                    $body = $ready.Content | ConvertFrom-Json
                    if ($body.storage_backend -ne 'sqlite' -or $body.degraded -or $body.redis_required) {
                        throw 'Desktop sidecar did not select healthy isolated SQLite storage.'
                    }
                    $page = Invoke-SmokeHttpRequest -Method GET -Uri "http://127.0.0.1:$port/ui/" -RetryCount 1
                    Assert-SmokeStatus -Name 'Desktop sidecar console' -Expected 200 -Response $page
                    $observed = @{ uiPid = $shell[0].ProcessId; backendPid = $child[0].ProcessId; readiness = $body; consoleStatus = 200 }
                }
            }
        }
        Start-Sleep -Milliseconds 250
    }
    if (-not $runner.HasExited) { throw 'Desktop smoke exceeded its startup and observation budget.' }
    $runner.WaitForExit()
    if ($runner.ExitCode -ne 0) { throw "Package desktop smoke failed ($($runner.ExitCode)); inspect command.log and stderr.log." }
    if ($null -eq $observed) { throw 'No healthy release-owned desktop sidecar was observed.' }
    if (-not (Test-Path -LiteralPath (Join-Path $data 'local/state/runtime.sqlite3'))) {
        throw 'Desktop sidecar did not write SQLite state under the isolated data directory.'
    }
    if (@(Get-OwnedProcesses).Count -ne 0) { throw 'Package smoke left an owned process running.' }
} catch {
    $failure = $_.Exception.Message
} finally {
    # Only this idle package was eligible at entry; never stop another release's processes.
    if ($null -ne $runner) {
        if (-not $runner.HasExited) { Stop-Process -Id $runner.Id -Force }
        $runner.Dispose()
    }
    foreach ($process in (Get-OwnedProcesses)) { Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue }
    [Environment]::SetEnvironmentVariable('GATEWAY_DATA_DIR', $null, 'Process')
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
}
$report = [ordered]@{
    passed = ($null -eq $failure); failure = $failure; port = $port; storageBackend = 'sqlite'
    releaseDir = $release; observed = $observed; supplierCalls = 0; nativeBusinessClicksVerified = $false
    uiSha256 = (Get-FileHash -LiteralPath $ui -Algorithm SHA256).Hash.ToLowerInvariant()
    backendSha256 = (Get-FileHash -LiteralPath $backend -Algorithm SHA256).Hash.ToLowerInvariant()
    remainingOwnedProcesses = @(Get-OwnedProcesses).Count
}
[IO.File]::WriteAllText((Join-Path $evidence 'result.json'), ($report | ConvertTo-Json -Depth 8), $encoding)
if ($null -ne $failure) { throw $failure }
Write-Host "[luma-desktop-smoke] PASS: $evidence"
