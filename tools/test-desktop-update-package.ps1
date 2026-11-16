param([string]$ScratchRoot = 'C:/Users/Public/nas_home/AI/GameEditor/linshi')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$root = Join-Path $ScratchRoot ('gateway-update-contract-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($root) | Out-Null
$script = Join-Path $PSScriptRoot '../apps/desktop/src-tauri/src/updates/prepare.ps1'
$utf8 = [Text.UTF8Encoding]::new($false)
function Fixture([string]$name, [string]$kind = 'release', [bool]$unsafe = $false, [bool]$badFileHash = $false) {
    $directory = Join-Path $root $name
    $payload = Join-Path $directory 'payload'
    [IO.Directory]::CreateDirectory($payload) | Out-Null
    [IO.File]::WriteAllText((Join-Path $payload 'gateway.exe'),'synthetic-backend-not-executable',$utf8)
    [IO.File]::WriteAllText((Join-Path $payload 'gateway-ui.exe'),'synthetic-desktop-not-executable',$utf8)
    $manifest = @{schemaVersion=1;app='Gateway';edition='desktop';buildKind=$kind;target='windows-x64';versionId='V1.2.3'}
    [IO.File]::WriteAllText((Join-Path $payload 'manifest.json'),($manifest|ConvertTo-Json),$utf8)
    $checksums = @()
    foreach ($file in Get-ChildItem -LiteralPath $payload -File) {
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($badFileHash -and $file.Name -eq 'gateway.exe') { $hash = '0' * 64 }
        $checksums += "$hash  $($file.Name)"
    }
    [IO.File]::WriteAllText((Join-Path $payload 'checksums.sha256'),(($checksums -join "`n") + "`n"),$utf8)
    $zip = Join-Path $directory 'release.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($payload,$zip)
    if ($unsafe) {
        $archive = [IO.Compression.ZipFile]::Open($zip,[IO.Compression.ZipArchiveMode]::Update)
        try { $entry = $archive.CreateEntry('../escaped.txt'); $writer = [IO.StreamWriter]::new($entry.Open()); $writer.Write('must-not-escape'); $writer.Dispose() }
        finally { $archive.Dispose() }
    }
    $request = @{archive=$zip;directory=(Join-Path $directory 'package');sha256=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash;version='V1.2.3'}
    $path = Join-Path $directory 'request.json'
    [IO.File]::WriteAllText($path,($request|ConvertTo-Json),$utf8)
    return $path
}
function Run-Case([string]$request, [bool]$success) {
    $directory = Split-Path -Parent $request
    $process = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',('"'+$script+'"'),'-RequestPath',('"'+$request+'"')) `
        -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $directory 'stdout.log') -RedirectStandardError (Join-Path $directory 'stderr.log')
    if (($process.ExitCode -eq 0) -ne $success) { throw "Unexpected verifier outcome: $directory" }
    if ((Test-Path -LiteralPath (Join-Path $directory 'prepared')) -ne $success) { throw 'Verification marker is incorrect' }
}
Run-Case (Fixture 'valid') $true
Run-Case (Fixture 'devtest' 'devtest') $false
Run-Case (Fixture 'traversal' 'release' $true) $false
Run-Case (Fixture 'file-hash' 'release' $false $true) $false
$wrong = Fixture 'archive-hash'
$request = Get-Content -LiteralPath $wrong -Raw | ConvertFrom-Json
$request.sha256 = '0' * 64
[IO.File]::WriteAllText($wrong,($request|ConvertTo-Json),$utf8)
Run-Case $wrong $false
if (Test-Path -LiteralPath (Join-Path $root 'escaped.txt')) { throw 'Archive escaped its directory' }
Write-Output "5 update package cases passed. Synthetic fixtures retained at $root. No application was installed or started."
