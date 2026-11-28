"""Transport contract tests; all responses and credentials are synthetic."""
import io
import json
from email.message import Message
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock
from urllib.error import HTTPError

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import amd_radeon_local as client

KEY = "rc-fixture-key-not-real"
MODEL = "Fixture-Model-V1"


class Response(io.BytesIO):
    def __init__(self, data, media="application/json"):
        super().__init__(data)
        self.headers = Message()
        self.headers["content-type"] = media


class ClientTests(unittest.TestCase):
    def run_fixture(self, request, body, stream=False, media="application/json"):
        out = io.StringIO()
        opener = Mock()
        opener.open.return_value = Response(body, media)
        client.run_request(request, out, stream, opener=opener)
        opener.open.assert_called_once_with(request, timeout=120)
        return out.getvalue()

    def test_catalog_bare_envelope_and_exact_host(self):
        req = client.request_for(KEY)
        self.assertEqual(req.full_url, client.BASE + "/models")
        self.assertEqual(req.method, "GET")
        self.assertEqual(req.get_header("Authorization"), "Bearer " + KEY)
        result = self.run_fixture(req, b'{"data":[{"id":"Fixture-Model-V1","supports_streaming":true}]}')
        self.assertEqual(json.loads(result)["data"][0]["id"], MODEL)

    def test_chat_preserves_model_unicode_and_does_not_include_key_in_body(self):
        req = client.request_for(KEY, MODEL, "解释 ROCm")
        self.assertEqual(req.full_url, client.BASE + "/chat/completions")
        self.assertEqual(req.method, "POST")
        self.assertEqual(json.loads(req.data), {
            "model": MODEL, "messages": [{"role": "user", "content": "解释 ROCm"}], "stream": False,
        })
        self.assertNotIn(KEY.encode(), req.data)
        result = self.run_fixture(req, b'{"choices":[{"message":{"content":"OK"}}]}')
        self.assertEqual(json.loads(result)["choices"][0]["message"]["content"], "OK")

    def test_sse_multiline_and_done(self):
        req = client.request_for(KEY, MODEL, "test", True)
        self.assertTrue(json.loads(req.data)["stream"])
        self.assertEqual(req.get_header("Accept"), "text/event-stream")
        result = self.run_fixture(
            req, b': ping\r\ndata: {"choices":\r\ndata: []}\r\n\r\ndata: [DONE]\r\n\r\n',
            True, "text/event-stream; charset=utf-8",
        )
        self.assertEqual(result, 'data: {"choices": []}\n\ndata: [DONE]\n\n')

    def test_rejects_truncated_stream_and_incorrect_content_type(self):
        req = client.request_for(KEY, MODEL, "test", True)
        with self.assertRaisesRegex(client.RadeonError, "without a"):
            self.run_fixture(req, b'data: {"choices":[]}\n\n', True, "text/event-stream")
        with self.assertRaisesRegex(client.RadeonError, "not return"):
            self.run_fixture(req, b'{"choices":[]}', True)

    def test_http_status_is_reported_but_not_raw_error_body(self):
        req = client.request_for(KEY)
        for status in (302, 401, 429, 503):
            with self.subTest(status=status):
                opener = Mock()
                opener.open.side_effect = HTTPError(req.full_url, status, KEY, {}, io.BytesIO(KEY.encode()))
                with self.assertRaises(client.RadeonError) as error:
                    client.run_request(req, io.StringIO(), opener=opener)
                self.assertEqual(str(error.exception), f"AMD request failed: HTTP {status}.")
        self.assertIsNone(client.NoRedirect().redirect_request(req, None, 302, "", {}, "https://other.invalid"))

    def test_usage_summary_uses_points_and_excludes_history(self):
        req = client.request_for(KEY, usage=True)
        self.assertEqual(req.full_url, client.BASE + "/usage?include_recent=false")
        self.assertEqual(req.method, "GET")
        self.assertEqual(req.get_header("Authorization"), "Bearer " + KEY)
        data = {
            "daily_cost_limit_usd": 10, "daily_cost_used_usd": 1.25,
            "daily_cost_remaining_usd": 8.75, "daily_reset_at": "2026-11-29T00:00:00+08:00",
            "recent": [{"unneeded": "must-not-be-output"}],
        }
        out = json.loads(self.run_fixture(req, json.dumps(data).encode()))
        self.assertEqual(out, {
            "unit": "pts", "daily_limit_pts": 10, "daily_used_pts": 1.25,
            "daily_remaining_pts": 8.75, "reset_at": "2026-11-29T00:00:00+08:00",
        })
        for invalid in ({}, {"daily_cost_used_usd": True}, {"daily_cost_remaining_usd": -1}):
            values = data | invalid if invalid else invalid
            with self.assertRaisesRegex(client.RadeonError, "unknown"):
                self.run_fixture(req, json.dumps(values).encode())

    def test_max_tokens_is_explicit_and_validated(self):
        req = client.request_for(KEY, MODEL, "fixture prompt", max_tokens=16)
        self.assertEqual(json.loads(req.data)["max_tokens"], 16)
        for invalid in (0, -1, True, "16"):
            with self.assertRaisesRegex(client.RadeonError, "positive integer"):
                client.request_for(KEY, MODEL, "fixture prompt", max_tokens=invalid)

    def test_invalid_key_and_payload_fail_before_opening_network(self):
        for bad in ("", "sk-fixture-not-token-factory", "rc-key\r\nX-Injected: 1"):
            with self.assertRaises(client.RadeonError):
                client.request_for(bad)
        with self.assertRaises(client.RadeonError):
            client.request_for(KEY, MODEL, "   ")
        with self.assertRaises(client.RadeonError):
            self.run_fixture(client.request_for(KEY), b'{"models":[]}')


if __name__ == "__main__":
    unittest.main()
