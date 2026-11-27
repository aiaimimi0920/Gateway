// Strict reconstruction of this Gateway's buffered Responses event contract.
import assert from "node:assert/strict";

export function responseLifecycle(value, events) {
  const byType = (list, type) => list.filter((e) => e.type === type);
  for (const [index, item] of value.output.entries()) {
    const list = events.filter((e) => e.output_index === index);
    const added = byType(list, "response.output_item.added"), done = byType(list, "response.output_item.done");
    assert.equal(added.length, 1, "output_item_added_count");
    assert.equal(done.length, 1, "output_item_done_count");
    assert.equal(list[0], added[0], "output_item_added_order");
    assert.equal(list.at(-1), done[0], "output_item_done_order");
    assert.equal(added[0].item.id, item.id, "output_item_identity");
    assert.equal(added[0].item.type, item.type, "output_item_type");
    assert.deepEqual(done[0].item, item, "output_item_done_mismatch");
    assert.ok(list.every((e) => !e.item_id || e.item_id === item.id), "event_item_identity");
    if (["function_call", "custom_tool_call"].includes(item.type)) {
      const custom = item.type === "custom_tool_call";
      const kind = custom ? "custom_tool_call_input" : "function_call_arguments";
      const field = custom ? "input" : "arguments";
      const deltas = byType(list, `response.${kind}.delta`), finished = byType(list, `response.${kind}.done`);
      assert.equal(deltas.map((e) => e.delta).join(""), item[field], "tool_delta_mismatch");
      assert.equal(finished.length, 1, "tool_input_done_count");
      assert.equal(finished[0][field], item[field], "tool_input_done_mismatch");
      assert.ok(deltas.every((e) => e.sequence_number < finished[0].sequence_number), "tool_delta_after_done");
    } else {
      assert.ok(["message", "reasoning"].includes(item.type), "unsupported_output_item");
      for (const [contentIndex, part] of item.content.entries()) {
        const parts = list.filter((e) => e.content_index === contentIndex);
        const kind = item.type === "reasoning" ? "reasoning_text" : "output_text";
        const partAdded = byType(parts, "response.content_part.added");
        const partDone = byType(parts, "response.content_part.done");
        const textDone = byType(parts, `response.${kind}.done`);
        const deltas = byType(parts, `response.${kind}.delta`);
        assert.equal(partAdded.length, 1, "content_part_added_count");
        assert.equal(partDone.length, 1, "content_part_done_count");
        assert.equal(textDone.length, 1, "content_text_done_count");
        assert.equal(deltas.map((e) => e.delta).join(""), part.text, "content_delta_mismatch");
        assert.equal(textDone[0].text, part.text, "content_text_done_mismatch");
        assert.deepEqual(partDone[0].part, part, "content_part_done_mismatch");
        assert.equal(parts[0], partAdded[0], "content_part_added_order");
        assert.equal(parts.at(-1), partDone[0], "content_part_done_order");
        assert.ok(deltas.every((e) => e.sequence_number < textDone[0].sequence_number), "content_delta_after_done");
      }
    }
  }
}
