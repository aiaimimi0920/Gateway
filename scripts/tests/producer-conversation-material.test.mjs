import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import * as stream from "../producer-browser/conversation-stream.mjs";
import * as prompts from "../producer-browser/video-prompts.mjs";
import { normalizeString, readNumberFields, readStringFields } from "../producer-browser/request-fields.mjs";

const api = { ...stream, ...prompts };
const plain = (value) => JSON.parse(JSON.stringify(value));

test("Producer request fields preserve trimmed aliases and finite numeric coercion", () => {
  for (const value of [null, undefined, 1, {}, "", "  "]) {
    assert.equal(normalizeString(value), null);
  }
  assert.equal(normalizeString(" fixture "), "fixture");
  assert.equal(readStringFields(null, ["a"]), null);
  assert.equal(readStringFields({ a: " ", b: " second " }, ["a", "b"]), "second");
  for (const value of [null, "4", { a: NaN }, { a: Infinity }, { a: "Infinity" }, { a: " " }, { a: true }]) {
    assert.equal(readNumberFields(value, ["a"]), null);
  }
  assert.equal(readNumberFields({ a: "bad", b: "0" }, ["a", "b"]), 0);
  assert.equal(readNumberFields({ a: -2, b: 3 }, ["a", "b"]), -2);
  assert.equal(readNumberFields({ a: " 1.5 " }, ["a"]), 1.5);
});

test("Producer worker wires pure module owners without executable test imports", async () => {
  const source = await readFile(new URL("../producer-browser-worker.mjs", import.meta.url), "utf8");
  const flow = await readFile(new URL("../producer-browser/video-flow.mjs", import.meta.url), "utf8");
  assert.ok(source.includes('from "./producer-browser/video-flow.mjs"'));
  assert.ok(source.includes("executeProducerConversationVideoFlow("));
  for (const name of ["request-fields", "conversation-stream", "video-prompts"]) {
    assert.ok(flow.includes(`from "./${name}.mjs"`));
  }
  for (const name of ["summarizeConversationStream", "extractConversationIdFromStream", "buildProducerProposalPrompt", "chooseProducerConfirmPrompt"]) {
    assert.ok(flow.includes(`${name}(`));
    assert.ok(!flow.includes(`function ${name}(`));
  }
});

test("Producer SSE commits trailing frames and extracts conversation IDs", () => {
  const stream = 'event: conversation_id\r\ndata: {"id":" fixture-id "}\r\n\r\nevent: text\ndata: first\ndata: second';
  assert.deepEqual(plain(api.parseSseFrames(stream)), [
    { event: "conversation_id", dataText: '{"id":" fixture-id "}' },
    { event: "text", dataText: "first\nsecond" },
  ]);
  assert.equal(api.extractConversationIdFromStream(stream), "fixture-id");
  assert.equal(api.parseMaybeJson(" plain text "), "plain text");
  assert.equal(api.parseMaybeJson(" "), null);
});

test("Producer conversation summaries classify tool parts, retry prompts and suggestions", () => {
  const parts = [
    ["part", { part: { part_kind: "tool-call", tool_name: "propose", args: { duration: 30 } } }],
    ["part", { part: { part_kind: "retry-prompt", content: " retry " } }],
    ["part", { part: { part_kind: "text", content: " hello " } }],
    ["suggestion", { parts: [{ part_kind: "tool-call", tool_name: "synthetic__suggest_actions", args: { action: "Create video" } }] }],
  ];
  const summary = plain(api.summarizeConversationStream(parts.map(([event, data]) => `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`).join("")));
  assert.deepEqual(summary.toolCalls, [{ toolName: "propose", args: { duration: 30 } }]);
  assert.deepEqual(summary.retryPrompts, ["retry"]);
  assert.deepEqual(summary.messageTexts, ["hello"]);
  assert.deepEqual(summary.suggestions, ["Create video"]);
});

test("Producer media extraction preserves nested order and supported video extensions", () => {
  assert.deepEqual(plain(api.collectMediaUrls({ a: ["https://example.test/a.mp4?x=1", "https://example.test/b.png"], b: { url: "http://example.test/c.mov" } })), ["https://example.test/a.mp4?x=1", "http://example.test/c.mov"]);
});

test("Producer tool summaries retain existing lyric and timestamp projection", () => {
  const value = plain(api.compactProducerToolContent({ lyrics_text: "x".repeat(121), char_timestamps: [1, 2, 3, 4], items: [1, 2, 3, 4, 5, 6] }));
  assert.deepEqual(value.lyrics_text, { preview: "x".repeat(120), charLength: 121, truncated: true });
  assert.deepEqual(value.char_timestamps, { count: 4, sample: [1, 2, 3] });
  assert.deepEqual(value.items, [1, 2, 3, 4, 5, { truncatedCount: 1 }]);
});

test("Producer proposal honors aliases and confirmation preserves proposed section", () => {
  const proposal = api.buildProducerProposalPrompt({ aspectRatio: "9:16", durationSeconds: "20", renderLyrics: true }, "fixture vision");
  assert.match(proposal, /Vision: fixture vision/);
  assert.match(proposal, /Use 9:16/);
  assert.match(proposal, /Lyrics on screen: yes/);
  assert.match(proposal, /about 20 seconds/);
  assert.equal(api.chooseProducerConfirmPrompt({ confirmPrompt: " explicit " }, [], []), "explicit");
  const confirm = api.chooseProducerConfirmPrompt({}, [], [], { start_s: 0, duration_s: 20, aspect_ratio: "9:16" });
  assert.match(confirm, /start time at 0s/);
  assert.match(confirm, /duration at 20s/);
  assert.equal(api.chooseProducerConfirmPrompt({}, ["Try again", "Create the video"], []), "Create the video");
});
