import assert from "node:assert/strict";
import test from "node:test";
import { compactProducerToolContent, summarizeConversationStream } from "../producer-browser/conversation-stream.mjs";

test("Producer tool summaries never retain the original object at the depth limit", () => {
  const leaf = { privateDetail: "DEEP_FIXTURE_CANARY", text: "x".repeat(5000) };
  const value = { a: { b: { c: { d: leaf } } } };
  const summary = compactProducerToolContent(value);
  assert.notEqual(summary.a.b.c.d, leaf);
  assert.ok(!JSON.stringify(summary).includes("DEEP_FIXTURE_CANARY"));
});

test("Producer tool summaries bound wide object projections", () => {
  const value = Object.fromEntries(Array.from({ length: 1000 }, (_, index) => [`field${index}`, "x".repeat(500)]));
  const summary = compactProducerToolContent(value);
  assert.ok(Object.keys(summary).length <= 33);
  assert.ok(JSON.stringify(summary).length < 16_000);
});

test("Producer tool summaries do not copy oversized property names", () => {
  const summary = compactProducerToolContent({ ["field".repeat(2000)]: "fixture" });
  assert.ok(JSON.stringify(summary).length < 500);
});

test("Producer tool summaries share one projection budget across all branches", () => {
  let value = { text: "x".repeat(300) };
  for (let depth = 0; depth < 4; depth++) {
    value = Object.fromEntries(Array.from({ length: 8 }, (_, index) => [`child${index}`, value]));
  }
  assert.ok(JSON.stringify(compactProducerToolContent(value)).length < 100_000);
});

test("Producer tool summaries remain serializable for cyclic input", () => {
  const value = {};
  value.self = value;
  assert.doesNotThrow(() => JSON.stringify(compactProducerToolContent(value)));
});

test("Producer tool summaries preserve prototype-named fields as data", () => {
  const summary = compactProducerToolContent(JSON.parse('{"__proto__":{"polluted":true}}'));
  assert.equal(Object.hasOwn(summary, "__proto__"), true);
  assert.equal(summary.polluted, undefined);
});

test("Producer tool projection retains job identity beyond the diagnostic field budget", () => {
  const content = Object.fromEntries(Array.from({ length: 40 }, (_, index) => [`detail${index}`, "fixture"]));
  content.job_id = "fixture-video-job";
  const frame = { part: { part_kind: "tool-return", tool_name: "video__create_music_video", content } };
  const summary = summarizeConversationStream(`event: part\ndata: ${JSON.stringify(frame)}\n\n`);
  assert.equal(summary.toolReturns[0].content.job_id, "fixture-video-job");
});
