import json
import pathlib
import subprocess
import tempfile
import unittest

from powershell_test_utils import powershell_executable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class VerifyGatewayLineExecutedTests(unittest.TestCase):
    def run_verifier(self, summaries, exit_codes=None, *, skip_cargo=False):
        with tempfile.TemporaryDirectory(prefix="gateway-line-count-") as directory:
            root = pathlib.Path(directory)
            tools = root / "tools"
            tools.mkdir()
            script = tools / "verify-gateway-line.ps1"
            script.write_bytes((GATEWAY_ROOT / "tools/verify-gateway-line.ps1").read_bytes())
            manifests = root / "manifests/lines"
            manifests.mkdir(parents=True)
            manifest = {
                "id": "fixture-line",
                "identity": {"serviceProviderKey": "fixture", "providerSurfaceKey": "fixture",
                             "protocolProfile": "fixture", "adapter": "fixture"},
                "compilation": {"lineFeature": "line-fixture", "familyCommonFeatures": []},
                "verification": {"useNoDefaultFeatures": True,
                                 "recommendedCargoTargetDirSlug": "fixture",
                                 "focusedCargoFilters": [f"filter-{index}" for index in range(len(summaries))]},
            }
            (manifests / "fixture.json").write_text(json.dumps(manifest), encoding="utf-8")
            fixture = root / "commands.json"
            fixture.write_text(json.dumps({"commands": [
                {"lines": lines, "exitCode": (exit_codes or [0] * len(summaries))[index]}
                for index, lines in enumerate(summaries)
            ]}), encoding="utf-8")
            wrapper = root / "invoke.ps1"
            wrapper.write_text(
                "$ErrorActionPreference = 'Stop'\n"
                "$global:fixtureCommands = (Get-Content -Raw -Encoding UTF8 "
                "(Join-Path $PSScriptRoot 'commands.json') | ConvertFrom-Json).commands\n"
                "$global:fixtureIndex = 0\n"
                "function global:python { $global:LASTEXITCODE = 0 }\n"
                "function global:cargo {\n"
                "  Write-Output 'fixture-cargo-called'\n"
                "  $entry = $global:fixtureCommands[$global:fixtureIndex]\n"
                "  $global:fixtureIndex++\n"
                "  foreach ($line in $entry.lines) { Write-Output $line }\n"
                "  $global:LASTEXITCODE = [int]$entry.exitCode\n"
                "}\n"
                "& (Join-Path $PSScriptRoot 'tools/verify-gateway-line.ps1') "
                "-LineId fixture-line -LibOnly -AsJson "
                "-SharedCargoTargetDir (Join-Path $PSScriptRoot 'target') "
                + ("-SkipCargo" if skip_cargo else "") + "\n",
                encoding="utf-8",
            )
            return subprocess.run(
                [powershell_executable(), "-NoProfile", "-NonInteractive", "-File", str(wrapper)],
                cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=20,
            )

    def assert_rejected(self, summaries, message):
        result = self.run_verifier(summaries)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(message, result.stdout + result.stderr)

    def test_empty_filter_is_rejected_even_when_cargo_exits_zero(self):
        self.assert_rejected([["test result: ok. 0 passed; 0 failed; 0 ignored; 3 filtered out"]],
                             "Cargo command executed no tests")

    def test_missing_summary_is_rejected(self):
        self.assert_rejected([["Finished test profile"]], "Cargo command executed no tests")

    def test_ignored_only_filter_is_rejected(self):
        self.assert_rejected([["test result: ok. 0 passed; 0 failed; 2 ignored; 3 filtered out"]],
                             "Cargo command executed no tests")

    def test_previous_filter_success_does_not_hide_an_empty_filter(self):
        self.assert_rejected([
            ["test result: ok. 1 passed; 0 failed; 0 ignored; 3 filtered out"],
            ["test result: ok. 0 passed; 0 failed; 0 ignored; 3 filtered out"],
        ], "Cargo command executed no tests")

    def test_executed_tests_across_harnesses_preserve_empty_auxiliary_targets(self):
        result = self.run_verifier([[
            "test result: ok. 0 passed; 0 failed; 0 ignored; 3 filtered out",
            "test result: ok. 2 passed; 0 failed; 1 ignored; 3 filtered out",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 filtered out",
        ]])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads(result.stdout[result.stdout.index("{"):])
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["lineCount"], 1)
        self.assertIsInstance(report["results"], list)
        self.assertIn("--locked --lib --no-default-features --features line-fixture filter-0 -- --nocapture --test-threads=1",
                      result.stdout)

    def test_failed_cargo_status_is_not_hidden_by_a_successful_summary(self):
        result = self.run_verifier([["test result: ok. 1 passed; 0 failed; 0 ignored; 3 filtered out"]], [101])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Command failed with exit code 101", result.stdout + result.stderr)

    def test_skip_cargo_preserves_manifest_only_success(self):
        result = self.run_verifier([[]], skip_cargo=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("fixture-cargo-called", result.stdout)
        report = json.loads(result.stdout[result.stdout.index("{"):])
        self.assertTrue(report["cargoSkipped"])
        self.assertEqual(report["lineCount"], 1)


if __name__ == "__main__":
    unittest.main()
