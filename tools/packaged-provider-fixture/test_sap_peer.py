"""Exercise both SAP authentication peers over sockets before starting Gateway."""
from contextlib import closing
from copy import deepcopy
import http.client
import json
import os
from pathlib import Path
import socket
import ssl
import unittest
from urllib.parse import urlencode, urlsplit
import struct
import zlib
import uuid

import sap_native as wire
import sap_tools
from sap_peer import AUTH, AWS, AZURE, CERT_AUTH, DEPLOY, GROUP, PROMPT, SAP, VERSION


class SapPeerTest(unittest.TestCase):
    def test_loopback_secret_and_direct_client_certificate(self):
        root = Path(os.environ["GATEWAY_FIXTURE_EVIDENCE_ROOT"]) / ("sap-peer-" + uuid.uuid4().hex)
        root.mkdir()
        with SAP(root).serve() as peer:
            ctx = ssl.create_default_context(cafile=str(peer.ca))
            oauth = {"grant_type": "client_credentials"}

            def tunnel(host, path, body, headers):
                address, port = peer.proxy.rsplit(":", 1)
                with closing(http.client.HTTPSConnection(address, int(port), context=ctx, timeout=5)) as conn:
                    conn.set_tunnel(host, 443)
                    conn.request("POST", path, body, {"Host": host, **headers})
                    with conn.getresponse() as reply:
                        return reply.status, reply.read()

            form_type = {"Content-Type": "application/x-www-form-urlencoded"}
            status, data = tunnel(AUTH, "/oauth/token", urlencode(oauth), {**form_type, "Authorization": peer.basic})
            self.assertEqual(status, 200)
            self.assertEqual(json.loads(data)["access_token"], peer.tokens[AZURE])
            with self.assertRaisesRegex(OSError, "403 Forbidden"):
                tunnel(CERT_AUTH, "/oauth/token", urlencode(oauth), form_type)

            cert = root / "ephemeral-client.pem"
            cert.write_bytes((peer.client_cert + peer.client_key).encode("ascii"))
            ctx.load_cert_chain(cert)
            address, port = peer.mtls.rsplit(":", 1)
            content = urlencode(dict(oauth, client_id=peer.client_id)).encode("ascii")
            head = (f"POST /oauth/token HTTP/1.1\r\nHost: {CERT_AUTH}:{port}\r\n"
                    f"Content-Length: {len(content)}\r\nContent-Type: {form_type['Content-Type']}\r\n\r\n").encode()
            with socket.create_connection((address, int(port)), timeout=5) as raw:
                with ctx.wrap_socket(raw, server_hostname=CERT_AUTH) as tls:
                    tls.sendall(head + content)
                    response = http.client.HTTPResponse(tls)
                    response.begin()
                    self.assertEqual(response.status, 200)
                    self.assertEqual(json.loads(response.read())["access_token"], peer.tokens[AWS])
            body = json.dumps({"messages": [{"role": "user", "content": PROMPT}]})
            headers = {"AI-Resource-Group": GROUP, "Accept": "application/json", "Authorization": "Bearer " + peer.tokens[AZURE]}
            path = f"{DEPLOY}/chat/completions?api-version={VERSION}"
            self.assertEqual(tunnel(AWS, path, body, headers)[0], 400)
            self.assertEqual(tunnel(AZURE, path, body, headers)[0], 200)
            self.assertEqual([item["host"] for item in peer.snapshot()[0]], [AUTH, CERT_AUTH, AZURE])
            self.assertEqual(peer.oauth_attempts(), 2)
            self.assertEqual(tunnel(AUTH, "/wrong", urlencode(oauth), form_type)[0], 400)
            self.assertEqual(peer.oauth_attempts(), 3)  # Includes rejected OAuth-host POSTs.
            self.assertEqual(len(peer.snapshot()[0]), 3)
            self.assertEqual(peer.inference_attempts(), [{"host": host, "path": path.split("?")[0]}
                                                         for host in (AWS, AZURE)])

class SapNativeTest(unittest.TestCase):
    def test_tool_matrix_requires_exact_history_and_media(self):
        for mode in sap_tools.MODES:
            bedrock = mode.startswith("bedrock")
            for stream in (False, True):
                action = ("converse-stream" if stream else "converse") if bedrock else "v2/completion"
                accept = (wire.EVENTSTREAM if bedrock else wire.SSE) if stream else wire.JSON
                path = urlsplit(f"{wire.DEPLOY_ROOT}/fixture-{mode}/{action}")
                for final in (False, True):
                    with self.subTest(mode=mode, stream=stream, final=final):
                        body = (sap_tools.bedrock_body(mode, final, wire) if bedrock else
                                sap_tools.orchestration_body(mode, stream, final, wire))
                        reply = wire.reply(path, accept, body, "private")
                        self.assertIsNotNone(reply)
                        self.assertEqual((reply[0], reply[2]), (stream, accept))
                        self.assertIsNone(wire.reply(path, accept, dict(body, tools=sap_tools.TOOLS), "private"))
                        wrong = deepcopy(body)
                        if final and bedrock:
                            wrong["messages"][-1]["content"][0]["toolResult"]["toolUseId"] = "wrong"
                        elif final:
                            wrong["config"]["modules"]["prompt_templating"]["prompt"]["template"][0]["tool_call_id"] = "wrong"
                        elif bedrock:
                            wrong["messages"][0]["content"] = [{"text": "wrong"}]
                        else:
                            wrong["placeholder_values"] = {"gateway_content": "wrong"}
                        self.assertIsNone(wire.reply(path, accept, wrong, "private"))

    def test_responses_requires_stateless_reasoning_wire(self):
        path = urlsplit(wire.NATIVE_RESPONSES + "/responses")
        body = {"input": PROMPT, "store": False, "reasoning": {"effort": "low"}}
        disabled, payload, content_type = wire.reply(path, wire.JSON, body, "private")
        self.assertFalse(disabled)
        self.assertEqual(content_type, wire.JSON)
        self.assertEqual(json.loads(payload)["usage"]["total_tokens"], 5)
        for wrong in (dict(body, store=True), dict(body, model="wrong"), dict(body, reasoning={})):
            self.assertIsNone(wire.reply(path, wire.JSON, wrong, "private"))
        self.assertIsNone(wire.reply(urlsplit(path.path + "?api-version=x"), wire.JSON, body, "private"))

    def test_orchestration_requires_fixed_model_and_stream_config(self):
        path = urlsplit(wire.ORCHESTRATION + "/v2/completion")
        model = {"name": wire.ORCHESTRATION_MODEL}
        config = {"modules": {"prompt_templating": {"model": model}}, "stream": {"enabled": True}}
        body = {"config": config, "messages_history": [{"role": "user", "content": PROMPT}]}
        stream, payload, content_type = wire.reply(path, wire.SSE, body, "private")
        self.assertTrue(stream)
        self.assertEqual(content_type, wire.SSE)
        self.assertIn(wire.PRIVATE.encode(), payload)
        self.assertTrue(payload.endswith(b"data: [DONE]\n\n"))
        model["name"] = "wrong"
        self.assertIsNone(wire.reply(path, wire.SSE, body, "private"))

    def test_bedrock_requires_exact_png_and_returns_crc_checked_frames(self):
        path = urlsplit(wire.BEDROCK + "/converse-stream")
        source = {"bytes": wire.PNG}
        body = {"messages": [{"role": "user", "content": [{"text": "inspect"},
                {"image": {"format": "png", "source": source}}]}]}
        stream, payload, content_type = wire.reply(path, wire.EVENTSTREAM, body, "private")
        self.assertTrue(stream)
        self.assertEqual(content_type, wire.EVENTSTREAM)
        frames = 0
        while payload:
            size, header_length, prelude_crc = struct.unpack_from("!III", payload)
            self.assertGreater(size, header_length + 16)
            self.assertLessEqual(size, len(payload))
            self.assertEqual(prelude_crc, zlib.crc32(payload[:8]))
            self.assertEqual(struct.unpack_from("!I", payload, size - 4)[0], zlib.crc32(payload[:size - 4]))
            payload = payload[size:]
            frames += 1
        self.assertEqual(frames, 5)
        source["bytes"] = "wrong"
        self.assertIsNone(wire.reply(path, wire.EVENTSTREAM, body, "private"))

    def test_specialized_chat_requires_image_detail_and_reasoning_budget(self):
        path = urlsplit(wire.CHAT_IMAGE + "/chat/completions?api-version=" + VERSION)
        image = {"url": wire.IMAGE_URL, "detail": "high"}
        body = {"messages": [{"role": "user", "content": [{"type": "text", "text": PROMPT},
                {"type": "image_url", "image_url": image}]}]}
        self.assertIsNotNone(wire.reply(path, wire.JSON, body, "private"))
        del image["detail"]
        self.assertIsNone(wire.reply(path, wire.JSON, body, "private"))
        path = urlsplit(wire.CHAT_REASONING + "/chat/completions?api-version=" + wire.O3_VERSION)
        body = {"messages": [{"role": "user", "content": PROMPT}], "stream": True,
                "reasoning_effort": "high", "max_completion_tokens": 32}
        self.assertIsNotNone(wire.reply(path, wire.SSE, body, "private"))
        self.assertIsNone(wire.reply(path, wire.SSE, dict(body, max_tokens=32), "private"))

    def test_orchestration_images_and_signed_history_are_exact(self):
        image_path = urlsplit(wire.ORCHESTRATION_IMAGE + "/v2/completion")
        model = {"name": wire.ORCHESTRATION_MODEL}
        config = {"modules": {"prompt_templating": {"model": model}}}
        body = {"config": config, "messages_history": [{"role": "user", "content": wire.CHAT_IMAGE_INPUT}]}
        self.assertIsNotNone(wire.reply(image_path, wire.JSON, body, "private"))
        changed = deepcopy(body)
        changed["messages_history"][0]["content"][1]["image_url"]["detail"] = "low"
        self.assertIsNone(wire.reply(image_path, wire.JSON, changed, "private"))
        self.assertIsNone(wire.reply(image_path, wire.JSON, dict(body, sap_orchestration_image_input=True), "private"))
        reasoning_path = urlsplit(wire.ORCHESTRATION_REASONING + "/v2/completion")
        model.update(name=wire.REASONING_MODEL, params={"reasoning_effort": "high"})
        body["messages_history"] = [{"role": "user", "content": PROMPT}]
        _, payload, _ = wire.reply(reasoning_path, wire.JSON, body, "private")
        assistant = json.loads(payload)["final_result"]["choices"][0]["message"]
        self.assertEqual(assistant["reasoning_content"], wire.REASONING)
        body["messages_history"] += [assistant, {"role": "user", "content": "continue"}]
        model["params"]["reasoning_effort"] = "xhigh"
        config["stream"] = {"enabled": True}
        self.assertIsNotNone(wire.reply(reasoning_path, wire.SSE, body, "private"))
        for corrupted in ([{"content": "visible summary"}], list(reversed(wire.REASONING)),
                          [{"content": "visible summary"}, {"signature": "changed"}]):
            changed = deepcopy(body)
            changed["messages_history"][1]["reasoning_content"] = corrupted
            self.assertIsNone(wire.reply(reasoning_path, wire.SSE, changed, "private"))
