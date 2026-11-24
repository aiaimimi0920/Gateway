"""Native SAP wire contracts for fixed loopback fixture deployments."""
import json
import struct
import sys
import uuid
import zlib

from native_replies import event_stream, responses
import sap_tools
from sensenova import ANSWER, build_reply

DEPLOY_ROOT = "/v2/inference/deployments"
NATIVE_RESPONSES = f"{DEPLOY_ROOT}/fixture-responses"
RESPONSES_IMAGE = f"{DEPLOY_ROOT}/fixture-responses-image"
CHAT_IMAGE = f"{DEPLOY_ROOT}/fixture-chat-image"
CHAT_REASONING = f"{DEPLOY_ROOT}/fixture-chat-reasoning"
O3_VERSION = "2024-12-01-preview"
ORCHESTRATION = f"{DEPLOY_ROOT}/fixture-orchestration"
ORCHESTRATION_IMAGE = f"{DEPLOY_ROOT}/fixture-orchestration-image"
ORCHESTRATION_REASONING = f"{DEPLOY_ROOT}/fixture-orchestration-reasoning"
BEDROCK = f"{DEPLOY_ROOT}/fixture-bedrock"
ORCHESTRATION_MODEL = "gpt-4.1-mini"
REASONING_MODEL = "gpt-5"
REASONING = [{"content": "visible summary"}, {"signature": "opaque+/=signature"}]
PROMPT = "packaged fixture"
PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
IMAGE_URL = f"data:image/png;base64,{PNG}"
CHAT_IMAGE_INPUT = [{"type": "text", "text": PROMPT},
                    {"type": "image_url", "image_url": {"url": IMAGE_URL, "detail": "high"}}]
RESPONSES_IMAGE_INPUT = [{"role": "user", "content": [{"type": "input_text", "text": PROMPT},
                         {"type": "input_image", "image_url": IMAGE_URL, "detail": "high"}]}]
JSON = "application/json"
SSE = "text/event-stream"
EVENTSTREAM = "application/vnd.amazon.eventstream"
PRIVATE = "sap-fixture-private-intermediate"
USAGE = {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5}


def dump(value):
    return json.dumps(value, ensure_ascii=False).encode("utf-8")


def orchestration_reply(model, stream, reasoning=False):
    base = dict(id="orch-" + uuid.uuid4().hex, created=123, model=model)
    message = dict(role="assistant", content=ANSWER)
    if reasoning and not stream:
        message["reasoning_content"] = REASONING

    def envelope(final):
        return dict(request_id=base["id"], final_result=final,
                    intermediate_results={"llm": {"choices": [{"delta": {"content": PRIVATE}}],
                                                 "usage": {"total_tokens": 99}}})

    if not stream:
        choice = dict(index=0, message=message, finish_reason="stop")
        return dump(envelope(dict(base, object="chat.completion", choices=[choice], usage=USAGE))), JSON
    chunk = dict(base, object="chat.completion.chunk")
    frames = [envelope(dict(chunk, choices=[dict(index=0, delta={"reasoning_content": [block]},
                                                finish_reason="")])) for block in REASONING] if reasoning else []
    frames += [envelope(dict(chunk, choices=[dict(index=0, delta=message, finish_reason="")])),
              envelope(dict(chunk, choices=[dict(index=0, delta={}, finish_reason="stop")], usage=USAGE))]
    return b"".join(b"data: " + dump(frame) + b"\n\n" for frame in frames) + b"data: [DONE]\n\n", SSE


def orchestration_wire(path, accept, request, model):
    if path not in (ORCHESTRATION, ORCHESTRATION_IMAGE, ORCHESTRATION_REASONING):
        return None
    stream, reasoning = accept == SSE, path == ORCHESTRATION_REASONING
    fixed_model = {"name": REASONING_MODEL if reasoning else ORCHESTRATION_MODEL}
    if reasoning:
        fixed_model["params"] = {"reasoning_effort": "xhigh" if stream else "high"}
    config = {"modules": {"prompt_templating": {"model": fixed_model}}}
    if stream:
        config["stream"] = {"enabled": True}
    content = CHAT_IMAGE_INPUT if path == ORCHESTRATION_IMAGE else PROMPT
    history = [{"role": "user", "content": content}]
    if reasoning and stream:
        history += [{"role": "assistant", "content": ANSWER, "reasoning_content": REASONING},
                    {"role": "user", "content": "continue"}]
    # Exact envelope rejects leaked Gateway capability flags and misplaced stream options.
    expected = {"config": config, "messages_history": history}
    if accept not in (JSON, SSE) or request != expected:
        return None
    return (stream, *orchestration_reply(model, stream, reasoning))


def bedrock_event(kind, payload):
    headers = b""
    for name, value in ((b":message-type", b"event"), (b":event-type", kind.encode("ascii")),
                        (b":content-type", JSON.encode("ascii"))):
        headers += struct.pack("!B", len(name)) + name + struct.pack("!BH", 7, len(value)) + value
    data = dump(payload)
    prelude = struct.pack("!II", 16 + len(headers) + len(data), len(headers))
    message = prelude + struct.pack("!I", zlib.crc32(prelude)) + headers + data
    return message + struct.pack("!I", zlib.crc32(message))


def bedrock_reply(model, stream):
    usage = {"inputTokens": 2, "outputTokens": 3, "totalTokens": 5}
    if not stream:
        return dump(dict(model=model, output={"message": {"role": "assistant", "content": [{"text": ANSWER}]}},
                         stopReason="end_turn", usage=usage)), JSON
    events = [("messageStart", {"role": "assistant"}),
              ("contentBlockDelta", {"contentBlockIndex": 0, "delta": {"text": ANSWER}}),
              ("contentBlockStop", {"contentBlockIndex": 0}),
              ("messageStop", {"stopReason": "end_turn"}),
              ("metadata", {"usage": usage, "metrics": {"privateExtension": PRIVATE}})]
    return b"".join(bedrock_event(kind, body) for kind, body in events), EVENTSTREAM


def specialized_chat(path, accept, request, model):
    stream = request.get("stream") is True
    if accept != (SSE if stream else JSON):
        return None
    if path.path == f"{CHAT_IMAGE}/chat/completions":
        # Messages does not have a detail field; the other two public protocols do.
        for text, detail in ((PROMPT, {"detail": "high"}), (PROMPT + " via Messages", {})):
            content = [{"type": "text", "text": text},
                       {"type": "image_url", "image_url": {"url": IMAGE_URL, **detail}}]
            if request.get("messages") == [{"role": "user", "content": content}]:
                break
        else:
            return None
        if path.query != "api-version=2024-10-21" or "sap_image_input" in request:
            return None
    elif path.path == f"{CHAT_REASONING}/chat/completions":
        if (path.query != f"api-version={O3_VERSION}" or request.get("messages") != [{"role": "user", "content": PROMPT}]
                or request.get("reasoning_effort") != ("high" if stream else "low")
                or request.get("max_completion_tokens") != 32
                or any(k in request for k in ("sap_o3_mini", "sap_api_version", "max_tokens"))):
            return None
    else:
        return None
    return (stream, *build_reply(stream, model=model))


def reply(path, accept, request, model):
    """Return (stream, payload, content type) only for the exact fixture wire."""
    if "model" in request:
        return None
    if path.query:
        return specialized_chat(path, accept, request, model)
    deployment, _, action = path.path.removeprefix(DEPLOY_ROOT + "/fixture-").partition("/")
    if deployment in sap_tools.MODES and path.path.startswith(DEPLOY_ROOT + "/fixture-"):
        if deployment.startswith("bedrock") and action in ("converse", "converse-stream"):
            stream = action == "converse-stream"
        elif deployment.startswith("orchestration") and action == "v2/completion":
            stream = accept == SSE
        else:
            return None
        return sap_tools.wire(deployment, accept, request, model, stream, sys.modules[__name__])
    if path.path in (f"{NATIVE_RESPONSES}/responses", f"{RESPONSES_IMAGE}/responses"):
        image = path.path == f"{RESPONSES_IMAGE}/responses"
        expected_input = RESPONSES_IMAGE_INPUT if image else PROMPT
        reasoning = None if image else {"effort": "low"}
        stream = request.get("stream") is True
        if (accept != (SSE if stream else JSON) or request.get("input") != expected_input
                or request.get("store") is not False or request.get("reasoning") != reasoning
                or "sap_native_responses_image_input" in request):
            return None
        result = responses(model, ANSWER, stream, uuid.uuid4().hex)
        return stream, event_stream(result) if stream else dump(result), SSE if stream else JSON
    if path.path.endswith("/v2/completion"):
        return orchestration_wire(path.path.removesuffix("/v2/completion"), accept, request, model)
    if path.path in (f"{BEDROCK}/converse", f"{BEDROCK}/converse-stream"):
        stream = path.path.endswith("-stream")
        content = [{"text": "inspect"}, {"image": {"format": "png", "source": {"bytes": PNG}}}]
        if (accept != (EVENTSTREAM if stream else JSON)
                or request.get("messages") != [{"role": "user", "content": content}]
                or "sap_bedrock_image_input" in request):
            return None
        return (stream, *bedrock_reply(model, stream))
    return None
