"""Independent tests for the fixed, non-forwarding Hosted peer."""
from contextlib import ExitStack
import http.client
import json
import os
from pathlib import Path
import ssl
import threading
import unittest
import uuid

from hosted_product import COMMANDCODE_HOSTED, ENDPOINTS
from run_sensenova_hosted import TERMINAL, assert_public


class CommandCodeHostedTest(unittest.TestCase):
    def test_fixed_catalogue_and_public_assertions(self):
        payload = COMMANDCODE_HOSTED.payload()
        upstream_model, = payload["supported_models"]
        self.assertEqual(payload["model_catalog"]["supportedEndpoints"][upstream_model],
                         [f"/{endpoint}" for endpoint in ENDPOINTS])
        with self.assertRaisesRegex(RuntimeError, "exposed the upstream model"):
            assert_public("chat/completions", True, "public-alias",
                          f"packaged fixture answer {upstream_model}", "text/event-stream", upstream_model)

    def test_peer_requires_exact_images_and_three_native_targets(self):
        product = COMMANDCODE_HOSTED
        root = Path(os.environ["GATEWAY_FIXTURE_EVIDENCE_ROOT"]) / ("commandcode-peer-" + uuid.uuid4().hex)
        root.mkdir()
        with ExitStack() as stack:
            peer = stack.enter_context(product.peer(root))
            server = threading.Thread(target=peer.serve_forever)
            server.start()
            stack.callback(server.join, 5)
            stack.callback(peer.shutdown)
            trust = ssl.create_default_context(cafile=str(peer.ca))

            def request(path, body):
                conn = http.client.HTTPSConnection(*peer.server_address, context=trust, timeout=5)
                conn.set_tunnel(peer.host, 443)
                try:
                    conn.request("POST", path, json.dumps(body),
                                 {"Host": peer.host, "Authorization": f"Bearer {peer.key}"})
                    with conn.getresponse() as reply:
                        return reply.status, reply.headers.get_content_type(), reply.read().decode()
                finally:
                    conn.close()

            for endpoint in ENDPOINTS:
                path = f"{product.base_path}/{endpoint}"
                for stream in (False, True):
                    status, kind, reply = request(path, product.request_body(endpoint, peer.model, stream))
                    self.assertEqual(status, 200)
                    self.assertIn(peer.model, reply)
                    if stream:
                        self.assertEqual(kind, "text/event-stream")
                        self.assertEqual(reply.splitlines().count(TERMINAL[endpoint]), 1)
                    else:
                        self.assertEqual(kind, "application/json")
            self.assertEqual(len(peer.calls()), 6)
            bad = product.request_body("chat/completions", peer.model, False)
            bad["messages"][0]["content"] = "missing image"
            self.assertEqual(request(peer.post_paths[0], bad)[0], 400)
            self.assertEqual(len(peer.calls()), 6)


if __name__ == "__main__":
    unittest.main()
