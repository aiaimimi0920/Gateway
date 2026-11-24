"""The model capture utility must never print untrusted HTTP error material."""
import importlib.util
import io
import json
from pathlib import Path
import unittest
import urllib.error

SCRIPT = Path(__file__).resolve().parents[2] / "tools/luma/capture-mcp-models.py"
spec = importlib.util.spec_from_file_location("luma_model_capture", SCRIPT)
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)


class ModelCaptureErrors(unittest.TestCase):
    def detail(self, value):
        raw = json.dumps(value).encode("utf-8")
        error = urllib.error.HTTPError("http://127.0.0.1/", 400, "private", {}, io.BytesIO(raw))
        return capture.failure_detail(error)

    def test_fixed_expiry_identifiers_remain_diagnosable(self):
        self.assertEqual(self.detail({"error": {"code": "luma_mcp_material_unavailable",
            "message": "luma_oauth_expired"}}),
            " (luma_mcp_material_unavailable, luma_oauth_expired)")

    def test_supplier_prose_and_unknown_identifiers_are_not_printed(self):
        self.assertEqual(self.detail({"error": {"code": "secret-bearer", "message": "private-token"}}), "")
        self.assertEqual(self.detail({"error": {"code": "luma_mcp_catalog_failed", "message": "private-token"}}),
                         " (luma_mcp_catalog_failed)")

    def test_malformed_and_wrong_type_errors_are_silent(self):
        for value in [[], None, {"error": "private-token"}, {"error": {"code": ["private-token"]}}]:
            self.assertEqual(self.detail(value), "")
        error = urllib.error.HTTPError("http://127.0.0.1/", 400, "private", {}, io.BytesIO(b"not json"))
        self.assertEqual(capture.failure_detail(error), "")


if __name__ == "__main__":
    unittest.main()
