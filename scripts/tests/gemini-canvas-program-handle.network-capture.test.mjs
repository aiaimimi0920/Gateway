import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { networkFixture, requestFixture, responseFixture, deferred, turn, streamUrl, rpcUrl, pairText } from "./gemini-canvas-program-handle.network-fixtures.mjs";

for (const event of ["request", "response", "websocket"]) {
  test("standalone active network captures own distinct " + event + " listeners and state", async (t) => {
    const first = await networkFixture(t), second = await networkFixture(t, "text", { page: first.page });
    const value = event === "request" ? requestFixture({ data: pairText }) : event === "response"
      ? responseFixture({ text: async () => pairText }) : { url: () => "wss://generativelanguage.googleapis.com/ws" };
    await first.emit(event, value);
    const records = event === "response" ? "responses" : "requests";
    assert.notEqual(first.state, second.state);
    assert.equal(first.state[records].length, 1);
    assert.equal(second.state[records].length, 1);
    const surviving = first.page.listeners(event)[1];
    first.capture.stop();
    assert.deepEqual(first.page.listeners(event), [surviving]);
    await second.emit(event, value);
    assert.equal(first.state[records].length, 1);
    assert.equal(second.state[records].length, 2);
  });
}

test("standalone active network capture preserves external listeners across repeated stop", async (t) => {
  const page = new EventEmitter(), before = () => {}, after = () => {};
  for (const event of ["request", "response", "websocket"]) page.on(event, before);
  const { capture } = await networkFixture(t, "text", { page });
  for (const event of ["request", "response", "websocket"]) page.on(event, after);
  capture.stop(); capture.stop();
  for (const event of ["request", "response", "websocket"]) assert.deepEqual(page.listeners(event), [before, after]);
});

test("standalone active network capture resumes with accumulated state after page switch", async (t) => {
  const first = await networkFixture(t);
  await first.emit("request", requestFixture({ data: pairText }));
  first.capture.stop();
  const second = await networkFixture(t, "text", { state: first.state });
  assert.equal(second.state, first.state);
  await first.emit("request", requestFixture());
  await second.emit("websocket", { url: () => "wss://generativelanguage.googleapis.com/ws" });
  assert.equal(second.state.requests.length, 2);
  assert.equal(second.state.handlePairs[0].conversationId, "c_deadbeef");
});

test("standalone active network capture retains RPC record shape and preview limits", async (t) => {
  const f = await networkFixture(t), data = pairText + "x".repeat(1700);
  await f.emit("request", requestFixture({ url: rpcUrl + "&source-path=%2Fapp%2Fdeadbeef", data, headers: { Cookie: "fixture=active" } }));
  assert.equal(f.state.requests[0].postDataPreview.length, 1600);
  assert.equal(f.state.requests[0].resourceType, "fetch");
  assert.equal(f.state.rpcCaptures[0].sourcePath, "/app/deadbeef");
  assert.equal(f.state.rpcCaptures[0].cookieHeader, "fixture=active");
  assert.equal(f.state.rpcCaptures[0].bodyText, data);
  assert.equal(f.state.cookieHeader, undefined);
});

test("standalone active network capture ignores unrelated URLs without reading payloads", async (t) => {
  const f = await networkFixture(t), initial = structuredClone(f.state);
  const ignored = new Proxy({ url: () => "https://fixture.invalid/unrelated" }, { get(target, key) {
    if (key === "url") return target.url;
    throw new Error("unrelated payload accessor " + String(key));
  } });
  for (const event of ["request", "response", "websocket"]) await f.emit(event, ignored);
  assert.deepEqual(f.state, initial);
});

for (const reject of [false, true]) {
  test("standalone active network capture retains deferred text " + (reject ? "rejection" : "completion"), async (t) => {
    const f = await networkFixture(t), text = deferred();
    const pending = f.emit("response", responseFixture({ request: requestFixture({ url: streamUrl }), text: () => text.promise }));
    assert.equal(f.state.responses.length, 0);
    if (reject) text.reject(new Error("body unavailable")); else text.resolve(pairText);
    await pending;
    assert.equal(f.state.responses.length, 1);
    assert.equal(f.state.responses[0].bodyPreview, reject ? "" : pairText);
    assert.equal(f.state.rpcCaptures[0].bodyText, reject ? "" : pairText);
  });

  test("standalone active network capture retains deferred Cookie " + (reject ? "rejection" : "completion"), async (t) => {
    const cookies = deferred(), f = await networkFixture(t, "text", { cookies: () => cookies.promise });
    await f.emit("request", requestFixture({ url: streamUrl }));
    if (reject) cookies.reject(new Error("context unavailable")); else cookies.resolve([{ name: "fixture", value: "active" }]);
    await turn();
    assert.equal(f.state.cookieHeader, reject ? undefined : "fixture=active");
  });
}

for (const [operation, extension, mime, key] of [["music", "wav", "audio/wav", "audioUrls"], ["video", "mp4", "video/mp4", "videoUrls"]]) {
  test("standalone active network capture retains " + operation + " media URL without binary reads", async (t) => {
    const f = await networkFixture(t, operation), url = "https://files.googleusercontent.com/current." + extension;
    await f.emit("response", responseFixture({ request: requestFixture({ url, method: "GET" }), mime, body: () => { throw new Error("unexpected body read"); } }));
    assert.equal(f.state[key].length, 1);
    assert.equal(f.state[key][0].url, url);
    assert.equal(f.state[key][0].mimeType, mime);
    assert.equal(f.state.responses.length, 1);
  });
}
