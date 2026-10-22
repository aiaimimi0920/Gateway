[CmdletBinding()]
param(
    [ValidateSet("api", "web", "desktop")][string]$Edition = "desktop",
    [ValidateRange(1, 32)][int]$Jobs = [Math]::Max(1, [Math]::Min(4, [Environment]::ProcessorCount)),
    [switch]$DryRun
)
# Separate Cargo profiles preserve release artifacts and let Cargo own safe reuse.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
. (Join-Path $PSScriptRoot "release-editions.ps1")
. (Join-Path $PSScriptRoot "package-release/artifact-copy.ps1")
. (Join-Path $PSScriptRoot "package-release/source-provenance.ps1")
function Get-RelativeUnixPath([string]$BasePath, [string]$Path) {
    $base = [IO.Path]::GetFullPath($BasePath).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $full = [IO.Path]::GetFullPath($Path)
    if ($full.TrimEnd('\', '/').Equals($base.TrimEnd('\', '/'), [StringComparison]::OrdinalIgnoreCase)) { return "." }
    if (-not $full.StartsWith($base, [StringComparison]::OrdinalIgnoreCase)) { throw "Path outside source root: $full" }
    return $full.Substring($base.Length).Replace('\', '/')
}
$layout = Get-GatewayEditionLayout -Root $root -Edition $Edition -BuildKind devtest
$cargo = @("build", "--locked", "--profile", "devtest", "--bin", "gateway", "--jobs", "$Jobs", "--target-dir", "target")
if ($Edition -eq "api") { $cargo += @("--no-default-features", "--features", "runtime-defaults") }
if ($DryRun) {
    "buildKind=devtest edition=$Edition jobs=$Jobs incremental=1 lto=false opt-level=1"
    if ($layout.WebUi) { "npm run build:web --prefix apps/desktop" }
    "cargo $($cargo -join ' ')"
    if ($layout.DesktopUi) {
        "npm run build:tauri --prefix apps/desktop"
        "cargo build --locked --profile devtest --manifest-path apps/desktop/src-tauri/Cargo.toml --features tauri/custom-protocol --jobs $Jobs"
    }
    "artifacts=$($layout.ArtifactRelativePaths -join ',')"
    return
}
function Invoke-Step([string]$Command, [string[]]$Arguments) {
    Write-Host "[gateway-devtest] $Command $($Arguments -join ' ')"
    & rtk proxy $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "devtest command failed ($LASTEXITCODE): $Command" }
}
$settings = @{
    CARGO_BUILD_JOBS = "$Jobs"
    CARGO_INCREMENTAL = "1"
    GATEWAY_PREBUILT_WEB_UI = $(if ($layout.WebUi) { "1" } else { "0" })
}
$previous = @{}
foreach ($key in $settings.Keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, "Process") }
# Reject ambient redirection/optimization overrides: the recorded profile must be real.
foreach ($name in @("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")) {
    if ([Environment]::GetEnvironmentVariable($name, "Process")) { throw "Unset $name before this reproducible devtest build." }
}
if (@(Get-ChildItem Env:CARGO_PROFILE_DEVTEST_*).Count -gt 0) { throw "Unset CARGO_PROFILE_DEVTEST_* overrides." }
$before = Get-SourceTreeState -Root $root
Push-Location -LiteralPath $root
try {
    foreach ($key in $settings.Keys) { [Environment]::SetEnvironmentVariable($key, $settings[$key], "Process") }
    # Reuse installed lockfile dependencies; never run npm ci (which deletes node_modules).
    if ($layout.WebUi) {
        if (-not (Test-Path "$root/apps/desktop/node_modules")) { throw "Install the locked desktop dependencies first." }
        Invoke-Step "npm" @("run", "build:web", "--prefix", "apps/desktop")
    }
    Invoke-Step "cargo" $cargo
    if ($layout.DesktopUi) {
        # Tauri CLI has no custom-profile flag in the installed version. Build the
        # configured assets explicitly, then use its normal Rust build.rs/custom protocol.
        $tauri = Get-Content "$root/apps/desktop/src-tauri/tauri.conf.json" -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($tauri.build.beforeBuildCommand -ne "npm run build:tauri" -or $tauri.build.frontendDist -ne "../dist/tauri") {
            throw "Tauri asset contract changed; update devtest builder before proceeding."
        }
        Invoke-Step "npm" @("run", "build:tauri", "--prefix", "apps/desktop")
        Invoke-Step "cargo" @("build", "--locked", "--profile", "devtest", "--manifest-path", "apps/desktop/src-tauri/Cargo.toml", "--features", "tauri/custom-protocol", "--jobs", "$Jobs", "--target-dir", "apps/desktop/src-tauri/target")
    }
    $after = Get-SourceTreeState -Root $root
    if ($before.fingerprint -ne $after.fingerprint) { throw "Source changed during build; no devtest receipt published." }
    New-Item -ItemType Directory -Path (Split-Path $layout.GatewayPath -Parent) -Force | Out-Null
    Copy-Item "$root/target/devtest/gateway.exe" $layout.GatewayPath
    if ($layout.DesktopUi) { Copy-Item "$root/apps/desktop/src-tauri/target/devtest/gateway-ui.exe" $layout.UiPath }
    $records = @(foreach ($path in $layout.ArtifactPaths) {
        $file = Get-Item -LiteralPath $path
        [ordered]@{ path = Get-RelativeUnixPath -BasePath $root -Path $path; bytes = $file.Length
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    $receipt = [ordered]@{
        schemaVersion = 1; app = "Gateway"; edition = $Edition; buildKind = "devtest"
        capabilities = @{ managementUi = $layout.WebUi; desktopUi = $layout.DesktopUi }
        builtAt = [DateTimeOffset]::UtcNow.ToString("o")
        sourceTreeFingerprint = $after.fingerprint; sourceTreeDirty = $after.dirty; sourceTree = $after
        buildSettings = @{ profile = "devtest"; jobs = $Jobs; incremental = $true; lto = $false; optLevel = 1 }
        artifactReuse = "Cargo fingerprint/incremental cache; no unchecked binary reuse"
        artifacts = $records
    }
    [IO.File]::WriteAllText($layout.ProvenancePath, ($receipt | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    Write-Host "[gateway-devtest] ready: $($layout.ProvenancePath)"
} finally {
    Pop-Location
    foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], "Process") }
}
