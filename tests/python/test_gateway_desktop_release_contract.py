import json
import unittest
from pathlib import Path


GATEWAY_ROOT = Path(__file__).resolve().parents[2]
DESKTOP_ROOT = GATEWAY_ROOT / "apps" / "desktop"


class GatewayDesktopReleaseContractTests(unittest.TestCase):
    def test_release_plan_includes_headless_and_ui_gateway_exes(self):
        release_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = release_script.read_text(encoding="utf-8")
        self.assertIn("cargo build --locked --release --bin gateway", script_text)
        self.assertIn('"target\\\\release\\\\gateway.exe"', script_text)
        self.assertIn(
            '"apps\\\\desktop\\\\src-tauri\\\\target\\\\release\\\\gateway-ui.exe"',
            script_text,
        )

    def test_gateway_owned_release_builder_builds_ui_and_headless(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        self.assertTrue(build_script.exists(), "Gateway-owned release builder must exist")
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("cargo build --locked --release --bin gateway", script_text)
        self.assertIn("Push-Location -LiteralPath $desktopRoot", script_text)
        self.assertIn("npm ci", script_text)
        self.assertIn("npm run typecheck", script_text)
        self.assertIn(
            "npm run tauri --prefix apps/desktop -- build --no-bundle", script_text
        )
        self.assertIn(
            '-Arguments @("run", "tauri", "--", "build", "--no-bundle")',
            script_text,
        )
        self.assertIn("gateway-ui.exe", script_text)

    def test_gateway_owned_release_builder_installs_desktop_dependencies_before_headless_build(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        install_index = script_text.index(
            'Invoke-GatewayReleaseStep -Name "install desktop dependencies"'
        )
        cargo_index = script_text.index(
            'Invoke-GatewayReleaseStep -Name "build headless gateway"'
        )
        self.assertLess(
            install_index,
            cargo_index,
            "Desktop npm dependencies must be installed before cargo build triggers build.rs",
        )

    def test_gateway_owned_release_builder_retries_transient_npm_ci_file_locks(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("function Invoke-NpmCiWithRetry", script_text)
        self.assertIn("$maxNpmCiAttempts = 3", script_text)
        self.assertIn("EPERM", script_text)
        self.assertIn("EBUSY", script_text)
        self.assertIn("ENOTEMPTY", script_text)
        self.assertIn("Start-Sleep", script_text)
        self.assertIn("[string]$ComponentName", script_text)
        self.assertIn("npm ci failed for $ComponentName after", script_text)

    def test_gateway_owned_release_builder_runs_native_commands_via_process_capture(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("function Invoke-NativeCommandCapture", script_text)
        self.assertIn("Start-Process", script_text)
        self.assertIn("RedirectStandardOutput", script_text)
        self.assertIn("RedirectStandardError", script_text)
        self.assertIn("PassThru = $true", script_text)
        self.assertIn('WindowStyle = "Hidden"', script_text)
        self.assertIn('-Command "cargo"', script_text)
        self.assertIn('-Command "npm"', script_text)

    def test_gateway_owned_release_builder_streams_long_running_child_output(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("function Write-NewCaptureLines", script_text)
        self.assertIn("while (-not $process.HasExited)", script_text)
        self.assertIn(
            'Write-NewCaptureLines -Path $stdoutPath -LastLineIndex ([ref]$stdoutLineIndex)',
            script_text,
        )
        self.assertIn(
            'Write-NewCaptureLines -Path $stderrPath -LastLineIndex ([ref]$stderrLineIndex)',
            script_text,
        )
        self.assertIn("Start-Sleep -Milliseconds 200", script_text)

    def test_gateway_owned_release_builder_prefers_application_wrappers_for_start_process(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("Get-Command $Command -All", script_text)
        self.assertIn('$_.CommandType -eq "Application"', script_text)
        self.assertIn('if ($resolved.Count -eq 0)', script_text)
        self.assertNotIn('if ($null -eq $resolved)', script_text)

    def test_gateway_owned_release_builder_has_stage_logs_and_artifact_hash_summary(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("function Invoke-GatewayReleaseStep", script_text)
        self.assertIn("[gateway-release] BEGIN", script_text)
        self.assertIn("[gateway-release] END", script_text)
        self.assertIn("function Write-ArtifactHashSummary", script_text)
        self.assertIn("Get-FileHash", script_text)
        self.assertIn("sha256=", script_text)

    def test_gateway_release_builder_restores_cargo_environment_after_completion_or_failure(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("$previousCargoBuildJobs = $env:CARGO_BUILD_JOBS", script_text)
        self.assertIn("$previousCargoIncremental = $env:CARGO_INCREMENTAL", script_text)
        self.assertIn("Remove-Item Env:CARGO_BUILD_JOBS", script_text)
        self.assertIn("Remove-Item Env:CARGO_INCREMENTAL", script_text)
        self.assertIn("$env:CARGO_BUILD_JOBS = $previousCargoBuildJobs", script_text)
        self.assertIn("$env:CARGO_INCREMENTAL = $previousCargoIncremental", script_text)

    def test_gateway_owned_ui_release_smoke_script_validates_artifacts_and_optional_launch(self):
        smoke_script = GATEWAY_ROOT / "tools" / "smoke-gateway-ui-release.ps1"
        self.assertTrue(smoke_script.exists(), "Gateway UI release smoke script must exist")
        script_text = smoke_script.read_text(encoding="utf-8")

        for required in [
            "param(",
            "$ReleaseDir",
            "$LaunchUi",
            "manifest.json",
            "checksums.sha256",
            "gateway.exe",
            "gateway-ui.exe",
            "Get-FileHash",
            "bytesMatch",
            "shaMatch",
            "Start-Process",
            "-WindowStyle Hidden",
            "Stop-Process",
            "gateway-ui",
            "gateway",
            "may auto-start",
        ]:
            self.assertIn(required, script_text)

    def test_gateway_ui_release_smoke_only_detects_and_cleans_release_owned_sidecars(self):
        smoke_script = GATEWAY_ROOT / "tools" / "smoke-gateway-ui-release.ps1"
        script_text = smoke_script.read_text(encoding="utf-8")

        for required in [
            "function Get-ReleaseOwnedGatewayProcesses",
            "Get-CimInstance",
            "Win32_Process",
            "ExecutablePath",
            "CommandLine",
            "$expectedGatewayPath",
            "Test-PathOwnedByRelease",
            "ReleaseRoot = $ReleaseRoot",
        ]:
            self.assertIn(required, script_text)

        self.assertNotIn("function Get-ProcessIdsByName", script_text)
        self.assertNotIn('Get-Process -Name $Name', script_text)
        self.assertNotIn('Get-ProcessIdsByName -Name "gateway"', script_text)

        self.assertIn("function Read-ChecksumIndex", script_text)
        self.assertIn("function Assert-PackagedReleaseIntegrity", script_text)
        self.assertIn("newGatewayProcessIds", script_text)
        self.assertIn("Stop-Process -Id", script_text)
        self.assertIn("finally", script_text)

    def test_gateway_owned_release_builder_lets_tauri_run_frontend_build_once(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        tauri_conf = DESKTOP_ROOT / "src-tauri" / "tauri.conf.json"
        script_text = build_script.read_text(encoding="utf-8")
        config = json.loads(tauri_conf.read_text(encoding="utf-8"))

        self.assertEqual(config["build"]["beforeBuildCommand"], "npm run build:tauri")
        self.assertNotIn("& npm run build", script_text)
        self.assertIn('-Arguments @("run", "typecheck")', script_text)

    def test_vitest_excludes_playwright_e2e_specs(self):
        vitest_config = DESKTOP_ROOT / "vitest.config.ts"
        config_text = vitest_config.read_text(encoding="utf-8")

        self.assertIn("exclude", config_text)
        self.assertIn("e2e/**/*.spec.ts", config_text)
        self.assertIn("playwright test", (DESKTOP_ROOT / "package.json").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
