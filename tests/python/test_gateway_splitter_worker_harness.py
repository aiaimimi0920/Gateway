import json
import socket
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from gateway_splitter_worker_harness import (
    _MismatchedReadinessHandler,
    _http_json,
    _port_is_open,
    _read_http_response,
    _reserve_port,
    _wait_until,
)


class _JsonContractHandler(BaseHTTPRequestHandler):
    def _send_json(self, status: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("content-length", str(len(body)))
        self.send_header("x-contract-result", "present")
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:
        length = int(self.headers["content-length"])
        payload = json.loads(self.rfile.read(length).decode("utf-8"))
        self.server.observed_request = {
            "path": self.path,
            "token": self.headers.get("x-management-token"),
            "payload": payload,
        }
        self._send_json(202, {"accepted": True})

    def do_GET(self) -> None:
        self._send_json(502, {"error": "unavailable"})

    def log_message(self, _format: str, *_args) -> None:
        return


class GatewaySplitterWorkerHarnessTests(unittest.TestCase):
    def test_http_json_preserves_request_and_error_response_contracts(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), _JsonContractHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base_url = f"http://127.0.0.1:{server.server_address[1]}"
        try:
            status, payload, headers = _http_json(
                base_url,
                "/submit",
                method="POST",
                token="test-management-token",
                payload={"probe": 42},
            )
            self.assertEqual(status, 202)
            self.assertEqual(payload, {"accepted": True})
            self.assertEqual(headers["x-contract-result"], "present")
            self.assertEqual(
                server.observed_request,
                {
                    "path": "/submit",
                    "token": "test-management-token",
                    "payload": {"probe": 42},
                },
            )

            error_status, error_payload, _ = _http_json(base_url, "/failure")
            self.assertEqual(error_status, 502)
            self.assertEqual(error_payload, {"error": "unavailable"})
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        self.assertFalse(thread.is_alive())

    def test_mismatched_readiness_server_keeps_identity_mismatch_visible(self):
        server = ThreadingHTTPServer(
            ("127.0.0.1", 0), _MismatchedReadinessHandler
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base_url = f"http://127.0.0.1:{server.server_address[1]}"
        try:
            status, payload, headers = _http_json(base_url, "/readyz")
            self.assertEqual(status, 200)
            self.assertEqual(payload["worker_id"], "unrelated-worker")
            self.assertEqual(headers["x-gateway-worker-id"], "unrelated-worker")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        self.assertFalse(thread.is_alive())

    def test_wait_until_retries_transient_errors_and_returns_ready_value(self):
        attempts = 0

        def probe():
            nonlocal attempts
            attempts += 1
            if attempts == 1:
                raise OSError("temporary connection refusal")
            if attempts < 3:
                return None
            return "ready"

        self.assertEqual(_wait_until(probe, 1.0, "test readiness"), "ready")
        self.assertEqual(attempts, 3)

    def test_socket_and_port_helpers_track_open_connections(self):
        port = _reserve_port()
        listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            listener.bind(("127.0.0.1", port))
            listener.listen()
            self.assertTrue(_port_is_open(port))
        finally:
            listener.close()
        self.assertFalse(_port_is_open(port))

        reader, writer = socket.socketpair()
        try:
            response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok"
            writer.sendall(response)
            writer.shutdown(socket.SHUT_WR)
            self.assertEqual(_read_http_response(reader), response)
        finally:
            reader.close()
            writer.close()


if __name__ == "__main__":
    unittest.main()
