import json
import pathlib
import subprocess
import tempfile
import unittest

from powershell_test_utils import powershell_executable


class GatewayReleaseCandidateGateTests(unittest.TestCase):
    def test_release_candidate_script_reports_skipped_heavy_steps_as_json(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "verify-gateway-release-candidate.ps1"

        with tempfile.TemporaryDirectory() as log_root:
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(script_path),
                    "-SkipPythonTests",
                    "-SkipLineMatrix",
                    "-SkipRustAllTargets",
                    "-SkipReleaseBuild",
                    "-SkipBrowserWorkers",
                    "-LogRoot",
                    log_root,
                    "-AsJson",
                ],
                cwd=repo_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

        self.assertEqual(
            result.returncode,
            0,
            msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "pass")
        steps = {step["name"]: step for step in payload["steps"]}
        self.assertEqual(steps["manifest-validator"]["status"], "pass")
        self.assertEqual(steps["live-provider-canary-preflight"]["status"], "pass")
        self.assertEqual(steps["python-tests"]["status"], "skipped")
        self.assertEqual(steps["line-matrix"]["status"], "skipped")
        self.assertEqual(steps["rust-all-targets"]["status"], "skipped")
        self.assertEqual(steps["release-build"]["status"], "skipped")
        self.assertEqual(steps["browser-worker-tests"]["status"], "skipped")

    def test_gateway_package_workflow_contains_line_matrix_release_gate(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        workflow = (
            repo_root / ".github" / "workflows" / "ci.yml"
        ).read_text(encoding="utf-8")

        self.assertIn("gateway-line-matrix", workflow)
        self.assertIn("verify-gateway-line.ps1", workflow)
        self.assertIn("invoke-gateway-live-provider-canary.ps1", workflow)
        self.assertIn("-All", workflow)
        self.assertIn("-LibOnly", workflow)
        self.assertIn("-SharedCargoTargetDir", workflow)

    def test_release_candidate_runtime_smoke_forces_standalone_role(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script = (repo_root / "tools" / "verify-gateway-release-candidate.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn("Invoke-GatewayRuntimeSmoke", script)
        self.assertIn("Invoke-LiveProviderCanaryPreflight", script)
        self.assertIn("invoke-gateway-live-provider-canary.ps1", script)
        self.assertIn("live-provider-canary-preflight", script)
        self.assertIn("ConvertFrom-Json", script)
        self.assertIn("allowLiveProviderCalls", script)
        self.assertIn('"skipped"', script)
        self.assertIn("$env:GATEWAY_API_KEY", script)
        self.assertIn("$env:GATEWAY_RUNTIME_ROLE", script)
        self.assertIn("$env:RUST_LOG", script)
        self.assertIn("curl.exe", script)
        self.assertIn("--noproxy", script)
        self.assertIn('$PSBoundParameters.ContainsKey("Body")', script)
        self.assertIn("Authorization", script)
        self.assertIn('"standalone"', script)
        self.assertNotIn('"bash"', script)
        self.assertNotIn("tests/e2e.sh", script)

    def test_release_candidate_live_canary_preflight_cannot_enable_live_calls(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script = (repo_root / "tools" / "verify-gateway-release-candidate.ps1").read_text(
            encoding="utf-8"
        )
        preflight_start = script.index("function Invoke-LiveProviderCanaryPreflight")
        next_function_start = script.index("function Invoke-SmokeHttpRequest", preflight_start)
        preflight = script[preflight_start:next_function_start]

        self.assertIn("invoke-gateway-live-provider-canary.ps1", preflight)
        self.assertNotIn("-AllowLiveProviderCalls", preflight)
        self.assertNotIn("-GatewayBaseUrl", preflight)
        self.assertNotIn("-GatewayApiKey", preflight)
        self.assertNotIn("-TargetsPath", preflight)


if __name__ == "__main__":
    unittest.main()
