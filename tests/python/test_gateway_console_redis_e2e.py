import os
import pathlib
import shutil
import subprocess
import time
import unittest
import uuid


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]


def _wait_until(probe, timeout_seconds: float, description: str):
    deadline = time.time() + timeout_seconds
    last_error = None
    while time.time() < deadline:
        try:
            value = probe()
            if value:
                return value
        except Exception as error:  # pragma: no cover - diagnostic only
            last_error = error
        time.sleep(0.25)
    if last_error is not None:
        raise AssertionError(f"timed out waiting for {description}: {last_error}")
    raise AssertionError(f"timed out waiting for {description}")


@unittest.skipUnless(
    os.environ.get("GATEWAY_RUN_CONSOLE_REDIS_E2E") == "1",
    "set GATEWAY_RUN_CONSOLE_REDIS_E2E=1 for the isolated console Redis E2E",
)
class GatewayConsoleRedisE2ETests(unittest.TestCase):
    def test_namespaced_console_redis_commit_and_replica_reconcile(self):
        if shutil.which("docker") is None:
            raise unittest.SkipTest("Docker is required for the isolated console Redis E2E")
        docker_info = subprocess.run(
            ["docker", "info"],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if docker_info.returncode != 0:
            raise unittest.SkipTest(
                "Docker daemon is unavailable for the isolated console Redis E2E: "
                f"{docker_info.stderr.strip() or docker_info.stdout.strip()}"
            )

        run_id = uuid.uuid4().hex[:12]
        container_name = f"gateway-console-redis-e2e-{run_id}"
        namespace = f"console_e2e_{run_id}"

        try:
            container_id = subprocess.check_output(
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
            self.assertTrue(container_id)

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
                    "GATEWAY_CONSOLE_REDIS_TEST_URL": f"redis://127.0.0.1:{redis_port}/0",
                    "GATEWAY_CONSOLE_REDIS_TEST_NAMESPACE": namespace,
                }
            )
            result = subprocess.run(
                [
                    "cargo",
                    "test",
                    "--manifest-path",
                    str(REPO_ROOT / "Cargo.toml"),
                    "--locked",
                    "--test",
                    "console_redis_live_contract",
                    "--",
                    "--nocapture",
                ],
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=environment,
            )
            if result.returncode != 0:
                raise AssertionError(
                    "console Redis live contract failed\n"
                    f"stdout:\n{result.stdout}\n"
                    f"stderr:\n{result.stderr}"
                )
        finally:
            subprocess.run(
                ["docker", "rm", "--force", container_name],
                cwd=REPO_ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
