import pathlib
import re
import unittest


def read_release_candidate_script(repo_root):
    return "\n".join(
        (repo_root / "tools" / name).read_text(encoding="utf-8")
        for name in (
            "verify-gateway-release-candidate.ps1",
            "verify-gateway-release-candidate.runtime.ps1",
        )
    )


class GatewayPowerShellPortabilityTests(unittest.TestCase):
    def test_python_test_helper_prefers_pwsh_before_windows_powershell(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        helper = (repo_root / "tests" / "python" / "powershell_test_utils.py").read_text(
            encoding="utf-8"
        )

        self.assertIn('candidates = ["pwsh", "powershell"]', helper)
        self.assertNotIn('if os.name == "nt"', helper)

    def test_python_gateway_tests_do_not_hardcode_windows_powershell(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        python_tests = repo_root / "tests" / "python"
        pattern = re.compile(r"subprocess\.run\(\s*\[\s*\"powershell\"", re.DOTALL)

        offenders = []
        for path in python_tests.glob("test_*.py"):
            if path.name == pathlib.Path(__file__).name:
                continue
            if pattern.search(path.read_text(encoding="utf-8")):
                offenders.append(path.relative_to(repo_root).as_posix())

        self.assertEqual(offenders, [])

    def test_release_candidate_gate_resolves_nested_powershell_executable(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script = read_release_candidate_script(repo_root)
        preflight_start = script.index("function Invoke-LiveProviderCanaryPreflight")
        smoke_start = script.index("function Invoke-SmokeHttpRequest", preflight_start)
        preflight = script[preflight_start:smoke_start]
        line_matrix_start = script.index('Invoke-OptionalCommand -Name "line-matrix"')
        rust_targets_start = script.index('Invoke-OptionalCommand -Name "rust-all-targets"')
        line_matrix = script[line_matrix_start:rust_targets_start]

        self.assertIn("Resolve-PowerShellExecutable", script)
        self.assertIn("$PowerShellExecutable", preflight)
        self.assertIn("$PowerShellExecutable", line_matrix)
        self.assertNotIn('"powershell"', preflight)
        self.assertNotIn('"powershell"', line_matrix)

    def test_windows_workflows_use_runner_local_temp_storage(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        workflows = repo_root / ".github" / "workflows"

        for name in ("ci.yml", "build-windows.yml"):
            workflow = (workflows / name).read_text(encoding="utf-8")
            with self.subTest(workflow=name):
                self.assertIn("RUNNER_TEMP", workflow)
                self.assertIn('"TEMP=$gatewayTemp"', workflow)
                self.assertIn('"TMP=$gatewayTemp"', workflow)


if __name__ == "__main__":
    unittest.main()
