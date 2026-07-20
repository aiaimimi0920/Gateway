import pathlib
import subprocess
import tempfile
import unittest

from powershell_test_utils import powershell_executable


class VerifyGatewayLineListOnlyTests(unittest.TestCase):
    def test_list_only_reports_manifest_line_ids(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "verify-gateway-line.ps1"

        result = subprocess.run(
            [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(script_path),
                "-ListOnly",
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
        self.assertIn("xai-openai-official-vendor-api", result.stdout)
        self.assertIn("perplexity-chat-official-vendor-api", result.stdout)
        self.assertIn("freebuff-web-reverse-api", result.stdout)
        self.assertIn("xfyun-native-websocket-official-vendor-api", result.stdout)
        self.assertIn("producer-web-reverse-api", result.stdout)
        self.assertIn("kiro-official-vendor-api", result.stdout)

    def test_all_mode_reports_every_manifest_without_running_cargo_when_skipped(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "verify-gateway-line.ps1"

        with tempfile.TemporaryDirectory() as target_dir:
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(script_path),
                    "-All",
                    "-LibOnly",
                    "-SkipCargo",
                    "-SharedCargoTargetDir",
                    target_dir,
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
        self.assertIn('"status":  "pass"', result.stdout)
        self.assertIn('"lineCount":  41', result.stdout)
        self.assertIn('"cargoSkipped":  true', result.stdout)


if __name__ == "__main__":
    unittest.main()
