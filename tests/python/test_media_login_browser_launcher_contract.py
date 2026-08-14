import pathlib
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
LAUNCHER = REPO_ROOT / "tools" / "start-media-login-browsers.ps1"


class MediaLoginBrowserLauncherContractTests(unittest.TestCase):
    def test_uses_stable_provider_profiles_without_background_helpers(self):
        source = LAUNCHER.read_text(encoding="utf-8")

        self.assertIn("deploy\\gateway_data\\browser-profiles\\$providerName", source)
        self.assertIn("--remote-debugging-port=$($configuration.port)", source)
        self.assertIn("Test-LocalPortListening", source)
        self.assertIn('reason = "already-running"', source)
        self.assertNotIn("media-cdp-browser-helper", source)
        self.assertNotIn("Register-ScheduledTask", source)
        self.assertNotIn("Stop-Process", source)
        self.assertNotIn("WindowStyle Hidden", source)

    def test_executor_launcher_keeps_udio_manual_captcha_window_open(self):
        source = (REPO_ROOT / "tools" / "start-gateway-browser-executor.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn("[int]$UdioHcaptchaManualWaitMs = 300000", source)
        self.assertIn(
            '$env:UDIO_HCAPTCHA_MANUAL_WAIT_MS = [string]$UdioHcaptchaManualWaitMs',
            source,
        )
        self.assertIn("$previousUdioHcaptchaManualWaitMs", source)
        self.assertIn("UDIO_BROWSER_DEBUG_LOG_PATH", source)
        self.assertIn("udio-worker.debug.jsonl", source)


if __name__ == "__main__":
    unittest.main()
