"""Minimal native terminal frames for the packaged SenseNova fixture."""

import json


def event_stream(frames):
    return "".join(f"event: {frame['type']}\ndata: {json.dumps(frame)}\n\n" for frame in frames).encode()


def messages(model, answer, stream, ident):
    text = {"type": "text", "text": answer}
    start_usage = {"input_tokens": 2, "output_tokens": 0}
    msg = dict(id=f"msg_{ident}", type="message", role="assistant", model=model,
               content=[], stop_reason=None, stop_sequence=None, usage=start_usage)
    if not stream:
        return dict(msg, content=[text], stop_reason="end_turn", usage=dict(start_usage, output_tokens=3))
    return [
        dict(type="message_start", message=msg),
        dict(type="content_block_start", index=0, content_block=dict(text, text="")),
        dict(type="content_block_delta", index=0, delta=dict(type="text_delta", text=answer)),
        dict(type="content_block_stop", index=0),
        dict(type="message_delta", delta=dict(stop_reason="end_turn", stop_sequence=None), usage={"output_tokens": 3}),
        dict(type="message_stop"),
    ]


def responses(model, answer, stream, ident):
    base = dict(id=f"resp_{ident}", object="response", status="in_progress", model=model, output=[])
    part = dict(type="output_text", text=answer, annotations=[])
    item = dict(id=f"msg_{ident}", type="message", role="assistant", status="completed", content=[part])
    final = dict(base, status="completed", output=[item], usage={"input_tokens": 2, "output_tokens": 3, "total_tokens": 5})
    if not stream:
        return final
    frames = [
        dict(type="response.created", response=base),
        dict(type="response.output_item.added", output_index=0, item=dict(item, status="in_progress", content=[])),
        dict(type="response.content_part.added", item_id=item["id"], output_index=0, content_index=0, part=dict(part, text="")),
        dict(type="response.output_text.delta", item_id=item["id"], output_index=0, content_index=0, delta=answer),
        dict(type="response.output_text.done", item_id=item["id"], output_index=0, content_index=0, text=answer),
        dict(type="response.content_part.done", item_id=item["id"], output_index=0, content_index=0, part=part),
        dict(type="response.output_item.done", output_index=0, item=item),
        dict(type="response.completed", response=final),
    ]
    return [dict(frame, sequence_number=i) for i, frame in enumerate(frames)]
