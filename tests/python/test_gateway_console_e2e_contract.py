import pathlib
import unittest


class GatewayConsoleE2EContractTests(unittest.TestCase):
    def test_console_e2e_runner_uses_repo_local_runtime_outputs(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "run-gateway-console-e2e.ps1"

        self.assertTrue(script_path.is_file(), "Missing tools/run-gateway-console-e2e.ps1")

        script = script_path.read_text(encoding="utf-8")
        self.assertIn(".runtime\\console-e2e", script)
        self.assertIn("apps\\desktop", script)
        self.assertIn("console.spec.ts", script)
        self.assertIn("--output", script)
        self.assertIn("npm", script)
        self.assertNotIn("bash", script.lower())


if __name__ == "__main__":
    unittest.main()
