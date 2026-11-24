"""Opt-in stateful TLS peer; never broadens the existing stateless SAP fixture."""
import json
import threading
from urllib.parse import urlsplit

DEPLOY = "/v2/inference/deployments/fixture-stored"
PATH = DEPLOY + "/responses"


class Stored:
    def __init__(self):
        self.lock = threading.Lock()
        self.rows = {}
        self.calls = []
        self.attempts = []
        self._complete_next = None
        self._without_usage = set()

    def complete_next_create(self, *, usage=True):
        with self.lock:
            self._complete_next = usage

    def record_attempt(self, method, path):
        with self.lock:
            self.attempts.append((method, path))

    def reply(self, method, path, body, *, recorded=False):
        with self.lock:
            if not recorded:
                self.attempts.append((method, path))
            if method == "POST" and path == PATH:
                if (not isinstance(body, dict) or body.get("store") is not True
                        or body.get("stream", False) is not False or "model" in body
                        or body.get("input") != "fixture-stored" or body.get("background") is not True):
                    return 400, {}
                previous = body.get("previous_response_id")
                if len(self.rows) == 1 and previous != "resp_fixture_1":
                    return 400, {}
                if previous is not None and self.rows.get(previous) != "completed":
                    return 400, {}
                if len(self.rows) >= 8:
                    return 429, {}
                identity = f"resp_fixture_{len(self.rows) + 1}"
                self.rows[identity] = "completed" if self._complete_next is not None else "queued"
                if self._complete_next is False:
                    self._without_usage.add(identity)
                self._complete_next = None
                self.calls.append((method, path))
                return 200, self.response(identity)
            suffix = path.removeprefix(PATH + "/")
            identity = suffix.removesuffix("/cancel")
            if identity not in self.rows:
                return 404, {}
            if method == "GET" and suffix == identity:
                if self.rows[identity] == "deleted":
                    return 404, {}
                if self.rows[identity] == "queued":
                    self.rows[identity] = "completed"
            elif method == "POST" and suffix == identity + "/cancel" and body is None:
                if self.rows[identity] != "queued":
                    return 409, {}
                self.rows[identity] = "cancelled"
            elif method == "DELETE" and suffix == identity:
                if self.rows[identity] not in ("completed", "cancelled"):
                    return 409, {}
                self.rows[identity] = "deleted"
                self.calls.append((method, path))
                return 200, {"id": identity, "object": "response.deleted", "deleted": True}
            else:
                return 400, {}
            self.calls.append((method, path))
            return 200, self.response(identity)

    def response(self, identity):
        status = self.rows[identity]
        body = dict(id=identity, object="response", status=status, model="private-stored",
                    store=True, output=[], metadata={"private": "never-publish"})
        if status != "queued" and identity not in self._without_usage:
            body["usage"] = dict(input_tokens=2, output_tokens=3, total_tokens=5)
        return body

    def snapshot(self):
        with self.lock:
            return list(self.calls)

    def all_attempts(self):
        with self.lock:
            return list(self.attempts)

    def accept(self, method, path):
        return "application/json"

    def send(self, request, body):
        request.send_payload(json.dumps(body).encode(), "application/json")


def handle(request):
    peer, host = request.server.server.peer, request.server.authority
    state = getattr(peer, "stored", None)
    path = urlsplit(request.path)
    if state is None or not (path.path == PATH or path.path.startswith(PATH + "/")):
        return False
    state.record_attempt(request.command, request.path)
    from sap_peer import AZURE, GROUP
    if (host != AZURE or path.query or request.headers.get_all("Host") != [host]
            or request.headers.get_all("Authorization") != [f"Bearer {peer.tokens[host]}"]
            or request.headers.get_all("AI-Resource-Group") != [GROUP]
            or request.headers.get_all("Accept") != [state.accept(request.command, path.path)]):
        request.send_error(400)
        return True
    lengths = request.headers.get_all("Content-Length", ["0"])
    if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdecimal() or len(lengths[0]) > 6:
        request.send_error(400)
        return True
    size = int(lengths[0])
    if size > 64 * 1024:
        request.send_error(413)
        return True
    try:
        body = json.loads(request.rfile.read(size)) if size else None
    except (ValueError, UnicodeError):
        request.send_error(400)
        return True
    status, body = state.reply(request.command, path.path, body, recorded=True)
    if status != 200:
        request.send_error(status)
    else:
        peer.record(host, path.path, state.accept(request.command, path.path) == "text/event-stream")
        state.send(request, body)
    return True
