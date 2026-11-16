param([Parameter(Mandatory=$true)][string]$RequestPath)
$ErrorActionPreference = 'Stop'
$request = Get-Content -LiteralPath $RequestPath -Raw -Encoding UTF8 | ConvertFrom-Json
$encoding = [Text.UTF8Encoding]::new($false)
$process = $null
$oldExited = $false
function Save-Result([bool]$success, [string]$message) {
    $path = Join-Path $request.root 'updates/last-update-result.json'
    [IO.File]::WriteAllText($path,(@{success=$success;message=$message}|ConvertTo-Json -Compress),$encoding)
}
try {
$arm = [IO.Path]::ChangeExtension($RequestPath,'armed')
$deadline = [DateTime]::UtcNow.AddSeconds(180)
while (!(Test-Path -LiteralPath $arm)) {
    if ([DateTime]::UtcNow -gt $deadline) { throw 'Update handoff was not armed. No application was started.' }
    Start-Sleep -Milliseconds 200
}
$deadline = [DateTime]::UtcNow.AddSeconds(90)
while (Get-Process -Id $request.pid -ErrorAction SilentlyContinue) {
    if ([DateTime]::UtcNow -gt $deadline) { throw 'The old desktop did not exit. No update was installed.' }
    Start-Sleep -Milliseconds 200
}
$oldExited = $true
$env:GATEWAY_DATA_DIR = $request.root
$env:GATEWAY_UPDATE_RECEIPT = $request.receipt
$env:GATEWAY_RELEASE_PAYLOAD_ROOT = $request.package
$arguments = @('--gateway-updated','--gateway-data-dir',('"' + $request.root + '"'))
$process = Start-Process -FilePath (Join-Path $request.package 'gateway-ui.exe') -ArgumentList $arguments -WorkingDirectory $request.package -WindowStyle Hidden -PassThru
$started = $request.receipt
$deadline = [DateTime]::UtcNow.AddSeconds(60)
while (!(Test-Path -LiteralPath $started)) {
    $process.Refresh()
    if ($process.HasExited) {
        throw 'The new desktop exited during startup.'
    }
    if ([DateTime]::UtcNow -gt $deadline) { throw 'The new desktop did not acknowledge startup. The active version was not changed.' }
    Start-Sleep -Milliseconds 200
}
$receipt = Get-Content -LiteralPath $started -Raw -Encoding UTF8 | ConvertFrom-Json
$process.Refresh()
if ($process.HasExited -or $receipt.pid -ne $process.Id -or $receipt.ready -ne $true) { throw 'Invalid startup acknowledgement.' }
$active = Join-Path $request.root 'updates/active.json'
$pending = $active + '.pending'
[IO.File]::WriteAllText($pending,(@{package=$request.package}|ConvertTo-Json -Compress),[Text.UTF8Encoding]::new($false))
if ([IO.File]::Exists($active)) { [IO.File]::Replace($pending,$active,$null) } else { [IO.File]::Move($pending,$active) }
Save-Result $true 'The new desktop acknowledged startup.'
} catch {
    $message = $_.Exception.Message
    if ($null -ne $process) {
        [IO.File]::WriteAllText([IO.Path]::ChangeExtension($request.receipt,'cancel'),'cancel',$encoding)
        # Never forcibly kill the application or start parallel managed backends.
        $process.WaitForExit(30000) | Out-Null
        $process.Refresh()
    }
    if ($oldExited -and ($null -eq $process -or $process.HasExited)) {
        $env:GATEWAY_UPDATE_RECEIPT = $null
        $env:GATEWAY_RELEASE_PAYLOAD_ROOT = [IO.Path]::GetDirectoryName($request.previous)
        try {
            Start-Process -FilePath $request.previous -ArgumentList $arguments -WorkingDirectory $env:GATEWAY_RELEASE_PAYLOAD_ROOT -WindowStyle Hidden | Out-Null
            $message += ' The previous version was reopened.'
        } catch { $message += ' Cannot reopen the previous version: ' + $_.Exception.Message }
    } else { $message += ' No parallel instance was started.' }
    Save-Result $false $message
    exit 1
}
