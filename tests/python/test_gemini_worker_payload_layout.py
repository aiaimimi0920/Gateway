import pathlib
import re
import unittest


class GeminiWorkerPayloadLayoutTests(unittest.TestCase):
    def test_container_declares_payload_separately_from_executable_and_data(self):
        dockerfile = (pathlib.Path(__file__).resolve().parents[2] / "Dockerfile").read_text(encoding="utf-8")
        runtime = dockerfile[dockerfile.rfind("\nFROM "):]
        environment = re.sub(r"\\\r?\n\s*", " ", runtime)
        self.assertRegex(environment, r"(?m)^ENV\s+[^\n]*\bGATEWAY_RELEASE_PAYLOAD_ROOT=/app(?:\s|$)")
        self.assertIn("COPY scripts ./scripts", runtime)
        self.assertIn("/usr/local/bin/gateway", runtime)
        self.assertIn("GATEWAY_STATE_DIR=/data/state", environment)
        self.assertIn("GATEWAY_ROUTES_FILE=/data/routes.yaml", environment)


if __name__ == "__main__":
    unittest.main()
