"""Strict loopback wire and public assertions for two caller-owned SAP tool turns."""
import json

from server_process import require

USER = "literal {{?customer_value}}"
RESULT = "结果 {{?model}}"
ANSWER = "已收到结果"
ARGS = '{"city":"上海 &quot;"}'
NAME = "lookup"
TOOLS = [{"type": "function", "function": {"name": NAME, "strict": True,
          "parameters": {"type": "object", "properties": {"city": {"type": "string"}},
                         "required": ["city"], "additionalProperties": False}}}]
MODES = ("orchestration-tools", "orchestration-image-tools", "bedrock-tools", "bedrock-image-tools")


def call_id(mode):
    return "tooluse.sap:1" if mode.startswith("bedrock") else "call_sap"


def call(mode, arguments=ARGS):
    return {"id": call_id(mode), "type": "function", "function": {"name": NAME, "arguments": arguments}}


def public_request(body, mode, previous, native):
    body.pop("stream_options", None)
    content = USER
    if "-image-" in mode:
        content = [{"type": "text", "text": USER},
                   {"type": "image_url", "image_url": {"url": native.IMAGE_URL}}]
    body.update(messages=[{"role": "user", "content": content}], tools=TOOLS)
    if mode.startswith("bedrock"):
        body["tool_choice"] = "auto"
    if previous is not None:
        body["messages"] += [{"role": "assistant", "content": None, "tool_calls": [previous]},
                             {"role": "tool", "tool_call_id": previous["id"], "content": RESULT}]
    return body


def orchestration_body(mode, stream, final, native):
    history = public_request({}, mode, call(mode) if final else None, native)["messages"]
    template = history.pop()
    for message in history:
        if message["role"] == "assistant":
            del message["content"]
    if isinstance(template["content"], list):
        values = {"gateway_part_0": USER, "gateway_part_1": native.IMAGE_URL}
        template["content"] = [{"type": "text", "text": "{{?gateway_part_0}}"},
                               {"type": "image_url", "image_url": {"url": "{{?gateway_part_1}}"}}]
    else:
        values = {"gateway_content": template["content"]}
        template["content"] = "{{?gateway_content}}"
    config = {"modules": {"prompt_templating": {"model": {"name": native.ORCHESTRATION_MODEL},
                                               "prompt": {"template": [template], "tools": TOOLS}}}}
    if stream:
        config["stream"] = {"enabled": True}
    return {"config": config, "messages_history": history, "placeholder_values": values}


def native_call(mode):
    return {"toolUse": {"toolUseId": call_id(mode), "name": NAME, "input": json.loads(ARGS)}}


def bedrock_body(mode, final, native):
    content = [{"text": USER}]
    if "-image-" in mode:
        content.append({"image": {"format": "png", "source": {"bytes": native.PNG}}})
    messages = [{"role": "user", "content": content}]
    if final:
        messages += [{"role": "assistant", "content": [native_call(mode)]},
                     {"role": "user", "content": [{"toolResult": {
                         "toolUseId": call_id(mode), "content": [{"text": RESULT}]}}]}]
    tool = TOOLS[0]["function"]
    return {"messages": messages, "toolConfig": {"toolChoice": {"auto": {}}, "tools": [
        {"toolSpec": {"name": NAME, "strict": True, "inputSchema": {"json": tool["parameters"]}}}]}}


def orchestration_reply(mode, model, stream, final, native):
    base = {"id": "orch-fixture-tool", "created": 123, "model": model,
            "object": "chat.completion.chunk" if stream else "chat.completion"}
    finish = "stop" if final else "tool_calls"

    def envelope(choice, usage=None):
        return {"request_id": base["id"], "intermediate_results": {"llm": {"private": native.PRIVATE}},
                "final_result": dict(base, choices=[dict(choice, index=0)], usage=usage)}

    if not stream:
        message = {"role": "assistant", **({"content": ANSWER} if final else {"tool_calls": [call(mode)]})}
        return native.dump(envelope({"message": message, "finish_reason": finish}, native.USAGE)), native.JSON
    mid = ARGS.index(":")
    first = dict(call(mode, ARGS[:mid]), index=0)
    deltas = ([{"role": "assistant", "content": ANSWER}] if final else
              [{"role": "assistant", "tool_calls": [first]},
               {"tool_calls": [{"index": 0, "function": {"arguments": ARGS[mid:]}}]}])
    frames = [envelope({"delta": delta, "finish_reason": None}) for delta in deltas]
    frames.append(envelope({"delta": {}, "finish_reason": finish}, native.USAGE))
    return b"".join(b"data: " + native.dump(frame) + b"\n\n" for frame in frames) + b"data: [DONE]\n\n", native.SSE


def bedrock_reply(mode, model, stream, final, native):
    usage = {"inputTokens": 2, "outputTokens": 3, "totalTokens": 5}
    reason = "end_turn" if final else "tool_use"
    if not stream:
        content = [{"text": ANSWER}] if final else [native_call(mode)]
        return native.dump(dict(model=model, output={"message": {"role": "assistant", "content": content}},
                                stopReason=reason, usage=usage)), native.JSON
    events = [("messageStart", {"role": "assistant"})]
    if final:
        events.append(("contentBlockDelta", {"contentBlockIndex": 0, "delta": {"text": ANSWER}}))
    else:
        events.append(("contentBlockStart", {"contentBlockIndex": 0, "start": {
            "toolUse": {"toolUseId": call_id(mode), "name": NAME}}}))
        mid = ARGS.index(":")
        events.extend(("contentBlockDelta", {"contentBlockIndex": 0, "delta": {"toolUse": {"input": fragment}}})
                      for fragment in (ARGS[:mid], ARGS[mid:]))
    events.extend([("contentBlockStop", {"contentBlockIndex": 0}), ("messageStop", {"stopReason": reason}),
                   ("metadata", {"usage": usage, "metrics": {"privateExtension": native.PRIVATE}})])
    return b"".join(native.bedrock_event(kind, value) for kind, value in events), native.EVENTSTREAM


def wire(mode, accept, request, model, stream, native):
    bedrock = mode.startswith("bedrock")
    if accept != ((native.EVENTSTREAM if bedrock else native.SSE) if stream else native.JSON):
        return None
    body, reply = (bedrock_body, bedrock_reply) if bedrock else (None, orchestration_reply)
    for final in (False, True):
        expected = body(mode, final, native) if bedrock else orchestration_body(mode, stream, final, native)
        if request == expected:
            return stream, *reply(mode, model, stream, final, native)
    return None


def assert_public(mode, alias, text, kind, stream, final):
    finish = "stop" if final else "tool_calls"
    if stream:
        lines = [line for line in text.splitlines() if line]
        require(kind == "text/event-stream" and lines and lines[-1] == "data: [DONE]"
                and lines.count("data: [DONE]") == 1, "Tool stream did not terminate exactly once")
        frames = [json.loads(line[6:]) for line in lines[:-1]]
        require(frames and all(frame.get("model") == alias and "error" not in frame for frame in frames),
                "Wrong alias or error in tool stream")
        usages = [frame["usage"] for frame in frames if frame.get("usage") is not None]
        choices = [choice for frame in frames for choice in frame.get("choices", [])]
        require(all(choice["index"] == 0 for choice in choices)
                and [choice["finish_reason"] for choice in choices if choice.get("finish_reason") is not None] == [finish],
                "Tool stream had wrong finish reason or choice index")
        deltas = [choice.get("delta", {}) for choice in choices]
        chunks = [item for delta in deltas for item in delta.get("tool_calls", [])]
        content = "".join(delta.get("content") or "" for delta in deltas)
        require(all(item["index"] == 0 for item in chunks), "Unexpected tool index")
        arguments = "".join(item.get("function", {}).get("arguments", "") for item in chunks)
        calls = ([{"id": chunks[0]["id"], "type": chunks[0]["type"],
                   "function": {"name": chunks[0]["function"]["name"], "arguments": arguments}}] if chunks else [])
        require(sum("id" in item for item in chunks) == len(calls), "Repeated tool identity")
    else:
        reply = json.loads(text)
        require(kind == "application/json" and reply["model"] == alias, "Wrong tool JSON model or content type")
        choice, = reply["choices"]
        require(choice["index"] == 0 and choice["finish_reason"] == finish
                and choice["message"]["role"] == "assistant", "Wrong tool JSON terminal choice")
        calls = choice["message"].get("tool_calls") or []
        content = choice["message"].get("content") or ""
        usages = [reply["usage"]]
    require(len(usages) == 1 and all(usages[0].get(k) == v for k, v in
            (("prompt_tokens", 2), ("completion_tokens", 3), ("total_tokens", 5))), "Wrong tool turn usage")
    require(calls == ([] if final else [call(mode)]) and content == (ANSWER if final else ""),
            "Tool identity, argument bytes, or final answer changed")
    return calls[0] if calls else None
