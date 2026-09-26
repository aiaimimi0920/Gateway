import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const app = await importTestableProgramHandle();
const merge = app.mergeInvokeContract;
const laneFields = ["transportKind", "wsUrl", "apiStyle", "requestPath", "requestEnvelopeKind", "requestUrl", "requestBody", "requestRpcId", "responseRpcId", "sourcePath", "modelHint"];

test("program handle merge null input preserves identity and initialization is shallow", () => {
  const nested = Object.freeze({ marker: 1 }), source = Object.freeze({ operation: "music", requestBody: nested, cookieHeader: "synthetic" });
  assert.equal(merge(source, null), source); assert.equal(merge(null, null), null);
  const initialized = merge(null, source);
  assert.deepEqual(initialized, source); assert.notEqual(initialized, source); assert.equal(initialized.requestBody, nested);
});

const rankedLanes = [
  { transportKind: "program_music_streamgenerate_candidate" },
  { requestEnvelopeKind: "page_stream_generate_form" },
  { transportKind: "program_music_batchexecute_candidate" },
  { transportKind: "canvas_program_ws_candidate" },
  { requestEnvelopeKind: "canvas_proxy_request" },
  { transportKind: "unknown" },
];
for (let i = 0; i < rankedLanes.length - 1; i += 1) {
  test("program handle merge lane rank " + i + " wins in both arrival orders", () => {
    const higher = Object.freeze({ operation: "music", ...rankedLanes[i], requestUrl: "higher" });
    const lower = Object.freeze({ operation: "music", ...rankedLanes[i + 1], requestUrl: "lower" });
    assert.equal(merge(higher, lower).requestUrl, "higher");
    assert.equal(merge(lower, higher).requestUrl, "higher");
  });
}

test("program handle merge selected lane retains all empty and zero fields", () => {
  const primary = Object.fromEntries(laneFields.map((name) => [name, ""]));
  Object.assign(primary, { transportKind: " program_video_streamgenerate_candidate ", requestBody: 0, operation: "video" });
  const secondary = Object.fromEntries(laneFields.map((name) => [name, "fallback-" + name]));
  Object.assign(secondary, { transportKind: "canvas_program_ws_candidate", operation: "music" });
  const result = merge(Object.freeze(primary), Object.freeze(secondary));
  for (const name of laneFields) assert.equal(result[name], primary[name], name);
  assert.equal(result.operation, "video");
  const nulls = Object.fromEntries(laneFields.filter((name) => !["transportKind", "requestEnvelopeKind"].includes(name)).map((name) => [name, null]));
  const fallback = merge({ ...primary, ...nulls }, secondary);
  for (const name of Object.keys(nulls)) assert.equal(fallback[name], secondary[name], name);
});

test("program handle merge target quality improves independently of stronger request lane", () => {
  const target = Object.freeze({ operation: "music", transportKind: "program_music_streamgenerate_candidate", requestUrl: "stronger-lane", target: "blob:old", targetSource: "media_node_current_src", targetMimeType: "audio/wav" });
  const incoming = Object.freeze({ operation: "music", transportKind: "canvas_program_ws_candidate", requestUrl: "weaker-lane", target: "https://fixture.test/new.wav", targetSource: "network_audio_response", targetMimeType: "audio/wav" });
  const result = merge(target, incoming);
  assert.equal(result.requestUrl, "stronger-lane"); assert.equal(result.target, incoming.target);
  assert.equal(result.targetSource, incoming.targetSource); assert.equal(result.targetMimeType, incoming.targetMimeType);
});

test("program handle merge target and lane ties retain first metadata until target improves", () => {
  const first = Object.freeze({ operation: "music", transportKind: "canvas_program_ws_candidate", requestUrl: "first", target: "blob:first", targetSource: "media_node_current_src", targetMimeType: "audio/wav", prompt: "keep" });
  const tied = Object.freeze({ ...first, requestUrl: "second", target: "blob:second", prompt: "replace" });
  const result = merge(first, tied);
  assert.equal(result.requestUrl, "first"); assert.equal(result.target, "blob:first"); assert.equal(result.prompt, "keep");
  const improved = merge(first, { ...tied, target: "https://fixture.test/new.wav", targetSource: "network_audio_response" });
  assert.equal(improved.requestUrl, "second"); assert.equal(improved.prompt, "keep");
});

test("program handle merge candidates retain arrival order replace ties and cap at twelve", () => {
  const existing = Object.freeze(Array.from({ length: 13 }, (_, i) => Object.freeze({ url: "u" + i, score: i, origin: "first" })));
  const replacement = Object.freeze({ url: "u2", score: 2, origin: "second" });
  const incoming = Object.freeze([null, {}, replacement, Object.freeze({ url: "last-high", score: 1000 })]);
  const result = merge(Object.freeze({ targetCandidates: existing }), Object.freeze({ targetCandidates: incoming }));
  assert.deepEqual(result.targetCandidates.map((x) => x.url), existing.slice(0, 12).map((x) => x.url));
  assert.equal(result.targetCandidates[2], replacement); assert.equal(existing[2].origin, "first");
});

test("program handle merge action values remain target-first with nullish fallbacks", () => {
  const target = Object.freeze({ operation: "", actionName: "", actionInput: 0, prompt: "", durationSeconds: 0, aspectRatio: "" });
  const incoming = Object.freeze({ operation: "video", actionName: "act", actionInput: "input", prompt: "prompt", durationSeconds: 4, aspectRatio: "16:9" });
  const result = merge(target, incoming);
  for (const [name, value] of Object.entries(target)) assert.equal(result[name], value, name);
  const missing = merge({}, incoming);
  for (const [name, value] of Object.entries(incoming)) assert.equal(missing[name], value, name);
});

for (const [first, second, expected] of [["music_generating", "retry_without_app_visible", "retry_without_app_visible"], ["retry_without_app_visible", "video_player_ready", "video_player_ready"], ["music_player_ready", "video_player_ready", "music_player_ready"], ["unknown-first", "unknown-second", "unknown-first"]]) {
  test("program handle merge UI state " + first + " and " + second, () => assert.equal(merge({ uiState: first }, { uiState: second }).uiState, expected));
}

test("program handle merge second-stage schema omits inherited cookie and unknown fields", () => {
  const result = merge({ operation: "music", cookieHeader: "synthetic-first", extra: 1 }, { cookieHeader: "synthetic-second", unknown: 2 });
  assert.equal("cookieHeader" in result, false); assert.equal("extra" in result, false); assert.equal("unknown" in result, false);
  assert.deepEqual(Object.keys(result).sort(), ["operation", ...laneFields, "target", "targetSource", "targetMimeType", "targetCandidates", "actionName", "actionInput", "prompt", "durationSeconds", "aspectRatio", "uiState"].sort());
});

test("program handle merge integrates ranked media targets with a later generating snapshot", () => {
  const candidates = app.collectPlayerReadyTargetCandidates("video", {}, { videoUrls: [{ url: "https://fixture.test/ready.mp4" }] });
  const ready = { operation: "video", transportKind: "canvas_program_ws_candidate", target: candidates[0].url, targetSource: candidates[0].source, targetMimeType: candidates[0].mimeType, targetCandidates: candidates, uiState: "video_player_ready" };
  const later = { operation: "video", transportKind: "program_video_streamgenerate_candidate", requestBody: "form-data", uiState: "video_generating" };
  const merged = merge(ready, later);
  assert.equal(merged.uiState, "video_player_ready"); assert.equal(merged.target, ready.target);
  assert.equal(merged.requestBody, "form-data"); assert.equal(merged.transportKind, later.transportKind);
});
