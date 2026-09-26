import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const emptyHints = { invokeBaseUrl: null, musicWsUrl: null, videoInvokePath: null };
const origin = "https://generativelanguage.googleapis.com";
const videoUrl = `${origin}/v1beta/models/fixture:predictLongRunning?key=synthetic#fragment`;
const musicUrl = "wss://generativelanguage.googleapis.com/ws/fixture.BidiGenerateMusic?key=synthetic#fragment";

test("video transport discovery separates the invoke base from the request path", () => {
  assert.deepEqual(app.extractTransportHintsFromNetworkUrl(videoUrl), {
    invokeBaseUrl: `${origin}/v1beta`, musicWsUrl: null,
    videoInvokePath: "/v1beta/models/fixture:predictLongRunning",
  });
});

test("music transport discovery removes query and fragment from the websocket hint", () => {
  assert.deepEqual(app.extractTransportHintsFromNetworkUrl(musicUrl), {
    invokeBaseUrl: null, musicWsUrl: musicUrl.split("?")[0], videoInvokePath: null,
  });
});

test("transport discovery ignores malformed and unrelated URL inputs", () => {
  for (const value of [null, undefined, " ", "not a URL", "/v1/models/fixture:predictLongRunning", `${origin}/unrelated?BidiGenerateMusic=1`]) {
    assert.deepEqual(app.extractTransportHintsFromNetworkUrl(value), emptyHints);
  }
});

test("transport discovery requires a base prefix and an exact video action suffix", () => {
  assert.deepEqual(app.extractTransportHintsFromNetworkUrl(`${origin}/models/fixture:predictLongRunning`), {
    ...emptyHints, videoInvokePath: "/models/fixture:predictLongRunning",
  });
  assert.deepEqual(app.extractTransportHintsFromNetworkUrl(`${origin}/v1/models/fixture:predictLongRunning/operations`), {
    ...emptyHints, invokeBaseUrl: `${origin}/v1`,
  });
});

test("transport hint merging preserves discovery order and deduplicates normalized values", () => {
  const target = { invokeBaseUrls: [" https://fixture.invalid/v1 ", "https://fixture.invalid/v1"], untouched: true };
  const incoming = Object.freeze({ invokeBaseUrl: "https://fixture.invalid/v2", musicWsUrl: " wss://fixture.invalid/music ", videoInvokePath: " /v1/models/fixture:predictLongRunning " });
  app.mergeTransportHints(target, incoming);
  app.mergeTransportHints(target, incoming);
  app.mergeTransportHints(target, null);
  assert.deepEqual(target, {
    invokeBaseUrls: ["https://fixture.invalid/v1", "https://fixture.invalid/v2"],
    musicWsUrls: ["wss://fixture.invalid/music"], videoInvokePaths: ["/v1/models/fixture:predictLongRunning"], untouched: true,
  });
});

test("transport hint presence recognizes each independently discovered lane", () => {
  assert.equal(app.hasTransportHints(null), false);
  assert.equal(app.hasTransportHints({}), false);
  assert.equal(app.hasTransportHints({ invokeBaseUrls: [], musicWsUrls: [], videoInvokePaths: [] }), false);
  for (const key of ["invokeBaseUrls", "musicWsUrls", "videoInvokePaths"]) {
    assert.equal(app.hasTransportHints(Object.freeze({ [key]: Object.freeze(["synthetic"]) })), true);
  }
});

for (const event of ["request", "response", "websocket"]) {
  test(`network capture discovers transport hints from its ${event} listener and detaches only owned listeners`, async (t) => {
    const page = new EventEmitter(), external = () => {};
    page.on(event, external);
    const capture = app.startNetworkCapture(page, "video");
    t.after(() => capture.stop());
    const request = { url: () => videoUrl, method: () => "POST", postData: () => null, headers: () => ({}) };
    const value = event === "request" ? request : event === "response"
      ? { url: () => videoUrl, request: () => request, headers: () => ({ "content-type": "application/json" }), text: async () => "{}", status: () => 200 }
      : { url: () => musicUrl };
    // Await asynchronous response handling without replacing the production listeners.
    for (const listener of page.listeners(event)) await listener(value);
    assert.deepEqual(capture.state.transportHints, event === "websocket" ? {
      invokeBaseUrls: [], musicWsUrls: [musicUrl.split("?")[0]], videoInvokePaths: [],
    } : {
      invokeBaseUrls: [`${origin}/v1beta`], musicWsUrls: [], videoInvokePaths: ["/v1beta/models/fixture:predictLongRunning"],
    });
    assert.equal(app.hasTransportHints(capture.state.transportHints), true);
    assert.equal(capture.state.events.at(-1).type, event);
    capture.stop();
    for (const name of ["request", "response", "websocket"]) assert.equal(page.listenerCount(name), name === event ? 1 : 0);
    assert.equal(page.listeners(event)[0], external);
  });
}
