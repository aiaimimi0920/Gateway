"""Non-forwarding TLS peer for the packaged SenseNova acceptance runner."""

import datetime as dt
import json
import secrets
import socketserver
import ssl
import threading
import uuid
from http.server import BaseHTTPRequestHandler
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

from native_replies import event_stream, messages, responses

HOST = "token.sensenova.cn"
MODEL = "deepseek-flash"
PATH = "/v1/chat/completions"
CATALOG_MODELS = (MODEL, "fixture-sensenova-text")
NATIVE = {"/v1/messages": messages, "/v1/responses": responses}
ANSWER = "packaged fixture answer"
USAGE = {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5}
MAX_HEAD = 8192
MAX_BODY = 65536


def certificate_files(root: Path, host=HOST):
    """Create unique CA and leaf files in a fresh evidence directory, never an OS store."""
    now = dt.datetime.now(dt.timezone.utc)
    ca_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "Gateway fixture only")])
    leaf_name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, host)])
    ca_key = ec.generate_private_key(ec.SECP256R1())
    leaf_key = ec.generate_private_key(ec.SECP256R1())

    def builder(subject, key):
        return (x509.CertificateBuilder().issuer_name(ca_name).subject_name(subject)
                .public_key(key.public_key()).serial_number(x509.random_serial_number())
                .not_valid_before(now - dt.timedelta(minutes=5))
                .not_valid_after(now + dt.timedelta(hours=1)))

    ca = (builder(ca_name, ca_key).add_extension(x509.BasicConstraints(ca=True, path_length=0), True)
          .sign(ca_key, hashes.SHA256()))
    leaf = (builder(leaf_name, leaf_key).add_extension(x509.BasicConstraints(ca=False, path_length=None), True)
            .add_extension(x509.SubjectAlternativeName([x509.DNSName(host)]), False)
            .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), False)
            .sign(ca_key, hashes.SHA256()))
    ca_path, leaf_path, key_path = [root / name for name in ("fixture-ca.pem", "leaf.pem", "leaf-key.pem")]
    for path, value in (
        (ca_path, ca.public_bytes(serialization.Encoding.PEM)),
        (leaf_path, leaf.public_bytes(serialization.Encoding.PEM)),
        (key_path, leaf_key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                                          serialization.NoEncryption())),
    ):
        with path.open("xb") as target:
            target.write(value)
    return ca_path, leaf_path, key_path


class Fixture(socketserver.ThreadingTCPServer):
    """The only socket operation is accepting loopback; no CONNECT is ever forwarded."""

    daemon_threads = True
    host = HOST
    model = MODEL
    key_prefix = "cpk-local-fixture-"
    post_paths = (PATH, *NATIVE)
    reply = staticmethod(lambda request, stream, path: build_reply(stream, path))

    def __init__(self, root: Path):
        self.key = self.key_prefix + secrets.token_hex(16)
        self.ca, leaf, key = certificate_files(root, self.host)
        self.tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.tls.minimum_version = ssl.TLSVersion.TLSv1_2
        self.tls.load_cert_chain(leaf, key)
        self.tls.set_alpn_protocols(["http/1.1"])
        self._calls = []
        self._post_attempts = 0
        self._catalog_calls = 0
        self._tls_rejections = 0
        self._lock = threading.Lock()
        super().__init__(("127.0.0.1", 0), Tunnel)

    @property
    def proxy(self):
        return f"127.0.0.1:{self.server_address[1]}"

    def calls(self):
        with self._lock:
            return list(self._calls)

    @property
    def post_attempts(self):
        with self._lock:
            return self._post_attempts

    def record_post_attempt(self):
        with self._lock:
            self._post_attempts += 1

    @property
    def catalog_calls(self):
        with self._lock:
            return self._catalog_calls

    def record_catalog(self):
        with self._lock:
            self._catalog_calls += 1

    @property
    def tls_rejections(self):
        with self._lock:
            return self._tls_rejections

    def record_tls_rejection(self):
        with self._lock:
            self._tls_rejections += 1

    def record(self, path, stream):
        with self._lock:
            self._calls.append({"host": self.host, "path": path, "model": self.model, "stream": stream})


class Tunnel(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.settimeout(5)
        try:
            head = b""
            while b"\r\n\r\n" not in head and len(head) < MAX_HEAD:
                data = self.request.recv(min(1024, MAX_HEAD - len(head)))
                if not data:
                    return
                head += data
            line = head.split(b"\r\n", 1)[0]
            expected = f"CONNECT {self.server.host}:443 HTTP/1.1".encode("ascii")
            if line != expected or not head.endswith(b"\r\n\r\n"):
                self.request.sendall(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                return
            self.request.sendall(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            try:
                conn = self.server.tls.wrap_socket(self.request, server_side=True)
            except ssl.SSLError:
                self.server.record_tls_rejection()
                return
            with conn:
                SyntheticInference(conn, self.client_address, self.server)
        except OSError:
            # Probes with the wrong trust root and client cancellation intentionally disconnect.
            return


class SyntheticInference(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # Never log request headers, keys, or body.

    def do_GET(self):
        if self.server.host != HOST or self.path != "/v1/models" or self.headers.get("Host") != HOST:
            self.send_error(404)
            return
        if self.headers.get_all("Authorization") != [f"Bearer {self.server.key}"] or self.headers.get("Cookie"):
            self.send_error(401)
            return
        self.server.record_catalog()
        text = [{"id": model, "output_modalities": ["text"]} for model in CATALOG_MODELS]
        self.send_payload(json.dumps({"data": [*text, text[0],
            {"id": "sensenova-u-video", "output_modalities": ["video"]},
            {"id": "fixture-image-only", "output_modalities": ["image"]}]}).encode(), "application/json")

    def do_POST(self):
        # Include rejected wire/auth/body attempts; successful replies alone cannot prove zero send.
        self.server.record_post_attempt()
        if self.path not in self.server.post_paths or self.headers.get("Host") != self.server.host:
            self.send_error(404)
            return
        if self.headers.get_all("Authorization") != [f"Bearer {self.server.key}"]:
            self.send_error(401)
            return
        length = self.headers.get_all("Content-Length", [])
        if (len(length) != 1 or not length[0].isascii() or not length[0].isdecimal()
                or len(length[0]) > 6 or not 0 < int(length[0]) <= MAX_BODY):
            self.send_error(400)
            return
        try:
            request = json.loads(self.rfile.read(int(length[0])))
        except (ValueError, UnicodeError):
            self.send_error(400)
            return
        if not isinstance(request, dict) or request.get("model") != self.server.model:
            self.send_error(400)
            return
        stream = request.get("stream") is True
        result = self.server.reply(request, stream, self.path)
        if result is None:
            self.send_error(400)
            return
        self.server.record(self.path, stream)
        self.send_payload(*result)

    def send_payload(self, reply, kind):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(reply)))
        self.end_headers()
        self.wfile.write(reply)


def build_reply(stream, path=PATH, *, model=MODEL):
    if path in NATIVE:
        result = NATIVE[path](model, ANSWER, stream, uuid.uuid4().hex)
        return (event_stream(result), "text/event-stream") if stream else (json.dumps(result).encode(), "application/json")
    response_id = "chatcmpl-fixture-" + uuid.uuid4().hex
    common = {"id": response_id, "created": 1700000000, "model": model}
    message = {"role": "assistant", "content": ANSWER}
    if not stream:
        return json.dumps(dict(common, object="chat.completion", usage=USAGE,
                               choices=[{"index": 0, "message": message, "finish_reason": "stop"}])
                          ).encode(), "application/json"
    common["object"] = "chat.completion.chunk"
    frames = [dict(common, choices=[{"index": 0, "delta": message, "finish_reason": None}]),
              dict(common, choices=[{"index": 0, "delta": {}, "finish_reason": "stop"}]),
              dict(common, choices=[], usage=USAGE)]
    return ("".join(f"data: {json.dumps(frame)}\n\n" for frame in frames) + "data: [DONE]\n\n").encode(), "text/event-stream"
