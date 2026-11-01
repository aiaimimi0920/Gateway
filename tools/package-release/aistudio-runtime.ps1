# No credential files are accepted. Node validates the opt-in runtime manifest and source/hash bindings.
function Copy-AistudioHttpRuntime([string]$ScriptsRoot, [string]$PackageScripts, [string]$BuildKind) {
    & rtk proxy node (Join-Path $PSScriptRoot "aistudio-runtime.mjs") $ScriptsRoot $PackageScripts $BuildKind
    if ($LASTEXITCODE -ne 0) { throw "AI Studio native runtime package validation failed." }
}
