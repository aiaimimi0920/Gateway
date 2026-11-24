import http.client
import json
import os
from pathlib import Path
import ssl
import threading
import unittest
import uuid

import commandcode
from scheduler_product import SENSENOVA

from sensenova import ANSWER, CATALOG_MODELS, HOST, MODEL, PATH, USAGE, Fixture
from server_process import response_error_code


class DiagnosticTest(unittest.TestCase):
    def test_extracts_only_structured_error_code(self):
        self.assertEqual(response_error_code('{"error":{"code":"balance_not_initialized","message":"not printed"}}'),
                         "balance_not_initialized")
        for invalid in ('<html>error</html>', '[]', '{"code":"top_level"}', '{"error":{"code":429}}',
                        '{"error":{"code":"unsafe value"}}'):
            with self.subTest(invalid=invalid):
                self.assertIsNone(response_error_code(invalid))


class FixtureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(os.environ["GATEWAY_FIXTURE_EVIDENCE_ROOT"]) / ("tls-peer-" + uuid.uuid4().hex)
        root.mkdir()
        cls.peer = Fixture(root)
        cls.thread = threading.Thread(target=cls.peer.serve_forever)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.peer.shutdown()
        cls.thread.join(timeout=5)
        cls.peer.server_close()

    def request(self, *, target=HOST, key=None, context=None, stream=False, path=PATH):
        host, port = self.peer.server_address
        conn = http.client.HTTPSConnection(host, port, timeout=5, context=context or self.trusted())
        conn.set_tunnel(target, 443)
        try:
            # Verify auth precedes body validation without racing a Windows TCP reset
            # when the peer rejects headers before reading unauthorized body bytes.
            body = None if key is not None else json.dumps({"model": MODEL, "stream": stream})
            conn.request("POST", path, body, {"Host": target, "Authorization": f"Bearer {key or self.peer.key}"})
            with conn.getresponse() as reply:
                return reply.status, reply.read()
        finally:
            conn.close()

    def trusted(self):
        return ssl.create_default_context(cafile=str(self.peer.ca))

    def test_successful_json_and_sse_keep_unique_id_and_usage(self):
        before = len(self.peer.calls())
        status, body = self.request()
        self.assertEqual(status, 200)
        buffered = json.loads(body)
        self.assertEqual(buffered["choices"][0]["message"]["content"], ANSWER)
        self.assertEqual(buffered["usage"], USAGE)
        status, body = self.request(stream=True)
        self.assertEqual(status, 200)
        self.assertTrue(body.endswith(b"data: [DONE]\n\n"))
        frames = [json.loads(line[6:]) for line in body.splitlines() if line.startswith(b"data: {")]
        self.assertEqual(frames[0]["choices"][0]["delta"]["content"], ANSWER)
        self.assertEqual(frames[-1]["usage"], USAGE)
        self.assertNotEqual(frames[0]["id"], buffered["id"])
        self.assertEqual(self.peer.calls()[before:], [
            {"host": HOST, "path": PATH, "model": MODEL, "stream": stream} for stream in (False, True)
        ])

    def test_catalog_is_authenticated_and_separate_from_inference(self):
        def read(key):
            conn = http.client.HTTPSConnection(*self.peer.server_address, timeout=5, context=self.trusted())
            conn.set_tunnel(HOST, 443)
            try:
                conn.request("GET", "/v1/models", headers={"Host": HOST, "Authorization": f"Bearer {key}"})
                with conn.getresponse() as reply:
                    return reply.status, reply.read() if reply.status == 200 else b""
            finally:
                conn.close()

        inference_before, catalog_before = self.peer.calls(), self.peer.catalog_calls
        self.assertEqual(read("wrong")[0], 401)
        self.assertEqual(self.peer.catalog_calls, catalog_before)
        status, wire = read(self.peer.key)
        self.assertEqual(status, 200)
        rows = json.loads(wire)["data"]
        self.assertEqual([x["id"] for x in rows[:2]], list(CATALOG_MODELS))
        self.assertEqual(len(rows), 5)
        self.assertEqual(self.peer.catalog_calls, catalog_before + 1)
        self.assertEqual(self.peer.calls(), inference_before)

    def test_native_messages_and_responses(self):
        before = len(self.peer.calls())
        for path, terminal in (("/v1/messages", "message_stop"), ("/v1/responses", "response.completed")):
            with self.subTest(path=path):
                status, body = self.request(path=path)
                self.assertEqual(status, 200)
                parsed = json.loads(body)
                self.assertIn(ANSWER, body.decode())
                self.assertEqual((parsed["usage"]["input_tokens"], parsed["usage"]["output_tokens"]), (2, 3))
                status, body = self.request(path=path, stream=True)
                self.assertEqual(status, 200)
                lines = body.decode().splitlines()
                self.assertEqual(lines[-3], f"event: {terminal}")
                opening = json.loads(lines[1][6:])
                self.assertNotEqual(opening["message" if path.endswith("messages") else "response"]["id"], parsed["id"])
        self.assertEqual(len(self.peer.calls()) - before, 4)

    def test_refuses_non_allowlisted_host_wrong_token_and_untrusted_ca(self):
        before = len(self.peer.calls())
        attempts = self.peer.post_attempts
        with self.assertRaisesRegex(OSError, "403"):
            self.request(target="not-the-fixture.invalid")
        self.assertEqual(self.request(key="wrong")[0], 401)
        with self.assertRaises(ssl.SSLCertVerificationError):
            self.request(context=ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT))
        self.assertEqual(len(self.peer.calls()), before)
        self.assertEqual(self.peer.post_attempts, attempts + 1)


class SchedulerFixtureTest(unittest.TestCase):
    def test_product_scheduler_peers_keep_separate_identity_and_exact_chat_shape(self):
        for product, foreign_host, foreign_path in [(commandcode.COMMANDCODE, HOST, PATH),
                                                    (SENSENOVA, commandcode.HOST, commandcode.PATH)]:
            with self.subTest(product=product.display_name):
                self.check_wire(product, foreign_host, foreign_path)

    def check_wire(self, product, foreign_host, foreign_path):
        root = Path(os.environ["GATEWAY_FIXTURE_EVIDENCE_ROOT"]) / ("scheduler-tls-" + uuid.uuid4().hex)
        root.mkdir()
        scope = product.scope_prefix + uuid.uuid4().hex
        with product.peer(root, scope) as peer:
            thread = threading.Thread(target=peer.serve_forever)
            thread.start()
            host, path = peer.host, peer.post_paths[0]
            try:
                def send(prompt, *, target=host, endpoint=path, stream=False, max_tokens=256):
                    conn = http.client.HTTPSConnection(*peer.server_address, timeout=5,
                                                       context=ssl.create_default_context(cafile=str(peer.ca)))
                    conn.set_tunnel(target, 443)
                    try:
                        body = None if endpoint != path else json.dumps({"model": scope, "stream": stream,
                            "messages": [{"role": "user", "content": prompt}], "max_tokens": max_tokens, "n": 1})
                        conn.request("POST", endpoint, body, {"Host": target, "Authorization": f"Bearer {peer.key}"})
                        with conn.getresponse() as reply:
                            return reply.status, reply.read() if reply.status == 200 else b""
                    finally:
                        conn.close()

                with self.assertRaisesRegex(OSError, "403"):
                    send(commandcode.PROMPT, target=foreign_host)
                self.assertEqual(send(commandcode.PROMPT, endpoint=foreign_path)[0], 404)
                self.assertEqual(send(commandcode.PROMPT, stream=True)[0], 400)
                self.assertEqual(send("wrong prompt")[0], 400)
                if product == SENSENOVA:
                    self.assertEqual(send(commandcode.PROMPT, max_tokens=10)[0], 400)
                self.assertEqual(peer.calls(), [])
                status, body = send(commandcode.PROMPT)
                self.assertEqual(status, 200)
                parsed = json.loads(body)
                self.assertEqual(parsed["model"], scope)
                self.assertEqual(parsed["usage"], USAGE)
                self.assertEqual(peer.calls(), [{"host": host, "path": path, "model": scope, "stream": False}])
            finally:
                peer.shutdown()
                thread.join(timeout=5)


if __name__ == "__main__":
    unittest.main()
