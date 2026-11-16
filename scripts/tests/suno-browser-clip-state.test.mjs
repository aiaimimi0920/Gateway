import test from "node:test";
import assert from "node:assert/strict";
import { snapshot, complete } from "../suno-browser/clip-state.mjs";
const clip = (id, status = "complete") => ({ id, status, audio_url: "https://asset.invalid/audio" });
test("Suno submission validates every ID and a bounded nonempty set", () => {
  for (const clips of [[], Array.from({ length: 17 }, (_, i) => clip(String(i))),
    [clip("A"), {}], [clip("A"), clip("A")], [clip("")], [clip(" A")], [clip("界".repeat(43))]]) {
    assert.throws(() => snapshot({ clips }), { code: "suno_browser_clip_identity" });
  }
  assert.throws(() => snapshot({}), { code: "suno_browser_clip_identity" });
});
test("Suno invalid status cannot be silently dropped", () => {
  for (const status of [null, 42, "", " complete", "x".repeat(65)]) {
    assert.throws(() => snapshot({ clips: [clip("A", status)] }), { code: "suno_browser_clip_status" });
  }
});
test("Suno global feed keeps original order and ignores failed unrelated songs", () => {
  const selected = snapshot({ clips: [clip("B"), clip("foreign", "failed"), clip("A")] }, ["A", "B"]);
  assert.deepEqual(selected.map((c) => c.id), ["A", "B"]);
  assert.equal(complete(selected, ["A", "B"], "audio"), true);
});
test("Suno failed or cancelled original song rejects the entire snapshot", () => {
  for (const state of ["error", "failed", "cancelled", "canceled", "FAILED"]) {
    assert.throws(() => snapshot({ clips: [clip("A"), clip("B", state)] }, ["A", "B"]),
      { code: "suno_browser_generation_failed" });
  }
});
test("Suno unknown status, missing asset and partial sets never prove completion", () => {
  assert.equal(complete([clip("A", "future-state")], ["A"], "audio"), false);
  assert.equal(complete([{ id: "A", status: "complete" }], ["A"], "audio"), false);
  assert.equal(complete(snapshot({ clips: [clip("foreign")] }, ["A"]), ["A"], "audio"), false);
  assert.equal(complete(snapshot({ clips: [clip("A")] }, ["A", "B"]), ["A", "B"], "audio"), false);
});
test("Suno feed is bounded and duplicate foreign IDs still reject ambiguity", () => {
  assert.throws(() => snapshot({ clips: [clip("other"), clip("other")] }, ["A"]), { code: "suno_browser_clip_identity" });
  assert.throws(() => snapshot({ clips: Array.from({ length: 129 }, (_, i) => clip(String(i))) }, ["A"]),
    { code: "suno_browser_clip_identity" });
});
