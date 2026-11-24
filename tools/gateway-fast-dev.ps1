[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet("check", "test", "retest")]
    [string]$Mode = "check",

    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$Rest = @()
)
# Fast inner-loop helper for Gateway provider development:
# - check  : isolated target/nightly-check with +nightly -Z threads parallel frontend
#            so typechecking 560K+ lines uses multi-core queries without touching target/debug.
# - test   : process-isolated cargo nextest with automatic dual-session CPU/RAM job throttling.
# - retest : zero-Cargo ~0.1s direct execution of the newest compiled neuro_gateway test binary.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))

function Get-AdaptiveConcurrency {
    $logical = [Math]::Max(1, [Environment]::ProcessorCount)
    $defaultJobs = [Math]::Min(6, $logical)
    try {
        $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
        $freeGb = [double]$os.FreePhysicalMemory / 1MB
        $otherRustc = @(Get-CimInstance Win32_Process -Filter "Name = 'rustc.exe'" -ErrorAction SilentlyContinue |
            Where-Object {
                $cmd = [string]$_.CommandLine
                -not [string]::IsNullOrWhiteSpace($cmd) -and
                $cmd.IndexOf($repoRoot, [StringComparison]::OrdinalIgnoreCase) -lt 0
            })
        if ($otherRustc.Count -gt 0 -or $freeGb -lt 6.0) {
            return [Math]::Max(2, [Math]::Min(3, $defaultJobs))
        }
    } catch {
    }
    return $defaultJobs
}

Push-Location -LiteralPath $repoRoot
try {
    $jobs = Get-AdaptiveConcurrency
    switch ($Mode) {
        "check" {
            $prevTarget = [Environment]::GetEnvironmentVariable("CARGO_TARGET_DIR", "Process")
            $prevFlags = [Environment]::GetEnvironmentVariable("RUSTFLAGS", "Process")
            $prevColor = [Environment]::GetEnvironmentVariable("CARGO_TERM_COLOR", "Process")
            try {
                [Environment]::SetEnvironmentVariable("CARGO_TARGET_DIR", (Join-Path $repoRoot "target\nightly-check"), "Process")
                [Environment]::SetEnvironmentVariable("RUSTFLAGS", "-Z threads=$jobs", "Process")
                [Environment]::SetEnvironmentVariable("CARGO_TERM_COLOR", "never", "Process")
                $cargoArgs = @(
                    "+nightly", "check", "--locked", "--offline", "--lib",
                    "--no-default-features", "--features", "runtime-defaults",
                    "--jobs", "$jobs"
                )
                if ($Rest.Count -gt 0) { $cargoArgs += $Rest }
                Write-Host "[gateway-fast-dev] cargo $($cargoArgs -join ' ') (RUSTFLAGS=-Z threads=$jobs)"
                & cargo @cargoArgs
                exit $LASTEXITCODE
            } finally {
                [Environment]::SetEnvironmentVariable("CARGO_TARGET_DIR", $prevTarget, "Process")
                [Environment]::SetEnvironmentVariable("RUSTFLAGS", $prevFlags, "Process")
                [Environment]::SetEnvironmentVariable("CARGO_TERM_COLOR", $prevColor, "Process")
            }
        }
        "test" {
            $expr = if ($Rest.Count -gt 0 -and -not [string]::IsNullOrWhiteSpace($Rest[0])) { $Rest[0] } else { "" }
            $extra = if ($Rest.Count -gt 1) { $Rest[1..($Rest.Count - 1)] } else { @() }
            $nextestArgs = @(
                "nextest", "run", "--locked", "--offline", "--lib",
                "--no-default-features", "--features", "runtime-defaults",
                "--cargo-jobs", "$jobs", "--test-threads", "$jobs"
            )
            if (-not [string]::IsNullOrWhiteSpace($expr)) {
                $nextestArgs += @("-E", $expr)
            }
            if ($extra.Count -gt 0) { $nextestArgs += $extra }
            Write-Host "[gateway-fast-dev] cargo $($nextestArgs -join ' ')"
            $env:CARGO_TERM_COLOR = "never"
            & cargo @nextestArgs
            exit $LASTEXITCODE
        }
        "retest" {
            $filterArgs = @($Rest | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
            if ($filterArgs.Count -eq 0) {
                throw "Pass a test name filter to 'retest' (e.g. npm run test:rust:retest --prefix scripts sap_aicore_service_key) so all 6000+ unit tests do not run accidentally."
            }
            $depsDir = Join-Path $repoRoot "target\debug\deps"
            if (-not (Test-Path -LiteralPath $depsDir -PathType Container)) {
                throw "No compiled test artifacts under target/debug/deps. Run 'test' mode once first."
            }
            $binary = Get-ChildItem -LiteralPath $depsDir -Filter "neuro_gateway-*.exe" -File |
                Sort-Object LastWriteTimeUtc -Descending |
                Select-Object -First 1
            if ($null -eq $binary) {
                throw "No compiled neuro_gateway-*.exe test binary found in target/debug/deps."
            }
            $binArgs = @("--color", "never") + $filterArgs
            Write-Host "[gateway-fast-dev] retest $($binary.Name) $($binArgs -join ' ')"
            & $binary.FullName @binArgs
            exit $LASTEXITCODE
        }
    }
} finally {
    Pop-Location
}
