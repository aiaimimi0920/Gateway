param([Parameter(Mandatory=$true)][string]$RequestPath)
$ErrorActionPreference = 'Stop'
$request = Get-Content -LiteralPath $RequestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $request.archive -Algorithm SHA256).Hash -ine $request.sha256) { throw 'Release checksum mismatch' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$root = [IO.Path]::GetFullPath($request.directory).TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
if (Test-Path -LiteralPath $root) { throw 'Update directory already exists' }
$archive = [IO.Compression.ZipFile]::OpenRead($request.archive)
try {
    if ($archive.Entries.Count -gt 30000) { throw 'Too many archive entries' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [long]$total = 0
    foreach ($entry in $archive.Entries) {
        $name = $entry.FullName
        if ($name -match '(^[/\\]|[:\\]|(^|/)\.\.?(/|$)|(^|/)\.ng(/|$))') { throw 'Unsafe update path' }
        foreach ($part in $name.TrimEnd('/').Split('/')) {
            if (!$part -or $part -match '[. ]$|^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.|$)') { throw 'Unsafe update filename' }
        }
        $path = [IO.Path]::GetFullPath((Join-Path $root $name))
        if (!$path.StartsWith($root,[StringComparison]::OrdinalIgnoreCase) -or !$seen.Add($path.TrimEnd('\','/'))) { throw 'Duplicate or escaping update path' }
        if (($entry.ExternalAttributes -shr 16 -band 0xF000) -eq 0xA000) { throw 'Update archive contains a link' }
        $total += $entry.Length
        if ($total -gt 4GB) { throw 'Extracted update exceeds limit' }
    }
    [IO.Directory]::CreateDirectory($root) | Out-Null
    foreach ($entry in $archive.Entries) {
        $path = [IO.Path]::GetFullPath((Join-Path $root $entry.FullName))
        if ($entry.FullName.EndsWith('/')) { [IO.Directory]::CreateDirectory($path) | Out-Null; continue }
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path)) | Out-Null
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$path,$false)
    }
} finally { $archive.Dispose() }
$manifest = Get-Content -LiteralPath (Join-Path $root 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or $manifest.app -ne 'Gateway' -or $manifest.edition -ne 'desktop' -or $manifest.buildKind -ne 'release' -or $manifest.target -ne 'windows-x64' -or $manifest.versionId -cne $request.version) { throw 'Not a compatible official desktop release' }
$verified = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($line in [IO.File]::ReadAllLines((Join-Path $root 'checksums.sha256'))) {
    if ($line -notmatch '^([0-9a-fA-F]{64})  (.+)$') { throw 'Invalid package checksum record' }
    $hash = $Matches[1]; $relative = $Matches[2]
    if ($relative -match '(^[/\\]|[:\\]|(^|/)\.\.?(/|$))') { throw 'Unsafe checksum path' }
    $path = [IO.Path]::GetFullPath((Join-Path $root $relative))
    if (!$path.StartsWith($root,[StringComparison]::OrdinalIgnoreCase) -or !$verified.Add($path)) { throw 'Invalid checksum path' }
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $hash) { throw 'Package file checksum mismatch' }
}
foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -File -Force) {
    if ($file.Name -eq 'checksums.sha256' -and $file.DirectoryName.TrimEnd('\') -eq $root.TrimEnd('\')) { continue }
    if (!$verified.Contains($file.FullName)) { throw 'Unverified package file' }
}
foreach ($name in @('gateway.exe','gateway-ui.exe','manifest.json')) {
    if (!$verified.Contains([IO.Path]::GetFullPath((Join-Path $root $name)))) { throw 'Missing required release file' }
}
[IO.File]::WriteAllText((Join-Path ([IO.Path]::GetDirectoryName($RequestPath)) 'prepared'), '1', [Text.UTF8Encoding]::new($false))
