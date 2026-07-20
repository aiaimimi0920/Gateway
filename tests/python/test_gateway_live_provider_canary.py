import http.server
import json
import pathlib
import subprocess
import tempfile
import threading
import unittest

from powershell_test_utils import powershell_executable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
CANARY = REPO_ROOT / "scripts" / "invoke-gateway-live-provider-canary.ps1"


class _ProofHandler(http.server.BaseHTTPRequestHandler):
    received_headers = None
    received_body = None
    received_method = None
    response_provider_line = "linkup-search-official-vendor-api"
    emit_proof_headers = True
    response_status = 200
    response_payload = {"ok": True}

    def _handle_request(self):
        content_length = int(self.headers.get("Content-Length", "0"))
        type(self).received_body = self.rfile.read(content_length).decode("utf-8")
        type(self).received_headers = self.headers
        type(self).received_method = self.command

        request_id = self.headers.get("X-Request-Id", "")
        body = json.dumps(type(self).response_payload).encode("utf-8")
        self.send_response(type(self).response_status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("X-Request-Id", request_id)
        self.send_header("X-Neuro-Gateway-Route-Proof", "v1")
        if type(self).emit_proof_headers:
            self.send_header(
                "X-Neuro-Gateway-Provider-Line",
                type(self).response_provider_line,
            )
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        self._handle_request()

    def do_GET(self):
        self._handle_request()

    def log_message(self, format, *args):
        return


class GatewayLiveProviderCanaryTests(unittest.TestCase):
    def run_canary(self, server_port, temp_root, targets):
        targets_path = temp_root / "targets.json"
        targets_path.write_text(
            json.dumps({"targets": targets}),
            encoding="utf-8",
        )
        return subprocess.run(
            [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(CANARY),
                "-AllowLiveProviderCalls",
                "-GatewayBaseUrl",
                f"http://127.0.0.1:{server_port}",
                "-GatewayApiKey",
                "gateway-test-key",
                "-TargetsPath",
                str(targets_path),
                "-ArtifactRoot",
                str(temp_root / "artifacts"),
                "-AsJson",
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_single_target_document_is_accepted(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "single-linkup-canary",
                            "provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "POST",
                            "model": "linkup-search",
                            "body": {
                                "model": "linkup-search",
                                "q": "gateway live canary",
                            },
                        }
                    ],
                )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            payload = json.loads(result.stdout)
            self.assertEqual(len(payload["targets"]), 1)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_successful_canary_requires_matching_gateway_route_proof(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                temp_root = pathlib.Path(temp_dir)
                proof_target = {
                    "name": "linkup-search-canary",
                    "provider_line": "linkup-search-official-vendor-api",
                    "credential_source": "env:LINKUP_API_KEY_1",
                    "endpoint": "/v1/search",
                    "method": "POST",
                    "model": "linkup-search",
                    "expected_provider_line": "linkup-search-official-vendor-api",
                    "body": {
                        "model": "linkup-search",
                        "q": "gateway live canary",
                    },
                }
                second_target = dict(proof_target)
                second_target["name"] = "linkup-search-canary-second"
                result = self.run_canary(
                    server.server_port,
                    temp_root,
                    [proof_target, second_target],
                )

                self.assertEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                payload = json.loads(result.stdout)
                target = payload["targets"][-1]

            received_headers = _ProofHandler.received_headers
            self.assertIsNotNone(received_headers)
            self.assertEqual(
                received_headers.get("X-Neuro-Gateway-Route-Proof-Request"), "v1"
            )
            self.assertTrue(received_headers.get("X-Request-Id"))
            self.assertEqual(
                json.loads(_ProofHandler.received_body),
                {
                    "model": "linkup-search",
                    "q": "gateway live canary",
                },
            )
            self.assertEqual(
                target["observed_provider_line"],
                "linkup-search-official-vendor-api",
            )
            self.assertEqual(target["request_id"], received_headers.get("X-Request-Id"))
            self.assertEqual(target["route_proof"]["source"], "gateway_response_headers_v1")
            self.assertEqual(
                target["route_proof"]["provider_line"],
                "linkup-search-official-vendor-api",
            )
            self.assertNotIn("adapter", target["route_proof"])
            self.assertNotIn("protocol_profile", target["route_proof"])
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_mismatched_route_proof_is_not_reported_as_success(self):
        _ProofHandler.response_provider_line = "tavily-search-official-vendor-api"
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "mismatched-route-canary",
                            "provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "POST",
                            "model": "linkup-search",
                            "expected_provider_line": "linkup-search-official-vendor-api",
                            "body": {"model": "linkup-search", "q": "route proof"},
                        }
                    ],
                )

            self.assertNotEqual(result.returncode, 0)
            payload = json.loads(result.stdout)
            target = payload["targets"][0]
            self.assertEqual(target["status"], "fail")
            self.assertEqual(
                target["failure_classification"], "route_proof_missing_or_mismatch"
            )
            self.assertEqual(
                target["observed_provider_line"],
                "tavily-search-official-vendor-api",
            )
        finally:
            _ProofHandler.response_provider_line = "linkup-search-official-vendor-api"
            _ProofHandler.emit_proof_headers = True
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_missing_route_proof_is_not_reported_as_success(self):
        _ProofHandler.emit_proof_headers = False
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "missing-route-proof-canary",
                            "provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "POST",
                            "model": "linkup-search",
                            "expected_provider_line": "linkup-search-official-vendor-api",
                            "body": {"model": "linkup-search", "q": "route proof"},
                        }
                    ],
                )

            self.assertNotEqual(result.returncode, 0)
            payload = json.loads(result.stdout)
            target = payload["targets"][0]
            self.assertEqual(target["status"], "fail")
            self.assertEqual(
                target["failure_classification"], "route_proof_missing_or_mismatch"
            )
            self.assertIsNone(target["route_proof"])
        finally:
            _ProofHandler.emit_proof_headers = True
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_missing_route_proof_is_required_without_an_explicit_expected_line(self):
        _ProofHandler.emit_proof_headers = False
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "missing-route-proof-without-expected-line",
                            "provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "POST",
                            "model": "linkup-search",
                            "body": {"model": "linkup-search", "q": "route proof"},
                        }
                    ],
                )

            self.assertNotEqual(result.returncode, 0)
            payload = json.loads(result.stdout)
            target = payload["targets"][0]
            self.assertEqual(target["status"], "fail")
            self.assertEqual(
                target["failure_classification"], "route_proof_missing_or_mismatch"
            )
            self.assertIsNone(target["route_proof"])
        finally:
            _ProofHandler.emit_proof_headers = True
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_http_429_token_quota_text_is_classified_as_quota(self):
        _ProofHandler.response_status = 429
        _ProofHandler.response_payload = {
            "error": {"message": "token quota exceeded for this billing period"}
        }
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "quota-classification-canary",
                            "provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "POST",
                            "model": "linkup-search",
                            "expected_provider_line": "linkup-search-official-vendor-api",
                            "body": {"model": "linkup-search", "q": "quota"},
                        }
                    ],
                )

            self.assertNotEqual(result.returncode, 0)
            payload = json.loads(result.stdout)
            target = payload["targets"][0]
            self.assertEqual(target["http_status"], 429)
            self.assertEqual(target["failure_classification"], "quota_or_rate_limit")
        finally:
            _ProofHandler.response_status = 200
            _ProofHandler.response_payload = {"ok": True}
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_missing_provider_line_cannot_be_promoted_from_documented_defaults(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [{"body": {"messages": []}}],
                )

            self.assertNotEqual(result.returncode, 0)
            payload = json.loads(result.stdout)
            target = payload["targets"][0]
            self.assertEqual(target["endpoint"], "/v1/chat/completions")
            self.assertEqual(target["method"], "POST")
            self.assertEqual(target["name"], "")
            self.assertEqual(target["model"], "")
            self.assertEqual(target["credential_source"], "")
            self.assertEqual(target["status"], "fail")
            self.assertEqual(
                target["failure_classification"],
                "route_proof_missing_or_mismatch",
            )
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_missing_expected_provider_line_defaults_to_declared_line(self):
        _ProofHandler.response_provider_line = "tavily-search-official-vendor-api"
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "declared-line-fallback-canary",
                            "provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "POST",
                            "model": "linkup-search",
                            "body": {"model": "linkup-search", "q": "route proof"},
                        }
                    ],
                )

            self.assertNotEqual(result.returncode, 0)
            payload = json.loads(result.stdout)
            target = payload["targets"][0]
            self.assertEqual(target["expected_provider_line"], "linkup-search-official-vendor-api")
            self.assertEqual(target["observed_provider_line"], "tavily-search-official-vendor-api")
            self.assertEqual(target["failure_classification"], "route_proof_missing_or_mismatch")
        finally:
            _ProofHandler.response_provider_line = "linkup-search-official-vendor-api"
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_get_target_is_sent_as_get_without_a_request_body(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _ProofHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        try:
            with tempfile.TemporaryDirectory() as temp_dir:
                result = self.run_canary(
                    server.server_port,
                    pathlib.Path(temp_dir),
                    [
                        {
                            "name": "get-canary",
                            "provider_line": "linkup-search-official-vendor-api",
                            "expected_provider_line": "linkup-search-official-vendor-api",
                            "credential_source": "env:LINKUP_API_KEY_1",
                            "endpoint": "/v1/search",
                            "method": "GET",
                            "model": "linkup-search",
                        }
                    ],
                )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertEqual(_ProofHandler.received_method, "GET")
            self.assertEqual(_ProofHandler.received_body, "")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


if __name__ == "__main__":
    unittest.main()
