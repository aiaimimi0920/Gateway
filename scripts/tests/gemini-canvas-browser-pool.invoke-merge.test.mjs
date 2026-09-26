import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const merge = app.mergeInvokeContract;
const oldUrl = "https://fixture.invalid/old.mp4", newUrl = "https://fixture.invalid/new.mp4";
const network = (url) => ({ operation: "video", target: url, targetSource: "network_video_response", targetMimeType: "video/mp4" });

test("invoke merge preserves absent updates and shallow-copies the initial contract", () => {
  const original = Object.freeze({ operation: "video", targetCandidates: Object.freeze([]) });
  assert.equal(merge(original, null), original);
  const initialized = merge(null, original);
  assert.deepEqual(initialized, original);
  assert.notEqual(initialized, original);
  assert.equal(initialized.targetCandidates, original.targetCandidates);
});

test("invoke UI progress never regresses and equal-rank player states remain stable", () => {
  const levels = ["unrecognized", "video_generating", "retry_without_app_visible", "video_player_ready"];
  for (let lower = 0; lower < levels.length; lower += 1) {
    for (let higher = lower; higher < levels.length; higher += 1) {
      assert.equal(merge({ uiState: levels[lower] }, { uiState: levels[higher] }).uiState, levels[higher]);
      assert.equal(merge({ uiState: levels[higher] }, { uiState: levels[lower] }).uiState, levels[higher]);
    }
  }
  assert.equal(merge({ uiState: "music_player_ready" }, { uiState: "video_player_ready" }).uiState, "music_player_ready");
  assert.equal(merge({ uiState: "music_generating" }, { uiState: "video_generating" }).uiState, "music_generating");
});

test("invoke transport selection preserves the complete lane precedence in either arrival order", () => {
  for (const operation of ["music", "video"]) {
    const lanes = [
      {}, { requestEnvelopeKind: "canvas_proxy_request" }, { transportKind: "canvas_program_ws_candidate" },
      { transportKind: `program_${operation}_batchexecute_candidate` }, { requestEnvelopeKind: "page_stream_generate_form" },
      { transportKind: `program_${operation}_streamgenerate_candidate` },
    ].map((lane, index) => Object.freeze({ operation, ...lane, requestUrl: `https://fixture.invalid/lane-${index}` }));
    for (let lower = 0; lower < lanes.length; lower += 1) {
      for (let higher = lower + 1; higher < lanes.length; higher += 1) {
        assert.equal(merge(lanes[lower], lanes[higher]).requestUrl, lanes[higher].requestUrl);
        assert.equal(merge(lanes[higher], lanes[lower]).requestUrl, lanes[higher].requestUrl);
      }
    }
  }
});

test("invoke lane metadata falls back per field without discarding empty primary values", () => {
  const fields = ["wsUrl", "apiStyle", "requestPath", "requestUrl", "requestBody", "requestRpcId", "responseRpcId", "sourcePath", "modelHint"];
  const secondary = { transportKind: "canvas_program_ws_candidate", requestEnvelopeKind: "canvas_proxy_request" };
  const primary = { transportKind: "program_video_streamgenerate_candidate" };
  for (const [index, field] of fields.entries()) {
    secondary[field] = `fallback-${field}`;
    primary[field] = index % 3 === 0 ? null : index % 3 === 1 ? "" : `primary-${field}`;
  }
  const result = merge(Object.freeze(secondary), Object.freeze(primary));
  assert.equal(result.transportKind, primary.transportKind);
  assert.equal(result.requestEnvelopeKind, secondary.requestEnvelopeKind);
  for (const [index, field] of fields.entries()) assert.equal(result[field], index % 3 === 0 ? secondary[field] : primary[field]);
});

test("invoke target quality can improve independently of the stronger request lane", () => {
  const prior = { operation: "video", target: "blob:https://fixture.invalid/player", targetSource: "media_node_current_src", transportKind: "program_video_streamgenerate_candidate", requestUrl: "https://fixture.invalid/stream" };
  const update = { ...network(newUrl), transportKind: "canvas_program_ws_candidate", requestUrl: "https://fixture.invalid/ws" };
  const result = merge(prior, update);
  assert.equal(result.target, newUrl);
  assert.equal(result.targetSource, update.targetSource);
  assert.equal(result.targetMimeType, "video/mp4");
  assert.equal(result.requestUrl, prior.requestUrl);
  assert.equal(result.transportKind, prior.transportKind);
});

test("equal-ranked invoke targets and lanes retain the first target and request metadata", () => {
  const result = merge({ ...network(oldUrl), requestBody: "first" }, { ...network(newUrl), requestBody: "later" });
  assert.equal(result.target, oldUrl);
  assert.equal(result.requestBody, "first");
});

test("a better target breaks a tied invoke lane without changing first action values", () => {
  const result = merge({ operation: "video", target: "blob:https://fixture.invalid/player", requestBody: "first", prompt: "original" }, { ...network(newUrl), requestBody: "later", prompt: "replacement" });
  assert.equal(result.target, newUrl);
  assert.equal(result.requestBody, "later");
  assert.equal(result.prompt, "original");
});

test("invoke candidate dedup keeps greater or equal score metadata without mutating inputs", () => {
  const first = Object.freeze({ url: oldUrl, score: 4, source: "first" });
  const tied = Object.freeze({ url: oldUrl, score: 4, source: "later" });
  const stronger = Object.freeze({ url: newUrl, score: 8, source: "stronger" });
  const left = Object.freeze([first, stronger]), right = Object.freeze([null, {}, tied, Object.freeze({ url: newUrl, score: 3 })]);
  const result = merge(Object.freeze({ targetCandidates: left }), Object.freeze({ targetCandidates: right }));
  assert.deepEqual(result.targetCandidates, [tied, stronger]);
  assert.notEqual(result.targetCandidates, left);
  assert.deepEqual(left, [first, stronger]);
});

test("invoke candidate cap retains arrival order instead of re-sorting by score", () => {
  const candidates = Object.freeze(Array.from({ length: 15 }, (_, index) => Object.freeze({ url: `https://fixture.invalid/${index}.mp4`, score: index })));
  const result = merge({ targetCandidates: candidates.slice(0, 8) }, { targetCandidates: candidates.slice(8) });
  assert.deepEqual(result.targetCandidates, candidates.slice(0, 12));
});

test("invoke action fields use nullish fallback while preserving empty text and zero duration", () => {
  const actionInput = Object.freeze({ prompt: "submitted" });
  const result = merge(Object.freeze({ operation: "music", actionName: "", actionInput, prompt: "", durationSeconds: 0, aspectRatio: null }), Object.freeze({ operation: "video", actionName: "later", actionInput: {}, prompt: "later", durationSeconds: 8, aspectRatio: "16:9" }));
  assert.equal(result.operation, "music");
  assert.equal(result.actionName, "");
  assert.equal(result.actionInput, actionInput);
  assert.equal(result.prompt, "");
  assert.equal(result.durationSeconds, 0);
  assert.equal(result.aspectRatio, "16:9");
});

test("assembled ready media survives a later generating snapshot and accepts its stronger lane", () => {
  const ready = app.buildCanvasProgramInvokeContract("video", null, { videoInvokePaths: ["/v1/models/fixture:predictLongRunning"] },
    { bodyText: "Your video is ready!", buttons: [], mediaNodes: [] }, null, { videoUrls: [{ url: newUrl }] });
  const result = merge(ready, { operation: "video", uiState: "video_generating", transportKind: "program_video_streamgenerate_candidate", requestUrl: "https://fixture.invalid/StreamGenerate", requestBody: "synthetic-body" });
  assert.equal(result.target, newUrl);
  assert.equal(result.uiState, "video_player_ready");
  assert.equal(result.transportKind, "program_video_streamgenerate_candidate");
  assert.equal(result.requestBody, "synthetic-body");
});
