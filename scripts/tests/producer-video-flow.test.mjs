import assert from "node:assert/strict";
import test, { mock } from "node:test";

const previousTrace = process.env.PRODUCER_BROWSER_TRACE_PATH;
delete process.env.PRODUCER_BROWSER_TRACE_PATH;
const { executeProducerConversationVideoFlow: flow } = await import("../producer-browser/video-flow.mjs");
if (previousTrace !== undefined) process.env.PRODUCER_BROWSER_TRACE_PATH = previousTrace;

const frame = (event, data) => `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
const tool = (name, content) => frame("part", { part: { part_kind: "tool-return", tool_name: name, content } });

async function runFixture(overrides = {}) {
  const calls = [];
  const replies = [
    { ok: true, status: 200, text: '{"job_id":"bootstrap"}' },
    { ok: true, status: 200, text: frame("conversation_id", { id: "conversation" }) },
    { ok: true, status: 200, text: '{"job_id":"creative"}' },
    { ok: true, status: 200, text: tool("video__propose_music_video", {}) },
    { ok: true, status: 200, text: '{"job_id":"confirm"}' },
    { ok: true, status: 200, text: tool("video__create_music_video", { job_id: "video" }) },
  ];
  for (const [index, reply] of Object.entries(overrides.replies ?? {})) replies[Number(index)] = reply;
  const page = {
    async evaluate(_callback, args) {
      calls.push(args);
      assert.ok(replies.length, "unexpected browser request");
      return replies.shift();
    },
  };
  const statusCalls = [];
  const fetchMock = mock.method(globalThis, "fetch", async (url, options) => {
    statusCalls.push({ url, options });
    const reply = overrides.status ?? { ok: true, status: 200, text: '{"status":"pending"}' };
    return new Response(reply.text, { status: reply.status });
  });
  try {
    const result = await flow({
      page, baseUrl: "https://producer.invalid", authToken: "fixture-token",
      cookieHeader: "fixture=cookie", clipId: "clip", creativePrompt: "fixture vision",
      requestBody: {}, timeoutMs: 1000, initialReferer: "https://producer.invalid/library",
    });
    return { result, calls, statusCalls };
  } finally {
    fetchMock.mock.restore();
  }
}

test("Producer async flow keeps pending distinct from completed and preserves request sequence", async () => {
  const { result, calls, statusCalls } = await runFixture();
  assert.equal(result.ok, true);
  assert.equal(result.result.accepted, true);
  assert.equal(result.result.completed, false);
  assert.equal(result.result.state, "pending");
  assert.equal(result.result.confirmation_job_id, "confirm");
  assert.equal(calls.length, 6);
  assert.equal(JSON.parse(calls[0].options.body).client_context.current_song_id, "clip");
  assert.match(JSON.parse(calls[2].options.body).parts[0].content, /fixture vision/);
  assert.equal(JSON.parse(calls[4].options.body).conversation_id, "conversation");
  assert.equal(statusCalls[0].options.headers.cookie, "fixture=cookie");
  assert.ok(statusCalls[0].options.signal instanceof AbortSignal);
});

test("Producer completed flow selects the matching video asset", async () => {
  const { result } = await runFixture({ status: {
    ok: true, status: 200, text: JSON.stringify({ status: "completed", assets: [
      "https://assets.invalid/other.mp4", "https://assets.invalid/music-video/video/final.mp4",
    ] }),
  } });
  assert.equal(result.result.completed, true);
  assert.equal(result.result.data[0].url, "https://assets.invalid/music-video/video/final.mp4");
});

for (const [name, overrides, code] of [
  ["conversation HTTP failure", { replies: { 0: { ok: false, status: 403, text: "denied" } } }, "producer_browser_conversation_failed"],
  ["invalid conversation JSON", { replies: { 0: { ok: true, status: 200, text: "invalid" } } }, "producer_browser_invalid_conversation_response"],
  ["missing conversation job", { replies: { 0: { ok: true, status: 200, text: "{}" } } }, "producer_browser_missing_job_id"],
  ["stream failure", { replies: { 1: { ok: false, status: 502, text: "failed" } } }, "producer_browser_stream_failed"],
  ["missing conversation ID", { replies: { 1: { ok: true, status: 200, text: "" } } }, "producer_browser_missing_conversation_id"],
  ["missing video job", { replies: { 5: { ok: true, status: 200, text: "" } } }, "producer_browser_missing_video_job_id"],
  ["status HTTP failure", { status: { ok: false, status: 503, text: "failed" } }, "producer_browser_status_failed"],
  ["terminal video failure", { status: { ok: true, status: 200, text: '{"status":"failed"}' } }, "producer_browser_video_failed"],
]) {
  test(`Producer flow retains ${name} contract`, async () => {
    const { result } = await runFixture(overrides);
    assert.equal(result.ok, false);
    assert.equal(result.error.code, code);
  });
}
