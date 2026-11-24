"""Strict SenseNova image and response-dependent tool/reasoning fixture contract."""
from copy import deepcopy
import json
import uuid

from hosted_product import HostedProduct
from sap_native import CHAT_IMAGE_INPUT
from sensenova import ANSWER, MODEL, PATH, USAGE, Fixture, build_reply
from server_process import require

PROMPT = "packaged fixture"
REASONING = "fixture reasoning\n  exact whitespace"
TOOL_ID = "fixture_tool_call"
TOOLS = [{"type": "function", "function": {"name": "lookup", "parameters": {"type": "object"}}}]
ASSISTANT = {"role": "assistant", "content": None, "reasoning_content": REASONING,
             "tool_calls": [{"id": TOOL_ID, "type": "function",
                             "function": {"name": "lookup", "arguments": "{}"}}]}
TOOL_RESULT = {"role": "tool", "tool_call_id": TOOL_ID, "content": "lookup result"}


def image_request(alias, stream):
    request = {"model": alias, "stream": stream,
               "messages": [{"role": "user", "content": deepcopy(CHAT_IMAGE_INPUT)}]}
    if stream:
        request["stream_options"] = {"include_usage": True}
    return request


def reasoning_request(alias, assistant=None):
    body = {"model": alias, "stream": assistant is not None, "reasoning_effort": "high",
            "tools": deepcopy(TOOLS), "messages": [{"role": "user", "content": PROMPT}]}
    if assistant is not None:
        body["messages"] += [deepcopy(assistant), deepcopy(TOOL_RESULT)]
        body["stream_options"] = {"include_usage": True}
    return body


def first_reply():
    return {"id": "chatcmpl-fixture-" + uuid.uuid4().hex, "created": 1700000000,
            "object": "chat.completion", "model": MODEL,
            "choices": [{"index": 0, "message": deepcopy(ASSISTANT), "finish_reason": "tool_calls"}],
            "usage": dict(USAGE, completion_tokens_details={"reasoning_tokens": 2})}


def check_first_reply(alias, text, kind):
    reply = json.loads(text)
    require(kind == "application/json" and reply.get("model") == alias and MODEL not in text,
            "SenseNova tool reply exposed the private model or used the wrong format")
    require(reply.get("choices") == [{"index": 0, "message": ASSISTANT, "finish_reason": "tool_calls"}],
            "SenseNova changed reasoning whitespace, null content, tool identity or arguments")
    require(reply.get("usage") == dict(USAGE, completion_tokens_details={"reasoning_tokens": 2}),
            "SenseNova changed or double-counted reasoning usage")
    return reply["choices"][0]["message"]


class MediaPeer(Fixture):
    post_paths = (PATH,)

    def reply(self, request, stream, path):
        if path != PATH:
            return None
        if request == image_request(MODEL, stream):
            return build_reply(stream)
        if request == reasoning_request(MODEL):
            return json.dumps(first_reply()).encode("utf-8"), "application/json"
        # The canonical Chat packer normalizes empty assistant content, not reasoning.
        expected = reasoning_request(MODEL, dict(ASSISTANT, content=""))
        if request == expected:
            return build_reply(True)
        return None


class MediaProduct(HostedProduct):
    @property
    def endpoint_kinds(self):
        return {"chat/completions": "chat_completions"}

    def public_cases(self):
        return (("chat/completions", stream) for stream in (False, True, False, True))


PRODUCT = MediaProduct("SenseNova", "SENSENOVA", "packaged-sensenova-media-", "sensenova_token_plan",
                       "sensenova_token_plan_key", "/v1", MediaPeer)


def invalid_requests(alias):
    image = image_request(alias, False)
    image["messages"][0]["content"][1]["image_url"]["url"] = "file:///fixture.png"
    wrong_role = image_request(alias, False)
    wrong_role["messages"][0]["role"] = "assistant"
    reasoning = reasoning_request(alias)
    reasoning["messages"][0]["reasoning_content"] = REASONING
    invalid_type = reasoning_request(alias, ASSISTANT)
    invalid_type["messages"][1]["reasoning_content"] = {"content": REASONING}
    return ((image, "sensenova_chat_image_unsupported"), (wrong_role, "sensenova_chat_image_unsupported"),
            (reasoning, "sensenova_chat_reasoning_history_invalid"),
            (invalid_type, "sensenova_chat_reasoning_history_invalid"))
