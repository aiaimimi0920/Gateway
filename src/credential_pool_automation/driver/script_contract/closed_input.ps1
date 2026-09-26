$ErrorActionPreference = 'Stop'
[IO.File]::WriteAllText($PSCommandPath + '.pid', [string]$PID, [Text.UTF8Encoding]::new($false))
$stream = [Console]::OpenStandardInput()
$field = $stream.GetType().GetField('_handle', [Reflection.BindingFlags]'Instance,NonPublic')
if ($null -eq $field) { throw 'Native stdin handle is unavailable' }
# Console streams do not own the standard handle; take ownership only in this fixture.
$handle = $field.GetValue($stream).DangerousGetHandle()
$owner = New-Object Microsoft.Win32.SafeHandles.SafeFileHandle($handle, $true)
$owner.Dispose()
Start-Sleep -Seconds 12
exit 99
