import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const input = '{"prompt":"synthetic action prompt","duration_seconds":8,"aspect_ratio":"16:9"}';
const actionText = `"action": "generate_video",\n"action_input": ${input},\n`;

test("action extraction returns no contract for absent text", () => {
  for (const value of [null, undefined, " ", "ordinary page text"]) {
    assert.deepEqual(app.extractCanvasProgramActionContractFromText(value), { canvasProgramAction: null, canvasProgramActionInput: null });
  }
});

test("action merging fills missing fields without replacing an earlier usable value", () => {
  const target = { canvasProgramAction: "generate_video", canvasProgramActionInput: null, untouched: true };
  app.mergeActionContract(target, Object.freeze({ canvasProgramAction: "ignored-later-action", canvasProgramActionInput: input }));
  app.mergeActionContract(target, { canvasProgramActionInput: "ignored-later-input" });
  app.mergeActionContract(target, null);
  app.mergeActionContract(null, { canvasProgramAction: "ignored" });
  assert.deepEqual(target, { canvasProgramAction: "generate_video", canvasProgramActionInput: input, untouched: true });
});

test("action extraction retains a line-delimited object payload without its trailing comma", () => {
  assert.deepEqual(app.extractCanvasProgramActionContractFromText(actionText), {
    canvasProgramAction: "generate_video", canvasProgramActionInput: input,
  });
});

test("quoted action input remains usable by escaped scalar extraction", () => {
  const contract = app.extractCanvasProgramActionContractFromText(`"action": "generate_video",\n"action_input": ${JSON.stringify(input)},\n`);
  assert.equal(contract.canvasProgramAction, "generate_video");
  assert.equal(app.extractQuotedScalar(contract.canvasProgramActionInput, ["prompt"]), "synthetic action prompt");
  assert.equal(app.extractQuotedScalar(contract.canvasProgramActionInput, ["aspect_ratio"]), "16:9");
});

test("quoted scalar extraction respects alias precedence and either quote style", () => {
  assert.equal(app.extractQuotedScalar("'aspect': ' 4:3 ', \"aspect_ratio\": \"16:9\"", ["aspect_ratio", "aspect"]), "16:9");
  assert.equal(app.extractQuotedScalar("'prompt': ' synthetic text '", ["prompt"]), "synthetic text");
  assert.equal(app.extractQuotedScalar("'prompt': ''", ["prompt"]), null);
  assert.equal(app.extractQuotedScalar(null, ["prompt"]), null);
});

test("number scalar extraction retains decimals and resolves aliases in caller order", () => {
  assert.equal(app.extractNumberScalar('"duration": 3, "duration_seconds": 8.5', ["duration_seconds", "duration"]), 8.5);
  assert.equal(app.extractNumberScalar("'duration': 0", ["duration"]), 0);
  for (const value of [null, '"duration": "unknown"', '"duration": -5', "unrelated"]) {
    assert.equal(app.extractNumberScalar(value, ["duration"]), null);
  }
});

test("duration fallback uses total player time including whole minutes", () => {
  assert.equal(app.extractDurationSecondsFromBodyText("Player 0:03 / 1:05"), 65);
  assert.equal(app.extractDurationSecondsFromBodyText("0:00 / 12:34"), 754);
  for (const value of [null, "", "Generating your video", "0:03"]) assert.equal(app.extractDurationSecondsFromBodyText(value), null);
});

for (const event of ["request", "response"]) {
  test(`captured ${event} action text supplies prompt duration and aspect to the invoke contract`, async (t) => {
    const page = new EventEmitter(), capture = app.startNetworkCapture(page, "video");
    t.after(() => capture.stop());
    const url = "https://generativelanguage.googleapis.com/v1beta/models/fixture:predictLongRunning";
    const request = { url: () => url, method: () => "POST", postData: () => actionText, headers: () => ({}) };
    const value = event === "request" ? request : {
      url: () => url, request: () => request, headers: () => ({
        "content-type": "application/json", "content-length": String(Buffer.byteLength(actionText, "utf8")),
      }),
      text: async () => actionText, status: () => 200,
    };
    for (const listener of page.listeners(event)) await listener(value);
    assert.deepEqual(capture.state.actionContract, { canvasProgramAction: "generate_video", canvasProgramActionInput: input });
    const contract = capture.state.invokeContract;
    assert.equal(contract.actionName, "generate_video");
    assert.equal(contract.prompt, "synthetic action prompt");
    assert.equal(contract.durationSeconds, 8);
    assert.equal(contract.aspectRatio, "16:9");
    assert.equal(contract.transportKind, "app_video_http");
    capture.stop();
    assert.deepEqual(page.eventNames(), []);
  });
}
