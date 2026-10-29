# Node handles package-lock's empty root key on Windows PowerShell 5.1 as well.
function Copy-ChatgptBrowserRuntime([string]$ScriptsRoot, [string]$PackageScripts) {
    & rtk proxy node (Join-Path $PSScriptRoot "verify-browser-runtime.mjs") $ScriptsRoot $PackageScripts
    if ($LASTEXITCODE -ne 0) { throw "Packaged ChatGPT browser runtime import failed." }
}
