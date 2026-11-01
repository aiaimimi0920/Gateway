param(
    [Parameter(Mandatory)][string]$ReferenceRoot,
    [Parameter(Mandatory)][string]$SpiderMonkeyRoot,
    [Parameter(Mandatory)][string]$LlvmBin,
    [Parameter(Mandatory)][string]$WorkRoot,
    [ValidateRange(1,4)][int]$Jobs = 2
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$revision = '9b694a6b7c6439db1767a577994dfcc87a3cf588'
foreach ($path in @($ReferenceRoot,$SpiderMonkeyRoot,$LlvmBin,$WorkRoot)) {
    if ($path -notmatch '^[A-Za-z]:[\\/]') { throw 'Explicit absolute non-UNC paths required.' }
}
$head = & git -C $ReferenceRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $head.Trim() -ne $revision) { throw 'AIStudio2API source revision mismatch.' }
& git -C $ReferenceRoot diff --exit-code HEAD -- internal/waa
if ($LASTEXITCODE -ne 0) { throw 'AIStudio2API WAA sources have local changes.' }
$work = Join-Path $WorkRoot ('aistudio-proof-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $work 'internal') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $work 'cmd\gateway-proof') -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $ReferenceRoot 'internal\waa') -Destination (Join-Path $work 'internal') -Recurse
Copy-Item -LiteralPath (Join-Path $ReferenceRoot 'LICENSE') -Destination (Join-Path $work 'AIStudio2API-LICENSE')
$sources = Join-Path $PSScriptRoot 'aistudio-proof'
Copy-Item -Path (Join-Path $sources '*.go') -Destination (Join-Path $work 'cmd\gateway-proof')
Copy-Item -LiteralPath (Join-Path $sources 'go.mod'),(Join-Path $sources 'go.sum') -Destination $work
$settings = @{
    CC = Join-Path $LlvmBin 'x86_64-w64-mingw32-clang.exe'
    CXX = Join-Path $LlvmBin 'x86_64-w64-mingw32-clang++.exe'
    CGO_CXXFLAGS = "-I$SpiderMonkeyRoot\include"
    CGO_LDFLAGS = "-L$SpiderMonkeyRoot\lib"
    CGO_ENABLED = '1'; GOTOOLCHAIN = 'go1.27.0'; GOMAXPROCS = "$Jobs"
    GOCACHE = Join-Path $WorkRoot 'go-cache'
    GOMODCACHE = Join-Path $WorkRoot 'go-mod-cache'
    GOTMPDIR = Join-Path $WorkRoot 'go-tmp'
}
New-Item -ItemType Directory -Path $settings.GOTMPDIR -Force | Out-Null
$saved = @{}
try {
    foreach ($name in $settings.Keys) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name,'Process')
        [Environment]::SetEnvironmentVariable($name,$settings[$name],'Process')
    }
    Push-Location $work
    try {
        & go build -mod=readonly -p $Jobs -o (Join-Path $work 'aistudio-proof.exe') ./cmd/gateway-proof
        if ($LASTEXITCODE -ne 0) { throw 'AI Studio native proof build failed.' }
    } finally { Pop-Location }
    $exe = Join-Path $work 'aistudio-proof.exe'
    $receipt = @{ buildKind='devtest'; dependencyKind='aistudio-native-proof'; browserRequired=$false;
        sourceRevision=$revision; executable=$exe; sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash;
        toolchain='go1.27.0'; sourceFiles=@(Get-ChildItem -LiteralPath $sources -File | ForEach-Object {
            @{name=$_.Name;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}
        }) }
    [IO.File]::WriteAllText((Join-Path $work 'build-receipt.json'),($receipt | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    $receipt | ConvertTo-Json -Depth 6
} finally {
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name,$saved[$name],'Process') }
}
