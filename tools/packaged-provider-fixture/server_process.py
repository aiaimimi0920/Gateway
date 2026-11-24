"""Run just the verified backend in an explicit, fresh loopback environment."""

import hashlib
import http.client
import json
import os
import re
import socket
import subprocess
import time
import urllib.parse

PG = "gateway-first-three-pg-20261114"
REDIS = "gateway-first-three-redis-20261114"
LABEL = "gateway.acceptance"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def package_identity(package):
    manifest = package / "manifest.json"
    data = json.loads(manifest.read_text(encoding="utf-8-sig"))
    require(data.get("buildKind") == "devtest" and data.get("checksums") == "checksums.sha256",
            "Expected an isolated devtest package")
    binary = package / "gateway.exe"
    require(binary.resolve() == binary and not (package / ".env").exists(),
            "Package binary must not be redirected and package must not contain .env")
    lines = (package / "checksums.sha256").read_text(encoding="ascii").splitlines()
    digest = sha256(binary)
    require([line.lower() for line in lines if line.endswith("  gateway.exe")] == [f"{digest}  gateway.exe"],
            "gateway.exe does not match the checksum index")
    return {"manifestSha256": sha256(manifest), "exeSha256": digest}


def docker(*args):
    result = subprocess.run(["docker", *args], capture_output=True, text=True, encoding="utf-8", timeout=20)
    require(result.returncode == 0, f"Isolated Docker {args[0]} command failed")
    return result.stdout.strip()


def fixture_url(name, container, service_port, scheme, path):
    raw = os.environ[name]
    try:
        url = urllib.parse.urlsplit(raw)
        port = url.port
    except ValueError:
        raise RuntimeError(f"Invalid {name}") from None
    require(url.scheme == scheme and url.hostname == "127.0.0.1" and url.path == path
            and port not in (None, 0, service_port) and not url.query and not url.fragment,
            f"Invalid isolated fixture URL: {name}")
    kind = '{{index .Config.Labels "' + LABEL + '"}} {{.State.Running}}'
    require(docker("inspect", "--format", kind, container) == "first-three-20261114 true",
            f"Start and check the dedicated fixture container {container} first")
    require(docker("port", container, f"{service_port}/tcp") == f"127.0.0.1:{port}",
            f"{name} must match the dedicated fixture container port")
    return raw


def sql(pg, statement):
    user = urllib.parse.unquote(urllib.parse.urlsplit(pg).username or "")
    require(user and not user.startswith("-"), "Fixture PostgreSQL user is missing or invalid")
    return docker("exec", PG, "psql", "-XAt", "-v", "ON_ERROR_STOP=1", "-U", user,
                  "-d", "gateway_fixture", "-c", statement)


def response_error_code(text):
    try:
        payload = json.loads(text)
    except ValueError:
        return None
    error = payload.get("error") if isinstance(payload, dict) else None
    code = error.get("code") if isinstance(error, dict) else None
    return code if isinstance(code, str) and re.fullmatch(r"[a-z][a-z0-9_]{0,79}", code) else None


class Backend:
    def __init__(self, package, root, fixture, namespace, token, signing_secret, pg, redis, *,
                 ca=None, route_doc=None, provider="SENSENOVA", disable_system_proxy=False):
        require(provider in {"SENSENOVA", "COMMANDCODE", "SAP_AICORE"}, "Unknown fixture provider")
        self.token = token
        # A standalone with explicit routes is allowed to serve degraded on Redis failure,
        # but the scheduler fixture still requires its actual Redis probe to be healthy.
        self.redis_required_for_public_routes = not (route_doc and route_doc.get("providers"))
        self.drain_reason = f"packaged_{provider.lower()}_fixture"
        self.cleanup_error = None
        root.mkdir()
        # Block dotenv discovery in the evidence directory's parents.
        (root / ".env").write_text("", encoding="utf-8")
        routes = root / "routes.yaml"
        routes.write_text(json.dumps(route_doc or {"providers": [], "model_routes": []}), encoding="utf-8")
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            self.port = listener.getsockname()[1]
        allow = {"SYSTEMROOT", "WINDIR", "COMSPEC", "PATH", "PATHEXT", "TEMP", "TMP"}
        env = {key: value for key, value in os.environ.items() if key.upper() in allow}
        env.update(PORT=str(self.port), GATEWAY_STORAGE_MODE="server", GATEWAY_RUNTIME_ROLE="standalone",
                   GATEWAY_DATA_DIR=str(root / "data"), GATEWAY_BIND_HOST="127.0.0.1", GATEWAY_ROUTES_FILE=str(routes),
                   GATEWAY_RELEASE_PAYLOAD_ROOT=str(package), GATEWAY_MANAGEMENT_TOKEN=token,
                   GATEWAY_API_KEY_SECRET=signing_secret,
                   GATEWAY_DATABASE_URL=pg, GATEWAY_REDIS_URL=redis, GATEWAY_CONSOLE_REDIS_NAMESPACE=namespace,
                   **{f"GATEWAY_DEVTEST_{provider}_PROXY": fixture.proxy,
                      f"GATEWAY_DEVTEST_{provider}_CA": str(ca if ca is not None else fixture.ca)})
        if disable_system_proxy:
            # Pin generic loopback transport without changing Windows or the explicit SAP fixture proxy.
            state = root / "fixture-state"
            state.mkdir()
            (state / "outbound-proxy.json").write_text('{"mode":"off"}\n', encoding="utf-8")
            env["GATEWAY_STATE_DIR"] = str(state)
        if provider == "SAP_AICORE":
            env["GATEWAY_DEVTEST_SAP_AICORE_MTLS"] = fixture.mtls
        with (root / "stdout.log").open("xb") as out, (root / "stderr.log").open("xb") as err:
            self.process = subprocess.Popen([str(package / "gateway.exe")], cwd=root, env=env, stdout=out, stderr=err,
                                            stdin=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)

    def request(self, path, body=None, *, expected=200, bearer=None, management=False, secrets=(), grant=None, timeout=10, method=None):
        headers = {"Content-Type": "application/json"}
        if management:
            headers["X-Management-Token"] = self.token
        if grant:
            headers["X-Secret-Grant"] = grant
        if bearer:
            headers["Authorization"] = f"Bearer {bearer}"
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=timeout)
        try:
            conn.request(method or ("GET" if body is None else "POST"), path,
                         None if body is None else json.dumps(body), headers)
            with conn.getresponse() as reply:
                raw = reply.read(1024 * 1024 + 1)
                require(len(raw) <= 1024 * 1024, f"{path}: response exceeded fixture size")
                text = raw.decode("utf-8")
                require(not any(secret in text for secret in (self.token, *secrets)), f"{path}: returned a fixture secret")
                if reply.status not in (expected if isinstance(expected, tuple) else (expected,)):
                    code = response_error_code(text)
                    suffix = f" ({code})" if code else ""
                    # Never include the response body or query parameters in diagnostics.
                    raise RuntimeError(f"{path.partition('?')[0]}: expected HTTP {expected}, got HTTP {reply.status}{suffix}")
                return text, reply.headers.get_content_type()
        finally:
            conn.close()

    def management(self, path, body=None, **options):
        raw, _ = self.request("/v1/internal/gateway/" + path, body, management=True, **options)
        return json.loads(raw) if raw else None

    def ready(self):
        end = time.monotonic() + 45
        while time.monotonic() < end:
            require(self.process.poll() is None, "Fixture backend exited before readiness; inspect isolated logs")
            try:
                raw, _ = self.request("/readyz")
            except (OSError, RuntimeError):
                time.sleep(0.25)
                continue
            state = json.loads(raw)
            require(all(state.get(key) is value for key, value in
                        {"database": True, "redis": True, "redis_required": self.redis_required_for_public_routes,
                         "degraded": False}.items()),
                    "Unexpected Server readiness: " + repr({key: state.get(key) for key in
                        ("database", "redis", "redis_required", "degraded", "runtime_role")}))
            return
        raise RuntimeError("Fixture backend readiness timed out")

    def drain(self):
        self.management("runtime/drain", {"reason": self.drain_reason}, expected=202)
        require(self.process.wait(timeout=20) == 0, "Fixture backend did not drain successfully")

    def close(self, *, graceful=False):
        if self.process.poll() is not None:
            return
        if graceful:
            try:
                self.drain()
                return
            except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
                self.cleanup_error = type(error).__name__
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
