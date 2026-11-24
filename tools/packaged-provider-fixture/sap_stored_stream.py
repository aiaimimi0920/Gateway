"""Opt-in streamed stored replies, with a barrier after the first SSE frame."""
from dataclasses import dataclass
import json
from threading import Event

from sap_stored import PATH, Stored
from server_process import require


def event(name, sequence, **fields):
    data = json.dumps(dict(type=name, sequence_number=sequence, **fields))
    return f"event: {name}\ndata: {data}\n\n".encode()


@dataclass(frozen=True)
class StreamReply:
    first: bytes
    tail: bytes


class StoredStream(Stored):
    def __init__(self):
        super().__init__()
        self.release = (Event(), Event())

    def accept(self, method, path):
        if method == "POST" and path == PATH:
            return "text/event-stream"
        return super().accept(method, path)

    def reply(self, method, path, body, *, recorded=False):
        if method != "POST" or path != PATH:
            return super().reply(method, path, body, recorded=recorded)
        with self.lock:
            if not recorded:
                self.attempts.append((method, path))
            if (not isinstance(body, dict) or body.get("store") is not True
                    or body.get("stream") is not True or body.get("input") != "fixture-stored"
                    or body.get("background", False) is not False
                    or "model" in body or "previous_response_id" in body):
                return 400, {}
            index = len(self.rows)
            if index >= len(self.release):
                return 429, {}
            identity = f"resp_fixture_{index + 1}"
            self.rows[identity] = "queued"
            created = dict(self.response(identity), status="in_progress")
            self.calls.append((method, path))
            delta = event("response.output_text.delta", 1, item_id="msg_fixture",
                          output_index=0, content_index=0, delta="fixture answer")
            if index == 0:
                tail = delta + event("response.completed", 2, response=dict(
                    created, status="completed", usage=dict(input_tokens=2, output_tokens=3, total_tokens=5)))
            else:
                tail = delta + b"event: response.completed\ndata: {"
            return 200, (index, StreamReply(event("response.created", 0, response=created), tail))

    def send(self, request, body):
        if not isinstance(body, tuple):
            return super().send(request, body)
        index, reply = body
        request.send_response(200)
        request.send_header("Content-Length", str(len(reply.first) + len(reply.tail)))
        request.send_header("Content-Type", "text/event-stream")
        request.end_headers()
        request.wfile.write(reply.first)
        request.wfile.flush()
        require(self.release[index].wait(15), "Stored SSE fixture barrier timed out")
        if index == 0:
            with self.lock:
                self.rows["resp_fixture_1"] = "completed"
        request.wfile.write(reply.tail)
