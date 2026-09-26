import http.server
import os
import pathlib
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import unittest
import uuid

from gateway_splitter_worker_harness import (
    _MismatchedReadinessHandler,
    _crash_process,
    _http_json,
    _port_is_open,
    _read_http_response,
    _request_graceful_splitter_exit,
    _reserve_port,
    _wait_until,
)


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
SPLITTER_STATUS_PATH = "/v1/internal/gateway/splitter/status"
SPLITTER_RELOAD_PATH = "/v1/internal/gateway/splitter/reload"


@unittest.skipUnless(
    os.environ.get("GATEWAY_RUN_SPLITTER_E2E_TESTS") == "1",
    "set GATEWAY_RUN_SPLITTER_E2E_TESTS=1 for the isolated splitter worker E2E",
)
class GatewaySplitterWorkerE2ETests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if shutil.which("docker") is None:
            raise unittest.SkipTest("Docker is required for the isolated Redis runtime")

        configured_binary = os.environ.get("GATEWAY_SPLITTER_E2E_BINARY")
        if configured_binary:
            cls.gateway_binary = pathlib.Path(configured_binary).resolve()
        else:
            build = subprocess.run(
                [
                    "cargo",
                    "build",
                    "--manifest-path",
                    str(GATEWAY_ROOT / "Cargo.toml"),
                    "--locked",
                    "--bin",
                    "gateway",
                ],
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            if build.returncode != 0:
                raise AssertionError(
                    f"failed to build gateway binary\nstdout:\n{build.stdout}\nstderr:\n{build.stderr}"
                )
            cls.gateway_binary = GATEWAY_ROOT / "target" / "debug" / (
                "gateway.exe" if os.name == "nt" else "gateway"
            )

        if not cls.gateway_binary.is_file():
            raise AssertionError(f"gateway binary does not exist: {cls.gateway_binary}")

    def test_real_worker_replacement_cutover_and_failed_replacement_cleanup(self):
        run_id = uuid.uuid4().hex[:12]
        container_name = f"gateway-splitter-e2e-{run_id}"
        splitter_port = _reserve_port()
        initial_worker_port = _reserve_port()
        replacement_worker_port = _reserve_port()
        failed_worker_port = _reserve_port()
        recovery_worker_port = _reserve_port()
        management_token = f"splitter-e2e-{run_id}"
        splitter = None
        with tempfile.TemporaryDirectory(prefix="gateway-splitter-e2e-") as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            stdout_path = temp_root / "splitter.stdout.log"
            stderr_path = temp_root / "splitter.stderr.log"
            stdout_handle = stdout_path.open("w", encoding="utf-8")
            stderr_handle = stderr_path.open("w", encoding="utf-8")
            mismatched_readiness_server = None
            mismatched_readiness_thread = None

            try:
                redis_container_id = subprocess.check_output(
                    [
                        "docker",
                        "run",
                        "--detach",
                        "--rm",
                        "--name",
                        container_name,
                        "--publish",
                        "127.0.0.1::6379",
                        "redis:7-alpine",
                        "redis-server",
                        "--save",
                        "",
                        "--appendonly",
                        "no",
                    ],
                    cwd=REPO_ROOT,
                    text=True,
                    stderr=subprocess.STDOUT,
                ).strip()
                self.assertTrue(redis_container_id)

                redis_port_output = _wait_until(
                    lambda: subprocess.check_output(
                        ["docker", "port", container_name, "6379/tcp"],
                        cwd=REPO_ROOT,
                        text=True,
                        stderr=subprocess.DEVNULL,
                    ).strip(),
                    10.0,
                    "Docker Redis port mapping",
                )
                redis_port = int(redis_port_output.rsplit(":", 1)[1])
                _wait_until(
                    lambda: subprocess.run(
                        ["docker", "exec", container_name, "redis-cli", "PING"],
                        cwd=REPO_ROOT,
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    ).stdout.strip()
                    == "PONG",
                    15.0,
                    "isolated Redis readiness",
                )

                environment = os.environ.copy()
                environment.update(
                    {
                        "GATEWAY_RUNTIME_ROLE": "splitter",
                        "PORT": str(splitter_port),
                        "GATEWAY_REDIS_URL": f"redis://127.0.0.1:{redis_port}/0",
                        "GATEWAY_DATABASE_URL": "",
                        "GATEWAY_API_KEY": f"api-{run_id}",
                        "GATEWAY_API_KEY_SECRET": f"api-secret-{run_id}",
                        "GATEWAY_MANAGEMENT_TOKEN": management_token,
                        "GATEWAY_SPLITTER_WORKER_EXECUTABLE_PATH": str(
                            self.gateway_binary
                        ),
                        "GATEWAY_SPLITTER_INITIAL_WORKER_PORT": str(
                            initial_worker_port
                        ),
                        "GATEWAY_SPLITTER_READY_TIMEOUT_SECS": "10",
                        "GATEWAY_SPLITTER_READY_POLL_INTERVAL_MILLIS": "100",
                        "GATEWAY_SPLITTER_RELOAD_SHUTDOWN_TIMEOUT_SECS": "10",
                        "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_ENABLED": "false",
                        "GATEWAY_PROVIDER_CREDENTIAL_REFRESH_ENABLED": "false",
                        "GATEWAY_CREDENTIAL_STOCK_MONITOR_ENABLED": "false",
                        "AI_GATEWAY_OBJECT_STORAGE_DRIVER": "local",
                        "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR": str(
                            temp_root / "objects"
                        ),
                        "RUST_LOG": "info",
                    }
                )
                environment.pop("DATABASE_URL", None)

                creation_flags = 0
                if os.name == "nt":
                    creation_flags = subprocess.CREATE_NEW_PROCESS_GROUP
                splitter = subprocess.Popen(
                    [str(self.gateway_binary)],
                    cwd=GATEWAY_ROOT,
                    env=environment,
                    stdin=subprocess.DEVNULL,
                    stdout=stdout_handle,
                    stderr=stderr_handle,
                    creationflags=creation_flags,
                )
                base_url = f"http://127.0.0.1:{splitter_port}"

                ready_payload = _wait_until(
                    lambda: (
                        lambda response: response[1]
                        if response[0] == 200
                        else None
                    )(_http_json(base_url, "/readyz", timeout=1.0)),
                    30.0,
                    "splitter and initial worker readiness",
                )
                initial_worker_id = ready_payload["activeWorkerId"]

                status_code, status_payload, _ = _http_json(
                    base_url, SPLITTER_STATUS_PATH, token=management_token
                )
                self.assertEqual(status_code, 200)
                initial_worker = next(
                    worker
                    for worker in status_payload["splitter"]["workers"]
                    if worker["id"] == initial_worker_id
                )
                self.assertEqual(initial_worker["status"], "active")
                self.assertEqual(initial_worker["port"], initial_worker_port)

                probe_status, _, probe_headers = _http_json(
                    base_url, f"/splitter-e2e/{run_id}", timeout=3.0
                )
                self.assertEqual(probe_status, 404)
                self.assertEqual(
                    probe_headers.get("x-gateway-splitter-worker-id"),
                    initial_worker_id,
                )

                slow_client = socket.create_connection(
                    ("127.0.0.1", splitter_port), timeout=5.0
                )
                slow_client.sendall(
                    (
                        "POST /v1/models HTTP/1.1\r\n"
                        f"Host: 127.0.0.1:{splitter_port}\r\n"
                        "Content-Type: application/json\r\n"
                        "Content-Length: 10\r\n"
                        "Connection: close\r\n\r\n"
                        "abc"
                    ).encode("ascii")
                )

                _wait_until(
                    lambda: next(
                        (
                            worker
                            for worker in _http_json(
                                base_url,
                                SPLITTER_STATUS_PATH,
                                token=management_token,
                            )[1]["splitter"]["workers"]
                            if worker["id"] == initial_worker_id
                            and worker["activeRequests"] == 1
                        ),
                        None,
                    ),
                    5.0,
                    "slow request admission on the initial worker",
                )

                reload_status, reload_payload, _ = _http_json(
                    base_url,
                    SPLITTER_RELOAD_PATH,
                    method="POST",
                    token=management_token,
                    payload={
                        "workerPort": replacement_worker_port,
                        "readyTimeoutSecs": 10,
                        "shutdownTimeoutSecs": 10,
                    },
                    timeout=20.0,
                )
                self.assertEqual(reload_status, 202, msg=reload_payload)
                replacement_worker_id = reload_payload["splitter"]["activeWorkerId"]
                self.assertNotEqual(replacement_worker_id, initial_worker_id)
                reload_workers = {
                    worker["id"]: worker
                    for worker in reload_payload["splitter"]["workers"]
                }
                self.assertEqual(
                    reload_workers[replacement_worker_id]["status"], "active"
                )
                self.assertEqual(reload_workers[initial_worker_id]["status"], "draining")
                self.assertEqual(
                    reload_workers[initial_worker_id]["activeRequests"], 1
                )

                slow_client.sendall(b"defghij")
                slow_response = _read_http_response(slow_client)
                slow_client.close()
                self.assertIn(b"HTTP/1.1", slow_response)
                self.assertIn(
                    f"x-gateway-splitter-worker-id: {initial_worker_id}".encode(
                        "ascii"
                    ),
                    slow_response.lower(),
                    msg=slow_response.decode("utf-8", errors="replace"),
                )

                probe_status, _, probe_headers = _http_json(
                    base_url, f"/splitter-e2e/{run_id}/after-cutover", timeout=3.0
                )
                self.assertEqual(probe_status, 404)
                self.assertEqual(
                    probe_headers.get("x-gateway-splitter-worker-id"),
                    replacement_worker_id,
                )

                exited_initial = _wait_until(
                    lambda: next(
                        (
                            worker
                            for worker in _http_json(
                                base_url,
                                SPLITTER_STATUS_PATH,
                                token=management_token,
                            )[1]["splitter"]["workers"]
                            if worker["id"] == initial_worker_id
                            and worker["status"] == "exited"
                        ),
                        None,
                    ),
                    15.0,
                    "old worker graceful exit",
                )
                self.assertEqual(exited_initial["drainReason"], "splitter_reload")
                self.assertIsNotNone(exited_initial["drainRequestedAt"])
                self.assertFalse(_port_is_open(initial_worker_port))

                mismatched_readiness_server = http.server.ThreadingHTTPServer(
                    ("127.0.0.1", failed_worker_port),
                    _MismatchedReadinessHandler,
                )
                mismatched_readiness_thread = threading.Thread(
                    target=mismatched_readiness_server.serve_forever,
                    daemon=True,
                )
                mismatched_readiness_thread.start()
                failure_started = time.monotonic()
                failure_status, failure_payload, _ = _http_json(
                    base_url,
                    SPLITTER_RELOAD_PATH,
                    method="POST",
                    token=management_token,
                    payload={
                        "executablePath": str(self.gateway_binary),
                        "workerPort": failed_worker_port,
                        "readyTimeoutSecs": 1,
                        "shutdownTimeoutSecs": 1,
                    },
                    timeout=8.0,
                )
                failure_elapsed = time.monotonic() - failure_started
                self.assertEqual(failure_status, 502, msg=failure_payload)
                self.assertLess(
                    failure_elapsed,
                    3.0,
                    msg="readyTimeoutSecs must bound failed replacement cleanup",
                )

                _, final_status, _ = _http_json(
                    base_url, SPLITTER_STATUS_PATH, token=management_token
                )
                self.assertEqual(
                    final_status["splitter"]["activeWorkerId"],
                    replacement_worker_id,
                )
                failed_worker = next(
                    worker
                    for worker in final_status["splitter"]["workers"]
                    if worker["port"] == failed_worker_port
                )
                self.assertEqual(failed_worker["status"], "exited")
                self.assertIn(
                    "replacement_readiness_failed", failed_worker["exitStatus"]
                )

                probe_status, _, probe_headers = _http_json(
                    base_url, f"/splitter-e2e/{run_id}/after-failure", timeout=3.0
                )
                self.assertEqual(probe_status, 404)
                self.assertEqual(
                    probe_headers.get("x-gateway-splitter-worker-id"),
                    replacement_worker_id,
                )

                active_before_crash = next(
                    worker
                    for worker in final_status["splitter"]["workers"]
                    if worker["id"] == replacement_worker_id
                )
                self.assertIsInstance(active_before_crash["pid"], int)
                _crash_process(active_before_crash["pid"])

                crashed_worker = _wait_until(
                    lambda: next(
                        (
                            worker
                            for status, payload, _headers in [
                                _http_json(
                                    base_url,
                                    SPLITTER_STATUS_PATH,
                                    token=management_token,
                                )
                            ]
                            if status == 200
                            and payload["splitter"]["activeWorkerId"] is None
                            for worker in payload["splitter"]["workers"]
                            if worker["id"] == replacement_worker_id
                            and worker["status"] == "exited"
                        ),
                        None,
                    ),
                    5.0,
                    "active worker crash supervision",
                )
                self.assertIn("crashed", crashed_worker["exitStatus"])
                not_ready_status, not_ready_payload, _ = _http_json(
                    base_url, "/readyz", timeout=2.0
                )
                self.assertEqual(not_ready_status, 503)
                self.assertEqual(not_ready_payload["reason"], "no_active_worker")

                recovery_status, recovery_payload, _ = _http_json(
                    base_url,
                    SPLITTER_RELOAD_PATH,
                    method="POST",
                    token=management_token,
                    payload={
                        "workerPort": recovery_worker_port,
                        "readyTimeoutSecs": 10,
                        "shutdownTimeoutSecs": 5,
                    },
                    timeout=20.0,
                )
                self.assertEqual(recovery_status, 202, msg=recovery_payload)
                recovery_worker_id = recovery_payload["splitter"]["activeWorkerId"]
                self.assertNotEqual(recovery_worker_id, replacement_worker_id)
                recovery_probe_status, _, recovery_probe_headers = _http_json(
                    base_url, f"/splitter-e2e/{run_id}/after-recovery", timeout=3.0
                )
                self.assertEqual(recovery_probe_status, 404)
                self.assertEqual(
                    recovery_probe_headers.get("x-gateway-splitter-worker-id"),
                    recovery_worker_id,
                )
            finally:
                if mismatched_readiness_server is not None:
                    mismatched_readiness_server.shutdown()
                    mismatched_readiness_server.server_close()
                if mismatched_readiness_thread is not None:
                    mismatched_readiness_thread.join(timeout=5)
                if splitter is not None:
                    if not _request_graceful_splitter_exit(splitter) and os.name == "nt":
                        subprocess.run(
                            [
                                "taskkill",
                                "/PID",
                                str(splitter.pid),
                                "/T",
                                "/F",
                            ],
                            stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE,
                        )
                    elif splitter.poll() is None:
                        splitter.terminate()
                    try:
                        splitter.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        splitter.kill()
                        splitter.wait(timeout=5)
                stdout_handle.close()
                stderr_handle.close()
                subprocess.run(
                    ["docker", "rm", "--force", container_name],
                    cwd=REPO_ROOT,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )

                for port in (
                    splitter_port,
                    initial_worker_port,
                    replacement_worker_port,
                    failed_worker_port,
                    recovery_worker_port,
                ):
                    _wait_until(
                        lambda port=port: not _port_is_open(port),
                        10.0,
                        f"port {port} cleanup",
                    )
                container_ids = subprocess.check_output(
                    [
                        "docker",
                        "ps",
                        "--all",
                        "--quiet",
                        "--filter",
                        f"name=^{container_name}$",
                    ],
                    cwd=REPO_ROOT,
                    text=True,
                ).strip()
                self.assertEqual(container_ids, "")


if __name__ == "__main__":
    unittest.main()


