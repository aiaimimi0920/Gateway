import assert from "node:assert/strict";
import test from "node:test";
import { networkFixture, requestFixture, responseFixture, deferred, turn, streamUrl, pairText } from "./gemini-canvas-browser-pool.network-fixtures.mjs";

for (const reject of [false, true]) {
  test(`stopped network capture ignores deferred text ${reject ? "rejection" : "completion"}`, async (t) => {
    const { capture, state, emit } = networkFixture(t), text = deferred();
    const pending = emit("response", responseFixture({ request: requestFixture({ url: streamUrl }),
      headers: { "content-length": String(Buffer.byteLength(pairText)) }, text: () => text.promise }));
    assert.equal(typeof state.streamGenerateResponseAt, "number");
    capture.stop();
    const stoppedState = structuredClone(state);
    if (reject) text.reject(new Error("old page closed"));
    else text.resolve(pairText);
    await pending;
    assert.deepEqual(state, stoppedState);
  });
}

for (const [operation, mime] of [["image", "image/png"], ["tts", "audio/wav"], ["music", "audio/wav"]]) {
  for (const reject of [false, true]) {
    test(`stopped network capture ignores deferred ${operation} body ${reject ? "rejection" : "completion"}`, async (t) => {
      const { capture, state, emit } = networkFixture(t, operation), body = deferred();
      const request = requestFixture({ url: "https://files.googleusercontent.com/stale", method: "GET" });
      const pending = emit("response", responseFixture({ request, mime, headers: { "content-length": "3" }, body: () => body.promise }));
      capture.stop();
      const stoppedState = structuredClone(state);
      if (reject) body.reject(new Error("old media page closed"));
      else body.resolve(Buffer.from([1, 2, 3]));
      await pending;
      assert.deepEqual(state, stoppedState);
    });
  }
}

test("stopped network capture ignores deferred Cookie completion", async (t) => {
  const cookies = deferred();
  const { capture, state, emit } = networkFixture(t, "text", { cookies: () => cookies.promise });
  await emit("request", requestFixture({ url: streamUrl }));
  capture.stop();
  const stoppedState = structuredClone(state);
  cookies.resolve([{ name: "fixture", value: "stale" }]);
  await turn();
  assert.deepEqual(state, stoppedState);
});

test("stopped old-page capture cannot overwrite state adopted by a live page", async (t) => {
  const cookies = deferred(), text = deferred(), body = deferred();
  const first = networkFixture(t, "image", { cookies: () => cookies.promise });
  await first.emit("request", requestFixture({ url: streamUrl }));
  const textPending = first.emit("response", responseFixture({ headers: { "content-length": String(Buffer.byteLength(pairText)) }, text: () => text.promise }));
  const mediaRequest = requestFixture({ url: "https://files.googleusercontent.com/stale.png", method: "GET" });
  const bodyPending = first.emit("response", responseFixture({ request: mediaRequest, mime: "image/png",
    headers: { "content-length": "3" }, body: () => body.promise }));
  first.capture.stop();
  const second = networkFixture(t, "image", { state: first.state });
  assert.equal(second.state, first.state);
  await second.emit("request", requestFixture({ url: streamUrl, data: '"c_feedface", "r_1234abcd"', headers: { Cookie: "fixture=current" } }));
  await second.emit("response", responseFixture({ request: requestFixture({ url: "https://files.googleusercontent.com/current.png", method: "GET" }), mime: "image/png" }));
  assert.equal(second.state.cookieHeader, "fixture=current");
  assert.equal(second.state.handlePairs[0].conversationId, "c_feedface");
  assert.equal(second.state.imageUrls.length, 1);
  const adoptedState = structuredClone(second.state);
  cookies.resolve([{ name: "fixture", value: "stale" }]);
  text.resolve(pairText);
  body.resolve(Buffer.from([4, 5, 6]));
  await Promise.all([textPending, bodyPending]);
  await turn();
  assert.deepEqual(second.state, adoptedState);
});

for (const event of ["request", "response", "websocket"]) {
  test(`stopped network capture ignores already queued ${event} callbacks before payload access`, async (t) => {
    const { capture, state, page } = networkFixture(t);
    const [queued] = page.listeners(event);
    capture.stop();
    const stoppedState = structuredClone(state);
    await queued({ url: () => { throw new Error("stale payload accessor called"); } });
    assert.deepEqual(state, stoppedState);
  });
}
