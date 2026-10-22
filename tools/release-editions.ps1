# One release contract for build, package and provenance. Desktop keeps legacy paths.
function Get-GatewayEditionLayout {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [ValidateSet("api", "web", "desktop")][string]$Edition = "desktop",
        [ValidateSet("release", "devtest")][string]$BuildKind = "release"
    )
    $desktop = $Edition -eq "desktop"
    $directory = if ($desktop) { "target/release" } else { "target/release-editions/$Edition" }
    $gateway = "$directory/gateway.exe"
    $ui = "apps/desktop/src-tauri/target/release/gateway-ui.exe"
    if ($BuildKind -eq "devtest") {
        $directory = "target/devtest-editions/$Edition"
        $gateway = "$directory/gateway.exe"
        $ui = "$directory/gateway-ui.exe"
    }
    $artifacts = @($gateway)
    $executables = @("gateway.exe")
    if ($desktop) { $artifacts += $ui; $executables += "gateway-ui.exe" }
    return [pscustomobject]@{
        Edition = $Edition
        BuildKind = $BuildKind
        WebUi = $Edition -ne "api"
        DesktopUi = $desktop
        GatewayRelativePath = $gateway
        GatewayPath = Join-Path $Root $gateway
        UiPath = Join-Path $Root $ui
        ProvenanceRelativePath = "$directory/gateway-build-provenance.json"
        ProvenancePath = Join-Path $Root "$directory/gateway-build-provenance.json"
        ArtifactRelativePaths = $artifacts
        ArtifactPaths = @($artifacts | ForEach-Object { Join-Path $Root $_ })
        ExecutableNames = $executables
    }
}

function Get-GatewayEditionBuildPlan {
    param([ValidateSet("api", "web", "desktop")][string]$Edition)
    "[gateway-release] edition=$Edition"
    'Node.js >= 22.22.0'
    'npm ci --prefix scripts --no-audit --no-fund (retry up to 3 attempts on transient Windows file locks)'
    'npm run audit:prod --prefix scripts'
    if ($Edition -ne "api") {
        'npm ci --prefix apps/desktop --no-audit --no-fund (retry up to 3 attempts on transient Windows file locks)'
        'npm run audit:prod --prefix apps/desktop'
        'npm run typecheck --prefix apps/desktop'
        'npm run build:web --prefix apps/desktop'
    }
    if ($Edition -eq "api") {
        'cargo build --locked --release --bin gateway --no-default-features --features runtime-defaults'
    } else { 'cargo build --locked --release --bin gateway' }
    if ($Edition -eq "desktop") { 'npm run tauri --prefix apps/desktop -- build --no-bundle' }
}
