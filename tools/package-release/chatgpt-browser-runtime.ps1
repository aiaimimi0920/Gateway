# Node copies the locked production closure and checks both browser worker imports.
function Copy-ChatgptBrowserRuntime([string]$ScriptsRoot, [string]$PackageScripts) {
    & rtk proxy node (Join-Path $PSScriptRoot "verify-browser-runtime.mjs") $ScriptsRoot $PackageScripts
    if ($LASTEXITCODE -ne 0) { throw "Packaged browser worker dependency/import check failed." }
}
