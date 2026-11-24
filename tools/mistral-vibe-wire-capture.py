"""Capture Vibe and Gateway inference locally; never relay to a real upstream."""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import subprocess
import threading
import ssl
import tempfile
import time
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("sandbox", type=Path, help="A disposable linshi folder")
parser.add_argument("vibe", type=Path, help="Path to the official vibe executable")
parser.add_argument("--gateway-test", type=Path, help="Gateway lib-test executable")
parser.add_argument("--codex", type=Path, help="Codex executable for a public /v1/responses probe")
parser.add_argument("--replay-only", action="store_true", help="Skip the public CLI paths")
args = parser.parse_args()
args.vibe = args.vibe.resolve(strict=True)
if args.gateway_test:
    args.gateway_test = args.gateway_test.resolve(strict=True)
if args.codex:
    if not args.gateway_test or args.replay_only:
        parser.error("--codex requires public paths and --gateway-test")
    args.codex = args.codex.resolve(strict=True)
root = args.sandbox.resolve(strict=True)
temporary_root = Path("C:/Users/Public/nas_home/AI/GameEditor/linshi").resolve(strict=True)
if temporary_root not in root.parents:
    parser.error("sandbox must be a subdirectory of linshi")
home = root / "vibe-home"
work = root / "empty-project"
home.mkdir(exist_ok=True)
work.mkdir(exist_ok=True)
(work / "system.txt").write_text("You are a fixture. Reply only OK.", encoding="utf-8")
captures = []
raw_bodies = {}
gateway_address = None
vibe_ingress = []
cert_path = root / "capture-cert.pem"
key_path = root / "capture-key.pem"
cert_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
cert_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "Local Vibe fixture")])
now = datetime.now(timezone.utc)
cert = (x509.CertificateBuilder().subject_name(cert_name).issuer_name(cert_name)
        .public_key(cert_key.public_key()).serial_number(x509.random_serial_number())
        .not_valid_before(now - timedelta(minutes=1)).not_valid_after(now + timedelta(hours=1))
        .add_extension(x509.SubjectAlternativeName([x509.DNSName("api.mistral.ai")]), critical=False)
        .add_extension(x509.BasicConstraints(ca=True, path_length=0), critical=True)
        .sign(cert_key, hashes.SHA256()))
cert_path.write_bytes(cert.public_bytes(serialization.Encoding.PEM))
key_path.write_bytes(cert_key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(cert_path, key_path)


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_CONNECT(self):
        if self.path != "api.mistral.ai:443":
            return self.send_error(403)
        self.send_response_only(200, "Connection Established")
        self.end_headers()
        self.wfile.flush()
        with tls.wrap_socket(self.connection, server_side=True) as conn:
            conn.settimeout(8)
            self.rfile = conn.makefile("rb", self.rbufsize)
            self.wfile = conn.makefile("wb", self.wbufsize)
            self.handle_one_request()
            self.wfile.flush()
        self.close_connection = True

    def reply(self, value, content_type="application/json", status=200):
        body = value if isinstance(value, bytes) else (
            value.encode() if isinstance(value, str) else json.dumps(value).encode())
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        captures.append({"method": self.command, "path": self.path})
        if self.path == "/v1/models":
            self.reply({"object": "list", "data": []})
        else:
            self.reply({"message": "fixture route"}, status=404)

    def do_POST(self):
        try:
            length = int(self.headers.get("Content-Length", 0))
        except ValueError:
            return self.reply({"message": "invalid content length"}, status=400)
        if not 0 <= length <= 256 * 1024:
            return self.reply({"message": "fixture request too large"}, status=413)
        raw = self.rfile.read(length)
        if self.path != "/v1/chat/completions":
            captures.append({"method": self.command, "path": self.path})
            return self.reply({"message": "fixture route"}, status=404)
        if self.headers.get("Authorization") == "Bearer public-api-contract-secret":
            # The pinned CLI may still address the official host for inference.
            # In this test only, deliver that authentic request to the public
            # loopback router. The synthetic upstream uses a different key.
            if gateway_address is None:
                return self.reply({"message": "no loopback Gateway"}, status=503)
            vibe_ingress.append({"body": json.loads(raw), "x_affinity": self.headers.get("X-Affinity")})
            conn = http.client.HTTPConnection(gateway_address, timeout=12)
            try:
                headers = {name: self.headers[name] for name in
                           ("Authorization", "Content-Type", "Accept", "User-Agent", "X-Affinity")
                           if name in self.headers}
                conn.request("POST", self.path, body=raw, headers=headers)
                response = conn.getresponse()
                body = response.read(256 * 1024 + 1)
                if len(body) > 256 * 1024:
                    return self.reply({"message": "Gateway fixture response too large"}, status=502)
                return self.reply(body, response.headers.get("Content-Type", "application/json"),
                                  response.status)
            finally:
                conn.close()
        sender = {"Bearer fixture-only-key": "cli",
                  "Bearer mistral-fixture-secret": "gateway"}.get(self.headers.get("Authorization"))
        if sender is None:
            return self.reply({"message": "only synthetic keys are accepted"}, status=401)
        body = json.loads(raw)
        headers = {k.lower(): v for k, v in self.headers.items() if k.lower() != "authorization"}
        headers["authorization"] = "Bearer <fixture>"
        raw_bodies[sender] = raw
        captures.append({
            "sender": sender, "method": self.command, "path": self.path,
            "http_version": self.request_version, "headers": headers, "body": body,
            "body_bytes": len(raw), "body_sha256": hashlib.sha256(raw).hexdigest(),
            "header_names_in_wire_order": [name for name, _ in self.headers.raw_items()],
        })
        common = {"id": "loopback", "model": body["model"], "created": 1}
        if body.get("stream"):
            chunk = dict(common, object="chat.completion.chunk", choices=[{"index": 0, "delta": {"role": "assistant", "content": "OK"}, "finish_reason": "stop"}])
            usage = dict(common, object="chat.completion.chunk", choices=[],
                         usage={"prompt_tokens": 10, "completion_tokens": 1, "total_tokens": 11})
            self.reply(f"data: {json.dumps(chunk)}\n\ndata: {json.dumps(usage)}\n\ndata: [DONE]\n\n", "text/event-stream")
        else:
            self.reply(dict(common, object="chat.completion", choices=[{"index": 0, "message": {"role": "assistant", "content": "OK"}, "finish_reason": "stop"}]))


def run_vibe(env):
    return subprocess.run(
        [str(args.vibe), "--workdir", str(work), "--trust",
         "--model", "wire-fixture", "--agent", "lean", "--prompt", "Reply OK",
         "--system-prompt-file", str(work / "system.txt"), "--max-turns", "1",
         "--time-limit", "25"], env=env, cwd=work, timeout=40)


def compare(reference, candidate):
    fields = set(reference["body"]) | set(candidate["body"])
    names = set(reference["headers"]) | set(candidate["headers"])
    missing = object()
    return {
        "method_equal": reference["method"] == candidate["method"],
        "path_equal": reference["path"] == candidate["path"],
        "http_version_equal": reference["http_version"] == candidate["http_version"],
        "body_bytes_equal": reference["body_sha256"] == candidate["body_sha256"],
        "header_order_and_case_equal": reference["header_names_in_wire_order"] == candidate["header_names_in_wire_order"],
        "header_values": {k: [reference["headers"].get(k), candidate["headers"].get(k)]
                          for k in ("accept-encoding", "connection", "content-length")},
        "matching_headers": sorted(k for k in names if reference["headers"].get(k) == candidate["headers"].get(k)),
        "different_headers": sorted(k for k in names if reference["headers"].get(k) != candidate["headers"].get(k)),
        "different_body_fields": sorted(k for k in fields if reference["body"].get(k, missing) != candidate["body"].get(k, missing)),
    }


def public_paths(cli, env):
    global gateway_address
    fixture = Path(tempfile.mkdtemp(prefix="gateway-wire-", dir=root))
    address_file = fixture / "gateway-address"
    with subprocess.Popen(
        [str(args.gateway_test), "--ignored", "--nocapture", "mistral_contract_public_wire_fixture"],
        env=dict(env, MISTRAL_WIRE_PUBLIC_FIXTURE=str(fixture), RUST_MIN_STACK="16777216",
                 TEMP=str(fixture), TMP=str(fixture)), cwd=work,
    ) as gateway:
        try:
            deadline = time.monotonic() + 20
            while not address_file.is_file():
                if gateway.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("Gateway loopback listener did not start")
                time.sleep(0.05)
            address = address_file.read_text(encoding="ascii")
            if not address.startswith("127.0.0.1:") or not address[10:].isdigit():
                raise RuntimeError("Gateway fixture must be loopback")
            gateway_address = address
            config = home / "config.toml"
            original_config = config.read_text(encoding="utf-8")
            config.write_text(
                original_config.replace(f"127.0.0.1:{port}/v1", f"{address}/v1"),
                encoding="utf-8",
            )
            try:
                start = len(captures)
                result = run_vibe(dict(env, MISTRAL_API_KEY="public-api-contract-secret"))
                report = {"vibe_via_gateway": public_result(cli, start, result)}
                if vibe_ingress and report["vibe_via_gateway"]["upstream_captures"]:
                    sent = next(item for item in captures[start:] if item.get("sender") == "gateway")
                    ingress = vibe_ingress[0]
                    report["vibe_via_gateway"]["affinity_preserved"] = (
                        ingress["x_affinity"] == sent["headers"].get("x-affinity"))
                    report["vibe_via_gateway"]["rewritten_body_fields"] = sorted(
                        key for key in ingress["body"].keys() | sent["body"].keys()
                        if (key in ingress["body"]) != (key in sent["body"])
                        or ingress["body"].get(key) != sent["body"].get(key))
            finally:
                config.write_text(original_config, encoding="utf-8")
            if args.codex:
                config_root = fixture / "codex-home"
                config_root.mkdir()
                (config_root / "config.toml").write_text(f"""
model = {json.dumps(cli["body"]["model"])}
model_provider = "wire_fixture"
[model_providers.wire_fixture]
name = "Wire fixture"
base_url = "http://{address}/v1"
wire_api = "responses"
env_key = "FIXTURE_CODEX_KEY"
request_max_retries = 0
stream_max_retries = 0
""".lstrip(), encoding="utf-8")
                start = len(captures)
                result = subprocess.run(
                    [str(args.codex), "exec", "--skip-git-repo-check", "--ephemeral", "Reply OK"],
                    env=dict(env, CODEX_HOME=str(config_root),
                             FIXTURE_CODEX_KEY="public-api-contract-secret"), cwd=work, timeout=45,
                )
                report["codex_via_gateway"] = public_result(cli, start, result)
            return report
        finally:
            gateway_address = None
            gateway.terminate()


def public_result(cli, start, result):
    sent = [item for item in captures[start:] if item.get("sender") == "gateway"]
    return {
        "exit_code": result.returncode, "upstream_captures": len(sent),
        "first_request_has_affinity": "x-affinity" in sent[0]["headers"] if sent else None,
        "comparison_to_direct_vibe": compare(cli, sent[0]) if sent else None,
    }


with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
    port = server.server_port
    (home / "config.toml").write_text(f"""
active_model = "wire-fixture"
enable_telemetry = false
enable_otel = false
installed_agents = ["lean"]

[[providers]]
name = "wire-mistral"
api_base = "http://127.0.0.1:{port}/v1"
api_key_env_var = "MISTRAL_API_KEY"
backend = "mistral"
preserve_api_base = true

[[models]]
name = "mistral-large-latest"
provider = "wire-mistral"
alias = "wire-fixture"
""".lstrip(), encoding="utf-8")
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        env = dict(os.environ, VIBE_HOME=str(home), MISTRAL_API_KEY="fixture-only-key",
                   HTTP_PROXY=f"http://127.0.0.1:{port}", HTTPS_PROXY=f"http://127.0.0.1:{port}",
                   ALL_PROXY=f"http://127.0.0.1:{port}", NO_PROXY="127.0.0.1,localhost",
                   SSL_CERT_FILE=str(cert_path), REQUESTS_CA_BUNDLE=str(cert_path))
        result = run_vibe(env)
        cli = next(item for item in captures if item.get("sender") == "cli")
        if args.gateway_test and result.returncode == 0:
            fixture_body = root / "replay-body.json"
            # This captures actual CLI-generated values, not synthetic Vibe session claims.
            fixture_body.write_bytes(raw_bodies["cli"])
            env.update(MISTRAL_WIRE_FIXTURE_PROXY=f"127.0.0.1:{port}",
                       MISTRAL_WIRE_FIXTURE_CA=str(cert_path), MISTRAL_WIRE_FIXTURE_BODY=str(fixture_body))
            result = subprocess.run([str(args.gateway_test), "--ignored", "mistral_contract_replay_captured_cli_request"],
                                    env=env, cwd=work, timeout=25)
            if result.returncode == 0:
                gateway = next(item for item in captures if item.get("sender") == "gateway")
                report = compare(cli, gateway)
                report["body_bytes_equal"] = raw_bodies["cli"] == raw_bodies["gateway"]
                report["public_paths"] = {} if args.replay_only else public_paths(cli, env)
                print(json.dumps(report, indent=2), flush=True)
                (root / "comparison.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
                if (not report["method_equal"] or not report["path_equal"]
                        or not report["http_version_equal"] or not report["body_bytes_equal"]
                        or not report["header_order_and_case_equal"] or report["different_headers"]):
                    raise SystemExit("Gateway transport replay differs from the captured Vibe request")
                if any(path["exit_code"] != 0 or not path["upstream_captures"]
                       for path in report["public_paths"].values()):
                    raise SystemExit("A public CLI path failed; see comparison.json")
    finally:
        (root / "cli-capture.json").write_text(json.dumps(captures, indent=2), encoding="utf-8")
        server.shutdown()
    raise SystemExit(result.returncode)
