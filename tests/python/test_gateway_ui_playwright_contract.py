import json
import pathlib
import re
import unittest


class GatewayUiPlaywrightContractTests(unittest.TestCase):
    def test_playwright_base_url_matches_web_dev_server_port(self):
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        package_json = json.loads((repo_root / "apps" / "desktop" / "package.json").read_text(encoding="utf-8"))
        playwright_config = (
            repo_root / "apps" / "desktop" / "playwright.config.ts"
        ).read_text(encoding="utf-8")

        dev_web_script = package_json["scripts"]["dev:web"]
        dev_port_match = re.search(r"--port\s+(?P<port>\d+)", dev_web_script)
        self.assertIsNotNone(dev_port_match, msg=f"Missing --port in dev:web script: {dev_web_script}")

        base_url_match = re.search(
            r'GATEWAY_UI_BASE_URL\s*\?\?\s*"http://127\.0\.0\.1:(?P<port>\d+)/ui/"',
            playwright_config,
        )
        self.assertIsNotNone(
            base_url_match,
            msg="Playwright config must default to a loopback /ui/ base URL.",
        )

        self.assertEqual(
            base_url_match.group("port"),
            dev_port_match.group("port"),
            msg="Playwright baseURL port must match the desktop web dev server port.",
        )


if __name__ == "__main__":
    unittest.main()
