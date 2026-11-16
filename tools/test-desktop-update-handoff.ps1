param()
$ErrorActionPreference = 'Stop'
$root = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-handoff-' + [Guid]::NewGuid().ToString('N'))
$encoding = [Text.UTF8Encoding]::new($false)
$helper = Join-Path $PSScriptRoot '../apps/desktop/src-tauri/src/updates/restart.ps1'
foreach ($scenario in @('success','start-failure','invalid-receipt')) {
    $folder = Join-Path $root $scenario
    New-Item -ItemType Directory -Path (Join-Path $folder 'updates') -Force | Out-Null
    $request = Join-Path $folder 'request.json'
    $receipt = Join-Path $folder 'started-123'
    $value = @{pid=987654;root=$folder;package=(Join-Path $folder 'package');previous=(Join-Path $folder 'old/gateway-ui.exe');receipt=$receipt}
    [IO.File]::WriteAllText($request,($value|ConvertTo-Json),$encoding)
    [IO.File]::WriteAllText((Join-Path $folder 'request.armed'),'armed',$encoding)
    # The helper is exercised in a child PowerShell with process operations replaced.
    # No executable, backend, or user database is opened by these fixtures.
    $wrapper = @'
param($Helper,$Request,$Scenario)
$global:fixtureCalls = 0
$global:fixtureScenario = $Scenario
$global:fixtureRequest = Get-Content -LiteralPath $Request -Raw | ConvertFrom-Json
function Get-Process { param($Id,$ErrorAction) return $null }
function Start-Process {
    param($FilePath,$ArgumentList,$WorkingDirectory,$WindowStyle,[switch]$PassThru)
    $global:fixtureCalls++
    $r = $global:fixtureRequest
    [IO.File]::AppendAllText((Join-Path $r.root 'calls'),$FilePath + "`n")
    if ($global:fixtureCalls -eq 1 -and $global:fixtureScenario -eq 'start-failure') { throw 'Synthetic launch failure' }
    if (!$PassThru) { return }
    $id = if ($global:fixtureScenario -eq 'invalid-receipt') { 4321 } else { 1234 }
    [IO.File]::WriteAllText($r.receipt,(@{pid=$id;ready=$true}|ConvertTo-Json))
    $process = [pscustomobject]@{Id=1234;HasExited=$false}
    $process | Add-Member ScriptMethod Refresh { }
    $process | Add-Member ScriptMethod WaitForExit { param($timeout) $this.HasExited=$true; return $true }
    return $process
}
& $Helper -RequestPath $Request
'@
    $script = Join-Path $folder 'wrapper.ps1'
    [IO.File]::WriteAllText($script,$wrapper,$encoding)
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $script $helper $request $scenario
    $result = Get-Content -LiteralPath (Join-Path $folder 'updates/last-update-result.json') -Raw | ConvertFrom-Json
    $calls = @(Get-Content -LiteralPath (Join-Path $folder 'calls'))
    if ($scenario -eq 'success') {
        if (!$result.success -or $calls.Count -ne 1 -or !(Test-Path -LiteralPath (Join-Path $folder 'updates/active.json'))) { throw 'Successful handoff contract failed' }
    } else {
        if ($result.success -or $calls.Count -ne 2 -or $calls[1] -ne $value.previous -or (Test-Path -LiteralPath (Join-Path $folder 'updates/active.json'))) { throw "Rollback contract failed: $scenario" }
    }
}
Write-Host "3 isolated handoff cases passed. No application was launched. Fixtures: $root"
