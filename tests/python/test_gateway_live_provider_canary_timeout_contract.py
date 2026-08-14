import pathlib
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
CANARY = REPO_ROOT / "scripts" / "invoke-gateway-live-provider-canary.ps1"


class GatewayLiveProviderCanaryTimeoutContractTests(unittest.TestCase):
    def test_browser_challenge_is_not_misclassified_as_quota(self):
        source = CANARY.read_text(encoding="utf-8")

        challenge_index = source.index('return "browser_challenge_required"')
        quota_index = source.index('return "quota_or_rate_limit"')
        self.assertLess(challenge_index, quota_index)

    def test_media_wait_timeout_extends_http_timeout_with_safety_margin(self):
        source = CANARY.read_text(encoding="utf-8")

        self.assertIn("[int]$MaxTimeSeconds = 60", source)
        self.assertIn("([int]$parsedBody.wait_timeout_secs + 30)", source)
        self.assertIn(
            '$requestArguments["MaxTimeSeconds"] = $requestMaxTimeSeconds', source
        )
        self.assertNotIn('"--max-time",\n    "60"', source)


if __name__ == "__main__":
    unittest.main()
