import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { executeProducerPageVideoFlow } from "../producer-browser/page-video-flow.mjs";

async function runPageFixture(overrides = {}) {
  const responses = [
    '{"job_id":"bootstrap-job"}',
    'event: conversation_id\ndata: {"id":"conversation-1"}\n\nevent: final\ndata: {}\n\n',
    '{"job_id":"creative-job"}',
    'event: message\ndata: {"parts":[{"part_kind":"tool-return","tool_name":"video__create_music_video","content":{"job_id":"video-job"}}]}\n\nevent: final\ndata: {}\n\n',
    '{"status":"processing"}',
    '{"status":"completed","video_url":"https://cdn.invalid/music-video/video-job/final.mp4"}',
  ];
  if (overrides.confirm) {
    responses[3] = 'event: message\ndata: {"parts":[{"part_kind":"text","content":"ready"},{"part_kind":"suggestion","content":["Create video"]}]}\n\nevent: final\ndata: {}\n\n';
    responses.splice(4, 0, '{"job_id":"confirm-job"}',
      'event: message\ndata: {"parts":[{"part_kind":"tool-return","tool_name":"video__create_music_video","content":{"jobId":"video-job"}}]}\n\nevent: final\ndata: {}\n\n');
  }
  for (const [index, value] of Object.entries(overrides.responses ?? {})) responses[Number(index)] = value;
  const calls = [];
  const delays = [];
  let timerId = 0;
  const page = { evaluate: async (callback, args) => structuredClone(await vm.runInNewContext(`(${callback.toString()})`, {
    AbortController,
    TextDecoder,
    fetch: async (url, options) => {
      calls.push({ url, method: options.method, body: options.body });
      assert.ok(responses.length, "unexpected page request");
      const reply = responses.shift();
      return typeof reply === "string" ? new Response(reply) : new Response(reply.text, { status: reply.status });
    },
    setTimeout(fn, ms) {
      delays.push(ms);
      if (ms === 5000) queueMicrotask(fn);
      return ++timerId;
    },
    clearTimeout() {},
  })(structuredClone(args))) };
  const result = await executeProducerPageVideoFlow(page, {
    authToken: "fixture-token", baseUrl: "https://producer.invalid",
    origin: "https://producer.invalid", referer: "https://producer.invalid/library/videos",
    requestBody: overrides.requestBody ?? { clip_id: "clip-1", prompt: "fixture video" },
    model: "producer:music-video", timeoutMs: 30000,
  });
  return { result, calls, delays, responses };
}

test("Producer entry passes the page to the imported phase orchestrator", async () => {
  const source = await readFile(new URL("../producer-browser-worker.mjs", import.meta.url), "utf8");
  assert.ok(source.includes('from "./producer-browser/page-video-flow.mjs"'));
  assert.match(source, /executeProducerPageVideoFlow\(\s*page,/);
});

test("Producer serialized page callback can poll pending video without outer lexical scope", async () => {
  const { result, calls, delays, responses } = await runPageFixture();
  assert.equal(result.ok, true);
  assert.equal(result.result.completed, true);
  assert.equal(result.result.data[0].url, "https://cdn.invalid/music-video/video-job/final.mp4");
  assert.equal(calls.length, 6);
  assert.equal(responses.length, 0);
  assert.ok(delays.includes(5000));
});

for (const confirm of [false, true]) {
  test(`Producer page accepts streamed part video job after ${confirm ? "confirmation" : "creation"}`, async () => {
    const stream = 'event: part\ndata: {"part":{"part_kind":"tool-return","tool_name":"video__create_music_video","content":{"jobId":"video-job"}}}\n\nevent: final\ndata: {}\n\n';
    const { result, calls, responses } = await runPageFixture({
      confirm, responses: { [confirm ? 5 : 3]: stream },
    });
    assert.equal(result.ok, true, JSON.stringify(result.error));
    assert.equal(result.result.completed, true);
    assert.equal(result.result.data[0].url, "https://cdn.invalid/music-video/video-job/final.mp4");
    assert.ok(calls.at(-1).url.includes("video-job"));
    assert.equal(responses.length, 0);
  });
}

test("Producer page confirmation retains nested aliases and stream material", async () => {
  const { result, calls } = await runPageFixture({ confirm: true, requestBody: {
    args: { songId: "clip-1", prompt: "nested vision", confirmPrompt: "confirm fixture", duration: "20" },
  } });
  assert.equal(result.ok, true);
  assert.equal(result.result.confirmation_job_id, "confirm-job");
  assert.equal(result.result.data[0].duration_seconds, 20);
  assert.equal(JSON.parse(calls[4].body).parts[0].content, "confirm fixture");
  assert.match(JSON.parse(calls[2].body).parts[0].content, /nested vision/);
  assert.deepEqual(Array.from(result.result.creative_stream.suggestions), ["Create video"]);
  assert.deepEqual(Array.from(result.result.creative_stream.message_texts), ["ready"]);
});

for (const [name, overrides, code] of [
  ["missing clip", { requestBody: {} }, "missing_video_clip_id"],
  ["conversation HTTP error", { responses: { 0: { status: 403, text: "denied" } } }, "producer_browser_conversation_failed"],
  ["missing job", { responses: { 0: "{}" } }, "producer_browser_missing_job_id"],
  ["stream HTTP error", { responses: { 1: { status: 502, text: "failed" } } }, "producer_browser_stream_failed"],
  ["stream error event", { responses: { 1: 'event: error\ndata: {"message":"fixture error"}\n\n' } }, "producer_browser_stream_error_event"],
  ["missing conversation ID", { responses: { 1: "" } }, "producer_browser_missing_conversation_id"],
  ["missing video job", { confirm: true, responses: { 5: "" } }, "producer_browser_missing_video_job_id"],
  ["status HTTP error", { responses: { 4: { status: 503, text: "failed" } } }, "producer_browser_status_failed"],
  ["terminal cancellation", { responses: { 4: '{"status":"cancelled"}' } }, "producer_browser_video_failed"],
  ["completed without asset", { responses: { 4: '{"status":"completed"}' } }, "producer_browser_missing_video_url"],
]) {
  test(`Producer page preserves ${name} boundary`, async () => {
    const { result } = await runPageFixture(overrides);
    assert.equal(result.ok, false);
    assert.equal(result.error.code, code);
  });
}
