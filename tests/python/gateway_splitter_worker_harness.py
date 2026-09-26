"""Local HTTP, socket, and process helpers for splitter integration tests."""

import http.server
import json
import os
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.request


def _reserve_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def _wait_until(predicate, timeout: float, description: str):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except (OSError, RuntimeError, urllib.error.URLError) as error:
            last_error = error
        time.sleep(0.1)
    suffix = f"; last error: {last_error}" if last_error else ""
    raise AssertionError(f"timed out waiting for {description}{suffix}")


def _http_json(
    base_url: str,
    path: str,
    *,
    method: str = "GET",
    token: str | None = None,
    payload: dict | None = None,
    timeout: float = 10.0,
) -> tuple[int, dict, dict[str, str]]:
    data = None
    headers = {"accept": "application/json"}
    if token:
        headers["x-management-token"] = token
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["content-type"] = "application/json"
    request = urllib.request.Request(
        base_url + path,
        data=data,
        headers=headers,
        method=method,
    )
    try:
        response = urllib.request.urlopen(request, timeout=timeout)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        body = response.read()
        parsed = json.loads(body.decode("utf-8")) if body else {}
        response_headers = {
            name.lower(): value for name, value in response.headers.items()
        }
        return response.status, parsed, response_headers


def _port_is_open(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as client:
        client.settimeout(0.2)
        return client.connect_ex(("127.0.0.1", port)) == 0


def _read_http_response(client: socket.socket) -> bytes:
    chunks: list[bytes] = []
    client.settimeout(10.0)
    while True:
        chunk = client.recv(65536)
        if not chunk:
            return b"".join(chunks)
        chunks.append(chunk)


def _crash_process(pid: int) -> None:
    if os.name == "nt":
        result = subprocess.run(
            ["taskkill", "/PID", str(pid), "/T", "/F"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if result.returncode != 0:
            raise AssertionError(
                f"failed to crash worker pid {pid}: {result.stdout}\n{result.stderr}"
            )
        return
    os.kill(pid, signal.SIGKILL)


def _request_graceful_splitter_exit(process: subprocess.Popen) -> bool:
    if process.poll() is not None:
        return True
    try:
        if os.name == "nt":
            process.send_signal(signal.CTRL_BREAK_EVENT)
        else:
            process.send_signal(signal.SIGTERM)
        process.wait(timeout=20)
        return True
    except (OSError, subprocess.TimeoutExpired):
        return False


class _MismatchedReadinessHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/readyz":
            self.send_error(404)
            return
        payload = b'{"status":"ready","worker_id":"unrelated-worker"}'
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.send_header("x-gateway-worker-id", "unrelated-worker")
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, _format, *_args):
        return
