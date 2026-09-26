import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import test from "node:test";
import { executeProducerPageVideoFlow } from "../producer-browser/page-video-flow.mjs";

const input = {
  authToken: "fixture", baseUrl: "https://producer.invalid",
  origin: "https://producer.invalid", referer: "https://producer.invalid/library/videos",
  requestBody: { clip_id: "fixture-clip" }, model: "producer:music-video", timeoutMs: 100,
};
function phaseResults() {
  const stream = { conversationId: "fixture-conversation", toolReturns: [], suggestions: [], messageTexts: [] };
  return [
    { ok: true, jobId: "bootstrap", stream },
    { ok: true, jobId: "creative", stream },
    { ok: true, jobId: "confirm", stream: { ...stream,
      toolReturns: [{ toolName: "video__create_music_video", content: { job_id: "fixture-video" } }],
    } },
    { ok: true, result: { videoUrl: "https://cdn.invalid/final.mp4", statusPayload: { status: "completed" } } },
  ];
}

for (const [index, name] of ["bootstrap", "creative", "confirmation", "status"].entries()) {
  test(`Producer page flow rejects ${name} results after the total deadline`, async (t) => {
    let now = 1000;
    t.mock.method(performance, "now", () => now);
    const replies = phaseResults();
    let calls = 0;
    const result = await executeProducerPageVideoFlow({ evaluate: async () => {
      const current = calls++;
      if (current === index) now += input.timeoutMs;
      return replies[current];
    } }, input);
    assert.equal(result.ok, false);
    assert.equal(result.error.status, 504);
    assert.equal(result.error.code, "producer_browser_video_timeout");
    assert.equal(calls, index + 1, "no subsequent phase is submitted after the deadline");
  });
}

test("Producer page phases receive only the remaining monotonic budget", async (t) => {
  let now = 0;
  t.mock.method(performance, "now", () => now);
  const replies = phaseResults();
  const budgets = [];
  const result = await executeProducerPageVideoFlow({ evaluate: async (_callback, args) => {
    budgets.push(args.timeoutMs);
    now += 20;
    return replies.shift();
  } }, input);
  assert.equal(result.ok, true);
  assert.deepEqual(budgets, [100, 80, 60, 40]);
});
