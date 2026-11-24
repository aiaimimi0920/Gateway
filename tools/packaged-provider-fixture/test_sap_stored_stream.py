"""Offline contracts for the opt-in packaged stored Responses SSE fixture."""
from contextlib import closing
from http.client import HTTPSConnection
from io import BytesIO
import json
import os
from pathlib import Path
import ssl
import unittest
import uuid

from sap_peer import AZURE, GROUP, SAP
from sap_stored import PATH, Stored
from sap_stored_stream import StoredStream, event
from sap_stored_stream_cases import read_event


REQUEST = dict(store=True, stream=True, input="fixture-stored")


class StoredStreamTests(unittest.TestCase):
    def test_loopback_tls_flushes_first_frame_before_release(self):
        root = Path(os.environ["GATEWAY_FIXTURE_EVIDENCE_ROOT"]) / ("sap-stored-tls-" + uuid.uuid4().hex)
        root.mkdir()
        peer = SAP(root)
        peer.stored = StoredStream()
        with peer.serve():
            ctx = ssl.create_default_context(cafile=str(peer.ca))
            _, port = peer.proxy.rsplit(":", 1)
            try:
                with closing(HTTPSConnection("127.0.0.1", int(port), context=ctx, timeout=5)) as conn:
                    conn.set_tunnel(AZURE, 443)
                    conn.request("POST", PATH, json.dumps(REQUEST),
                                 {"Host": AZURE, "AI-Resource-Group": GROUP, "Accept": "text/event-stream",
                                  "Authorization": "Bearer " + peer.tokens[AZURE]})
                    with conn.getresponse() as reply:
                        self.assertEqual(reply.status, 200)
                        self.assertEqual(reply.headers.get_content_type(), "text/event-stream")
                        first = read_event(reply, ())
                        identity = first["response"]["id"]
                        self.assertEqual(first["type"], "response.created")
                        self.assertEqual(peer.stored.rows[identity], "queued")
                        peer.stored.release[0].set()
                        self.assertEqual(read_event(reply, ())["delta"], "fixture answer")
                        self.assertEqual(read_event(reply, ())["type"], "response.completed")
                        self.assertEqual(reply.read(1), b"")
                        self.assertEqual(peer.stored.rows[identity], "completed")
            finally:
                peer.stored.release[0].set()
        self.assertEqual(peer.stored.snapshot(), [("POST", PATH)])
        self.assertEqual(peer.snapshot()[0], [dict(host=AZURE, path=PATH, stream=True)])

    def test_opt_in_leaves_json_contract_unchanged(self):
        old = Stored()
        self.assertEqual(old.accept("POST", PATH), "application/json")
        self.assertEqual(old.reply("POST", PATH, REQUEST)[0], 400)
        self.assertEqual(old.reply("POST", PATH, dict(REQUEST, stream=False, background=True))[0], 200)
        live = StoredStream()
        self.assertEqual(live.accept("POST", PATH), "text/event-stream")
        self.assertEqual(live.accept("GET", PATH + "/resp_fixture_2"), "application/json")
        for bad in (dict(REQUEST, stream=False), dict(REQUEST, model="private"),
                    dict(REQUEST, background=True), dict(REQUEST, previous_response_id=None)):
            self.assertEqual(live.reply("POST", PATH, bad)[0], 400)
        self.assertEqual(live.snapshot(), [])
        self.assertEqual(live.all_attempts(), [("POST", PATH)] * 4)
        self.assertEqual(live.rows, {})

    def test_success_and_truncation_have_distinct_ids_and_get_evidence(self):
        live = StoredStream()
        status, (index, complete) = live.reply("POST", PATH, REQUEST)
        self.assertEqual((status, index), (200, 0))
        created = read_event(BytesIO(complete.first), ())
        self.assertEqual(created["response"]["status"], "in_progress")
        self.assertNotIn("usage", created["response"])
        frames = BytesIO(complete.tail)
        self.assertEqual(read_event(frames, ())["delta"], "fixture answer")
        terminal = read_event(frames, ())
        self.assertEqual(terminal["response"]["usage"]["total_tokens"], 5)
        self.assertEqual(terminal["response"]["id"], created["response"]["id"])
        self.assertFalse(frames.read())
        status, (index, truncated) = live.reply("POST", PATH, REQUEST)
        self.assertEqual((status, index), (200, 1))
        second = read_event(BytesIO(truncated.first), ())["response"]["id"]
        self.assertNotEqual(created["response"]["id"], second)
        frames = BytesIO(truncated.tail)
        self.assertEqual(read_event(frames, ())["type"], "response.output_text.delta")
        with self.assertRaises(RuntimeError):
            read_event(frames, ())
        self.assertEqual(live.rows[second], "queued")
        status, recovered = live.reply("GET", PATH + "/" + second, None)
        self.assertEqual((status, recovered["id"], recovered["status"]), (200, second, "completed"))
        self.assertEqual(live.snapshot(), live.all_attempts())
        self.assertEqual(live.reply("POST", PATH, REQUEST)[0], 429)

    def test_client_rejects_secrets_size_and_envelope_drift(self):
        for body in (
            event("response.output_text.delta", 1, delta="never-publish"),
            event("response.output_text.delta", 1, delta="x" * 8192),
            b'event: a\ndata: {"type":"b"}\n\n',
            b'event: a\ndata: {"type":"a"}\n',
        ):
            with self.assertRaises(RuntimeError):
                read_event(BytesIO(body), ("never-publish",))


if __name__ == "__main__":
    unittest.main()
