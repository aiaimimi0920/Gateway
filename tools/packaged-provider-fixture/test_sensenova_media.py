"""Protect strict media wire and public evidence assertions without a Gateway build."""
from copy import deepcopy
import json
import unittest

from run_sensenova_hosted import assert_public
from sensenova import ANSWER, MODEL, PATH, build_reply
from sensenova_media import (ASSISTANT, MediaPeer, check_first_reply, first_reply,
                            image_request, reasoning_request)


class SenseNovaMediaTest(unittest.TestCase):
    def reply(self, body):
        return MediaPeer.reply(None, body, body.get("stream") is True, PATH)

    def test_images_require_exact_bytes_detail_and_order(self):
        for stream in (False, True):
            body = image_request(MODEL, stream)
            self.assertIsNotNone(self.reply(body))
            for field, value in (("url", "https://image.invalid/other.png"), ("detail", "low")):
                changed = deepcopy(body)
                changed["messages"][0]["content"][1]["image_url"][field] = value
                self.assertIsNone(self.reply(changed))
            changed = deepcopy(body)
            changed["messages"][0]["content"].reverse()
            self.assertIsNone(self.reply(changed))

    def test_reasoning_uses_actual_assistant_and_exact_canonical_wire(self):
        raw, _ = self.reply(reasoning_request(MODEL))
        first = json.loads(raw)
        first["model"] = "public-alias"
        assistant = check_first_reply("public-alias", json.dumps(first), "application/json")
        self.assertEqual(assistant, ASSISTANT)
        body = reasoning_request(MODEL, assistant)
        body["messages"][1]["content"] = ""
        self.assertIsNotNone(self.reply(body))
        for field, value in (("reasoning_content", "trimmed"), ("tool_calls", [])):
            changed = deepcopy(body)
            changed["messages"][1][field] = value
            self.assertIsNone(self.reply(changed))
        changed = deepcopy(body)
        changed["messages"][2]["tool_call_id"] = "unrelated"
        self.assertIsNone(self.reply(changed))

    def test_first_reply_rejects_changed_reasoning_and_double_counted_usage(self):
        reply = first_reply()
        reply["model"] = "alias"
        corrupted = deepcopy(reply)
        corrupted["choices"][0]["message"]["reasoning_content"] = "trimmed"
        with self.assertRaises(RuntimeError):
            check_first_reply("alias", json.dumps(corrupted), "application/json")
        reply["usage"]["total_tokens"] = 7
        with self.assertRaises(RuntimeError):
            check_first_reply("alias", json.dumps(reply), "application/json")

    def test_chat_sse_rejects_missing_duplicate_usage_or_error(self):
        raw, kind = build_reply(True, model="alias")
        text = raw.decode()
        assert_public("chat/completions", True, "alias", text, kind, MODEL)
        usage = next(line for line in text.splitlines() if '"usage"' in line)
        for corrupted in (text.replace(usage, ""), text.replace(usage, usage + "\n\n" + usage),
                          text.replace(ANSWER, ANSWER + " extra"),
                          text.replace('"stop"', '"length"'),
                          text.replace("data: [DONE]", 'data: {"model":"alias","error":{}}\n\ndata: [DONE]')):
            with self.assertRaises(RuntimeError):
                assert_public("chat/completions", True, "alias", corrupted, kind, MODEL)

    def test_chat_json_rejects_extra_answer_wrong_finish_or_total(self):
        raw, kind = build_reply(False, model="alias")
        text = raw.decode()
        assert_public("chat/completions", False, "alias", text, kind, MODEL)
        for corrupted in (text.replace(ANSWER, ANSWER + " extra"), text.replace('"stop"', '"length"'),
                          text.replace('"total_tokens": 5', '"total_tokens": 7')):
            with self.assertRaises(RuntimeError):
                assert_public("chat/completions", False, "alias", corrupted, kind, MODEL)
        reply = json.loads(text)
        reply["choices"][0]["message"]["reasoning_content"] = [{"signature": "opaque"}]
        # A provider-specific assertion validates extensions; the shared text assertion permits them.
        assert_public("chat/completions", False, "alias", json.dumps(reply), kind, MODEL)
