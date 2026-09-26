import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { networkFixture, requestFixture, responseFixture, dispatch, deferred, turn, streamUrl, pairText } from "./gemini-canvas-browser-pool.network-fixtures.mjs";
import {
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
} from "../gemini-canvas-browser-pool-body.mjs";

for (const event of ["request", "response", "websocket"]) {
  test(`independent network captures own distinct ${event} listeners and state`, async (t) => {
    const first = networkFixture(t), second = networkFixture(t, "bootstrap_program", { page: first.page });
    const value = event === "request" ? requestFixture({ data: pairText }) : event === "response"
      ? responseFixture({ text: async () => pairText }) : { url: () => "wss://fixture.invalid/ws" };
    await first.emit(event, value);
    assert.notEqual(first.state, second.state);
    assert.notEqual(first.state.events, second.state.events);
    assert.equal(first.state.events.length, 1);
    assert.equal(second.state.events.length, 1);
    const surviving = first.page.listeners(event)[1];
    first.capture.stop();
    assert.deepEqual(first.page.listeners(event), [surviving]);
    await second.emit(event, value);
    assert.equal(first.state.events.length, 1);
    assert.equal(second.state.events.length, 2);
  });
}

test("network capture preserves external listeners before and after repeated stop", (t) => {
  const page = new EventEmitter(), before = () => {}, after = () => {};
  for (const event of ["request", "response", "websocket"]) page.on(event, before);
  const { capture } = networkFixture(t, "video", { page });
  for (const event of ["request", "response", "websocket"]) page.on(event, after);
  capture.stop();
  capture.stop();
  for (const event of ["request", "response", "websocket"]) assert.deepEqual(page.listeners(event), [before, after]);
});

test("network capture reuses the same accumulated state after a page switch", async (t) => {
  const first = networkFixture(t);
  await first.emit("request", requestFixture({ data: pairText }));
  first.capture.stop();
  const second = networkFixture(t, "bootstrap_program", { state: first.state });
  assert.equal(second.state, first.state);
  await first.emit("request", requestFixture());
  assert.equal(second.state.events.length, 1);
  await second.emit("websocket", { url: () => "wss://fixture.invalid/new-page" });
  assert.equal(second.state.events.length, 2);
  assert.equal(second.state.handlePairs[0].conversationId, "c_deadbeef");
  assert.deepEqual(first.page.eventNames(), []);
});

test("network events retain the most recent 160 entries", async (t) => {
  const { state, emit } = networkFixture(t);
  for (let index = 0; index < 163; index += 1) await emit("websocket", { url: () => `wss://fixture.invalid/${index}` });
  assert.equal(state.events.length, 160);
  assert.equal(state.events[0].url, "wss://fixture.invalid/3");
  assert.equal(state.events.at(-1).url, "wss://fixture.invalid/162");
});

test("network capture ignores unrelated request and response URLs before reading payloads", async (t) => {
  const { state, emit } = networkFixture(t);
  const forbidden = () => { throw new Error("unrelated payload read"); };
  await emit("request", { url: () => "https://fixture.invalid/data", method: forbidden });
  await emit("response", { url: () => "https://fixture.invalid/data", request: forbidden });
  assert.deepEqual(state.events, []);
  assert.deepEqual(state.rpcCaptures, []);
});

test("network capture preserves request postData failure fallback", async (t) => {
  const { state, emit } = networkFixture(t), request = requestFixture();
  request.postData = () => { throw new Error("post body unavailable"); };
  await emit("request", request);
  assert.equal(state.events[0].postData, null);
  assert.equal(state.rpcCaptures[0].bodyText, null);
});

test("network capture preserves response text failure diagnostics", async (t) => {
  const { state, emit } = networkFixture(t);
  await emit("response", responseFixture({ headers: { "content-length": "1" }, text: async () => { throw new Error("fixture detached"); } }));
  assert.equal(state.rpcCaptures[0].bodyText, "[[response text unavailable: fixture detached]]");
  assert.equal(state.events[0].text, state.rpcCaptures[0].bodyText);
});

test("network capture rejects declared oversized program text before reading it", async (t) => {
  const { state, emit } = networkFixture(t);
  let reads = 0;
  await emit("response", responseFixture({
    headers: { "content-length": String(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES + 1) },
    text: async () => { reads++; return pairText; },
  }));
  assert.equal(reads, 0);
  assert.match(state.rpcCaptures[0].bodyText, /body exceeded/);
  assert.equal(state.handlePairs.length, 0);
});

test("network capture refuses unknown-length native reads and retains URL-only media", async (t) => {
  const text = networkFixture(t);
  let textReads = 0;
  await text.emit("response", responseFixture({
    headers: { "content-length": undefined },
    text: async () => { textReads++; return pairText; },
  }));
  assert.equal(textReads, 0);
  assert.match(text.state.rpcCaptures[0].bodyText, /no declared size/);
  assert.deepEqual(text.state.handlePairs, []);

  const media = networkFixture(t, "image");
  let bodyReads = 0;
  const url = "https://files.googleusercontent.com/unknown-length.png";
  await media.emit("response", responseFixture({
    request: requestFixture({ url, method: "GET" }),
    mime: "image/png",
    headers: { "content-length": undefined },
    body: async () => { bodyReads++; return Buffer.from([1]); },
  }));
  assert.equal(bodyReads, 0);
  assert.deepEqual(media.state.imageUrls, [{ url, mimeType: "image/png", bodyBase64: null }]);
});

for (const event of ["request", "response"]) {
  test(`network ${event} diagnostics retain header redaction and allowlisting`, async (t) => {
    const { state, emit } = networkFixture(t);
    const headers = { Authorization: "synthetic-fixture", "X-Goog-Api-Key": "", "Content-Type": "application/json", Cookie: "fixture=one", "X-Ignored": "fixture" };
    const request = requestFixture({ headers });
    await emit(event, event === "request" ? request : responseFixture({ request, headers }));
    const expected = { authorization: "<present>", "x-goog-api-key": "<missing>", "content-type": "application/json" };
    assert.deepEqual(event === "request" ? state.events[0].headers : state.events[0].requestHeaders, expected);
    assert.deepEqual(event === "request" ? state.rpcCaptures[0].headers : state.rpcCaptures[0].responseHeaders, expected);
  });
}

test("active network capture completes a deferred response text read", async (t) => {
  const { state, emit } = networkFixture(t), text = deferred();
  const pending = emit("response", responseFixture({ headers: { "content-length": String(Buffer.byteLength(pairText)) }, text: () => text.promise }));
  assert.deepEqual(state.events, []);
  text.resolve(pairText);
  await pending;
  assert.equal(state.handlePairs[0].responseId, "r_cafebabe");
  assert.equal(state.rpcCaptures[0].bodyText, pairText);
});

for (const [operation, extension, mime, key] of [
  ["image", "png", "image/png", "imageUrls"], ["tts", "wav", "audio/wav", "audioUrls"], ["music", "wav", "audio/wav", "audioUrls"],
]) {
  test(`active network capture completes deferred ${operation} media bytes`, async (t) => {
    const { state, emit } = networkFixture(t, operation), body = deferred();
    const url = `https://files.googleusercontent.com/fixture.${extension}`;
    const pending = emit("response", responseFixture({ request: requestFixture({ url, method: "GET" }), mime,
      headers: { "content-length": "3" }, body: () => body.promise }));
    assert.deepEqual(state[key], []);
    body.resolve(Buffer.from([1, 2, 3]));
    await pending;
    assert.deepEqual(state[key], [{ url, mimeType: mime, bodyBase64: "AQID" }]);
  });
}

for (const [operation, mime, key] of [["image", "image/png", "imageUrls"], ["music", "audio/wav", "audioUrls"]]) {
  test(`network capture retains URL-only ${operation} assets after body rejection`, async (t) => {
  const { state, emit } = networkFixture(t, operation), url = "https://files.googleusercontent.com/fixture";
    await emit("response", responseFixture({ request: requestFixture({ url, method: "GET" }), mime,
      headers: { "content-length": "1" }, body: async () => { throw new Error("body unavailable"); } }));
    assert.deepEqual(state[key], [{ url, mimeType: mime, bodyBase64: null }]);
  });
}

test("network capture keeps URL-only image fallback when the declared body is oversized", async (t) => {
  const { state, emit } = networkFixture(t, "image");
  let reads = 0;
  const url = "https://files.googleusercontent.com/fixture.png";
  await emit("response", responseFixture({
    request: requestFixture({ url, method: "GET" }),
    mime: "image/png",
    headers: { "content-length": String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1) },
    body: async () => { reads++; return Buffer.from([1]); },
  }));
  assert.equal(reads, 0);
  assert.deepEqual(state.imageUrls, [{ url, mimeType: "image/png", bodyBase64: null }]);
});

test("network capture deduplicates video URLs without reading the video body", async (t) => {
  const { state, emit } = networkFixture(t, "video"), url = "https://files.googleusercontent.com/fixture.mp4";
  const forbidden = () => { throw new Error("video body read"); };
  const response = responseFixture({ request: requestFixture({ url, method: "GET" }), mime: "video/mp4", body: forbidden, text: forbidden });
  await emit("response", response);
  await emit("response", response);
  assert.deepEqual(state.videoUrls, [{ url, mimeType: "video/mp4" }]);
});

for (const reject of [false, true]) {
  test(`active network capture handles deferred cookie ${reject ? "rejection" : "completion"}`, async (t) => {
    const cookies = deferred();
    const { state, page } = networkFixture(t, "text", { cookies: () => cookies.promise });
    await dispatch(page, "request", requestFixture({ url: streamUrl }));
    assert.equal(state.cookieHeader, undefined);
    if (reject) cookies.reject(new Error("context closed"));
    else cookies.resolve([{ name: "fixture", value: "one" }]);
    await turn();
    assert.equal(state.cookieHeader, reject ? undefined : "fixture=one");
  });
}
