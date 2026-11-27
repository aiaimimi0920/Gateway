"""Reject metered receipt false positives without network or live database access."""
import importlib.util
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[2] / "tools/cursor_remote/reconcile_protocols.py"
SPEC = importlib.util.spec_from_file_location("cursor_protocol_reconciliation", SOURCE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class MeteredReconciliationTests(unittest.TestCase):
    def test_missing_usage_cannot_disable_required_check(self):
        for name in ("bedrock-json-text", "dashscope-json-message", "dashscope-multimodal-stream-image"):
            self.assertTrue(MODULE.requires_metered_usage(name))
        self.assertFalse(MODULE.requires_metered_usage("bedrock-controls-rejections"))
        self.assertFalse(MODULE.requires_metered_usage("responses-json-text"))
        self.assertFalse(MODULE.exact_metered_usage({"phases": [{}]}, []))

    def test_exact_positive_integer_counts_required(self):
        rows = [{"status": "completed", "promptTokens": 12, "completionTokens": 3, "totalTokens": 15}]
        case = {"phases": [{"usage": {"input": 12, "output": 3, "total": 15}}]}
        self.assertTrue(MODULE.exact_metered_usage(case, rows))
        for usage in (None, {}, {"input": 0, "output": 0, "total": 0},
                      {"input": True, "output": 3, "total": 4},
                      {"input": 12, "output": 3, "total": 16}):
            self.assertFalse(MODULE.exact_metered_usage({"phases": [{"usage": usage}]}, rows))
        self.assertFalse(MODULE.exact_metered_usage(case, [{**rows[0], "completionTokens": True}]))
        self.assertFalse(MODULE.exact_metered_usage(case, rows * 2))


if __name__ == "__main__":
    unittest.main()
