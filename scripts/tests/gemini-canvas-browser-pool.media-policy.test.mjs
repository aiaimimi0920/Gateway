import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();

for (const [label, text, expected] of [
  ["empty", null, false],
  ["unrelated", "Create music", false],
  ["partial action", "music_generation", false],
  ["accepted action", 'music_generation {"action_input":{}}', true],
  ["mixed-case track details", "TRACK DETAILS", true],
  ["busy", "Getting a lot of requests right now", true],
  ["curly apostrophe snag", "I\u2019ve hit a bit of a snag", true],
]) {
  test(`Media policy music pending signal: ${label}`, () => {
    assert.equal(app.bodyIndicatesMusicPendingOrBusy(text), expected);
  });
}

for (const [label, operation, contract, bodyText, expected] of [
  ["absent contract", "music", null, "Track details", false],
  ["ready player", "video", { uiState: "video_player_ready" }, "", true],
  ["concrete target", "image", { target: "fixture-target" }, "", true],
  ["proxy-only target", "image", { target: "fixture-target", transportKind: "canvas_program_ws_candidate", requestEnvelopeKind: "canvas_proxy_request" }, "", false],
  ["nonproxy envelope", "image", { target: "fixture-target", transportKind: "canvas_program_ws_candidate", requestEnvelopeKind: "other" }, "", true],
  ["music action", "music", { actionName: "music_generation" }, "", true],
  ["music snapshot", "music", {}, "Track details", true],
  ["video busy", "video", {}, "Please try again later", true],
  ["operation isolation", "image", {}, "video_placeholder Track details", false],
]) {
  test(`Media policy concrete progress: ${label}`, () => {
    const snapshot = { bodyText }, original = structuredClone({ contract, snapshot });
    assert.equal(app.invokeContractIndicatesConcreteProgress(operation, contract, snapshot), expected);
    assert.deepEqual({ contract, snapshot }, original);
  });
}

test("Media policy recent gate text examines only the last eight events without mutation", () => {
  const events = [
    { type: "response", text: "excluded old gate" },
    { type: "response", text: "  retained whitespace  " },
    { type: "request", text: "ignored request" },
    { type: "response", text: 123 },
    { type: "response", text: "   " },
    { type: "other", text: "ignored event" },
    { type: "response", text: "second gate" },
    { type: "response" },
    { type: "response", text: "last gate" },
  ];
  const original = structuredClone(events);
  assert.equal(app.recentMediaProviderGateText({ events }), "  retained whitespace  \nsecond gate\nlast gate");
  assert.deepEqual(events, original);
});

test("Media policy recent gate text tolerates absent and nonarray history", () => {
  for (const state of [null, {}, { events: "fixture" }, { events: [] }]) {
    assert.equal(app.recentMediaProviderGateText(state), "");
  }
});
