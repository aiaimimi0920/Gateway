"""Non-forwarding CONNECT and direct mTLS peers for packaged SAP acceptance."""
from base64 import b64encode
from contextlib import ExitStack, contextmanager
from http.server import BaseHTTPRequestHandler
import json
import secrets
import socketserver
import ssl
import threading
from urllib.parse import parse_qs, urlsplit

import sap_catalog
import sap_native
import sap_stored
from sap_certificates import certificate_files
from sensenova import MAX_BODY, MAX_HEAD, build_reply

AZURE = "api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com"
AWS = "api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com"
AUTH = "gateway-fixture.authentication.eu20.hana.ondemand.com"
CERT_AUTH = "gateway-fixture.authentication.cert.eu20.hana.ondemand.com"
CONNECT_HOSTS = (AUTH, AZURE, AWS)
DEPLOY = "/v2/inference/deployments/fixture-chat"
GROUP = "gateway-fixture"
MODEL = "gpt-4o"
PROMPT = sap_native.PROMPT
VERSION = "2024-10-21"


class SAP:
    def __init__(self, root):
        self.ca, cert, key, self.client_cert, self.client_key, self.client_der = certificate_files(
            root, (*CONNECT_HOSTS, CERT_AUTH))
        self.client_id = "gateway-sap-fixture-client-" + secrets.token_hex(16)
        self.secret = "gateway-sap-fixture-secret-" + secrets.token_hex(16)
        self.tokens = {host: "sap-fixture-token-" + secrets.token_hex(24) for host in (AZURE, AWS)}
        self.basic = "Basic " + b64encode(f"{self.client_id}:{self.secret}".encode()).decode()
        self.connect_tls = self.context(cert, key, client_auth=False)
        self.mtls_tls = self.context(cert, key, client_auth=True)
        self._lock = threading.Lock()
        self._calls = []
        self._rejected = 0
        self._oauth_attempts = 0
        self._inference_attempts = []
        self.oauth_override = None
        self.inference_override = None

    def context(self, cert, key, *, client_auth):
        result = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        result.minimum_version = ssl.TLSVersion.TLSv1_2
        result.set_alpn_protocols(["http/1.1"])
        result.load_cert_chain(cert, key)
        if client_auth:
            result.verify_mode = ssl.CERT_REQUIRED
            result.load_verify_locations(cafile=str(self.ca))
        return result

    @contextmanager
    def serve(self):
        with ExitStack() as stack:
            for mode in ("proxy", "mtls"):
                server = stack.enter_context(Listener(("127.0.0.1", 0), Tunnel))
                server.mode, server.peer = mode, self
                setattr(self, mode, f"127.0.0.1:{server.server_address[1]}")
                thread = threading.Thread(target=server.serve_forever)
                thread.start()
                stack.callback(thread.join, 5)
                stack.callback(server.shutdown)
            yield self

    def service_key(self, certificate):
        host = AWS if certificate else AZURE
        extra = (dict(certurl=f"https://{CERT_AUTH}", certificate=self.client_cert, key=self.client_key)
                 if certificate else dict(url=f"https://{AUTH}", clientsecret=self.secret))
        return json.dumps(dict(clientid=self.client_id, serviceurls={"AI_API_URL": f"https://{host}"}, **extra))

    def token_response(self, host):
        token = self.tokens[AWS if host == CERT_AUTH else AZURE]
        return json.dumps(dict(access_token=token, token_type="Bearer", expires_in=900)).encode()

    def snapshot(self):
        with self._lock:
            return list(self._calls), self._rejected

    def record(self, host, path, stream=None):
        with self._lock:
            self._calls.append({"host": host, "path": path, "stream": stream})

    def record_oauth_attempt(self):
        with self._lock:
            self._oauth_attempts += 1

    def oauth_attempts(self):
        with self._lock:
            return self._oauth_attempts

    def inference_attempts(self):
        with self._lock:
            return list(self._inference_attempts)

    def record_inference_attempt(self, host, path):
        with self._lock:
            self._inference_attempts.append({"host": host, "path": path})

    def reject_tls(self):
        with self._lock:
            self._rejected += 1


class Listener(socketserver.ThreadingTCPServer):
    daemon_threads = True


class Tunnel(socketserver.BaseRequestHandler):
    def handle(self):
        peer = self.server.peer
        self.request.settimeout(5)
        direct = self.server.mode == "mtls"
        try:
            authority = CERT_AUTH if direct else self.connect()
            if authority is None:
                return
            context = peer.mtls_tls if direct else peer.connect_tls
            try:
                tls = context.wrap_socket(self.request, server_side=True)
            except ssl.SSLError:
                peer.reject_tls()
                return
            with tls:
                self.authority, self.direct = authority, direct
                SAPRequest(tls, self.client_address, self)
        except OSError:
            return

    def connect(self):
        header = bytearray()
        while not header.endswith(b"\r\n\r\n") and len(header) < MAX_HEAD:
            chunk = self.request.recv(min(1024, MAX_HEAD - len(header)))
            if not chunk:
                return None
            header.extend(chunk)
        line = header.split(b"\r\n", 1)[0]
        for host in CONNECT_HOSTS:
            if line == f"CONNECT {host}:443 HTTP/1.1".encode() and header.endswith(b"\r\n\r\n"):
                self.request.sendall(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                return host
        self.request.sendall(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        return None


class SAPRequest(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        if sap_stored.handle(self):
            return
        peer, host = self.server.server.peer, self.server.authority
        path = urlsplit(self.path)
        if (host not in (AZURE, AWS) or path.path != sap_catalog.PATH
                or self.headers.get_all("Host") != [host]
                or self.headers.get_all("Authorization") != [f"Bearer {peer.tokens[host]}"]
                or self.headers.get_all("AI-Resource-Group") != [GROUP]
                or self.headers.get_all("Accept") != [sap_native.JSON]):
            return self.send_error(400)
        result = sap_catalog.page(host, path.query)
        if result is None:
            return self.send_error(400)
        protocol, offset, page = result
        peer.record(host, f"{path.path}:{protocol}:{offset}")
        self.send_payload(sap_native.dump(page), sap_native.JSON)

    def do_POST(self):
        if sap_stored.handle(self):
            return
        peer = self.server.server.peer
        host = self.server.authority
        if host in (AUTH, CERT_AUTH):
            peer.record_oauth_attempt()
        else:
            # Count before authentication/body validation, not only successful wire matches.
            peer.record_inference_attempt(host, urlsplit(self.path).path)
        expected_host = f"{CERT_AUTH}:{self.server.server.server_address[1]}" if self.server.direct else host
        if self.headers.get_all("Host") != [expected_host]:
            return self.send_error(400)
        lengths = self.headers.get_all("Content-Length", [])
        if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdecimal() or len(lengths[0]) > 6:
            return self.send_error(400)
        size = int(lengths[0])
        if not 0 < size <= MAX_BODY:
            return self.send_error(400)
        data = self.rfile.read(size)
        if host in (AUTH, CERT_AUTH):
            self.oauth(peer, host, data)
        else:
            self.inference(peer, host, data)

    def do_DELETE(self):
        if not sap_stored.handle(self):
            self.send_error(404)

    def oauth(self, peer, host, data):
        if self.path != "/oauth/token" or self.headers.get_content_type() != "application/x-www-form-urlencoded":
            return self.send_error(400)
        try:
            form = parse_qs(data.decode("ascii"), strict_parsing=True, max_num_fields=2)
        except (UnicodeError, ValueError):
            return self.send_error(400)
        required = {"grant_type": ["client_credentials"]}
        if host == CERT_AUTH:
            required["client_id"] = [peer.client_id]
            allowed = self.connection.getpeercert(binary_form=True) == peer.client_der and not self.headers.get_all("Authorization")
        else:
            allowed = self.headers.get_all("Authorization") == [peer.basic]
        if not allowed or form != required:
            return self.send_error(401)
        if peer.oauth_override is not None and peer.oauth_override(self):
            return
        peer.record(host, self.path)
        self.send_payload(peer.token_response(host), "application/json")

    def inference(self, peer, host, data):
        path = urlsplit(self.path)
        if (self.headers.get_all("Authorization") != [f"Bearer {peer.tokens[host]}"]
                or self.headers.get_all("AI-Resource-Group") != [GROUP]):
            return self.send_error(400)
        try:
            request = json.loads(data)
        except ValueError:
            return self.send_error(400)
        if not isinstance(request, dict) or "model" in request:
            return self.send_error(400)
        if path.path != f"{DEPLOY}/chat/completions":
            reply = sap_native.reply(path, self.headers.get("Accept"), request, MODEL)
            if reply is None:
                return self.send_error(400)
            stream, payload, content_type = reply
            peer.record(host, path.path, stream)
            return self.send_payload(payload, content_type)
        if path.query != f"api-version={VERSION}" or request.get("messages") != [{"role": "user", "content": PROMPT}]:
            return self.send_error(400)
        stream = request.get("stream") is True
        if self.headers.get("Accept") != ("text/event-stream" if stream else "application/json"):
            return self.send_error(400)
        if peer.inference_override is not None and peer.inference_override(self):
            return
        peer.record(host, path.path, stream)
        self.send_payload(*build_reply(stream, model=MODEL))

    def send_payload(self, data, content_type):
        self.send_response(200)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Content-Type", content_type)
        self.end_headers()
        self.wfile.write(data)
