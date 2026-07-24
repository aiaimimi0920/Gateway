import pathlib
import unittest


class GatewayConsoleLiveE2EContractTests(unittest.TestCase):
    def test_live_console_runner_uses_real_gateway_runtime_and_playwright_env(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "run-gateway-console-live-e2e.ps1"
        spec_path = repo_root / "apps" / "desktop" / "e2e" / "console.live.spec.ts"

        self.assertTrue(script_path.is_file(), "Missing tools/run-gateway-console-live-e2e.ps1")
        self.assertTrue(spec_path.is_file(), "Missing apps/desktop/e2e/console.live.spec.ts")

        script = script_path.read_text(encoding="utf-8")
        self.assertIn(".runtime\\console-live-e2e", script)
        self.assertIn("GATEWAY_UI_BASE_URL", script)
        self.assertIn("PLAYWRIGHT_SKIP_WEBSERVER", script)
        self.assertIn("console.live.spec.ts", script)
        self.assertIn("target\\debug\\neuro-gateway.exe", script)
        self.assertIn("redis:7-alpine", script)


if __name__ == "__main__":
    unittest.main()
