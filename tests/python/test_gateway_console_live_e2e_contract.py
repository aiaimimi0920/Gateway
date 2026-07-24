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

    def test_live_console_runner_covers_revision_restore_after_browser_mutation(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "run-gateway-console-live-e2e.ps1"
        spec_path = repo_root / "apps" / "desktop" / "e2e" / "console.live.spec.ts"

        script = script_path.read_text(encoding="utf-8")
        spec = spec_path.read_text(encoding="utf-8")

        self.assertIn("GATEWAY_LIVE_EXPECT_ADDED_PROVIDER_ID", script)
        self.assertIn("GATEWAY_LIVE_EXPECT_ADDED_MODEL", script)
        self.assertIn("GATEWAY_LIVE_EXPECT_RESTORED_MODEL", script)
        self.assertIn("GATEWAY_LIVE_EXPECT_REMOVED_MODEL", script)
        self.assertIn("restores a live archived revision through the browser console", spec)
        self.assertIn("Restore revision as active config", spec)
        self.assertIn("Confirm restore", spec)
        self.assertIn("GATEWAY_LIVE_EXPECT_REMOVED_MODEL", spec)

    def test_live_console_runner_routes_chat_completions_through_local_fixture_upstream(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        script_path = repo_root / "tools" / "run-gateway-console-live-e2e.ps1"
        spec_path = repo_root / "apps" / "desktop" / "e2e" / "console.live.spec.ts"

        script = script_path.read_text(encoding="utf-8")
        spec = spec_path.read_text(encoding="utf-8")

        self.assertIn("Start-LiveOpenAiCompatibleUpstream", script)
        self.assertIn("GATEWAY_LIVE_UPSTREAM_BASE_URL", script)
        self.assertIn("GATEWAY_LIVE_CHAT_MODEL", script)
        self.assertIn("GATEWAY_LIVE_CHAT_EXPECT_TEXT", script)
        self.assertIn("routes a live chat completion through a browser-configured provider", spec)
        self.assertIn("/v1/chat/completions", spec)
        self.assertIn("GATEWAY_LIVE_UPSTREAM_BASE_URL", spec)
        self.assertIn("GATEWAY_LIVE_CHAT_MODEL", spec)


if __name__ == "__main__":
    unittest.main()
