import assert from "node:assert/strict";
import test from "node:test";
import { networkFixture, requestFixture, responseFixture, deferred, turn, streamUrl, pairText } from "./gemini-canvas-program-handle.network-fixtures.mjs";

for (const reject of [false, true]) {
  test("standalone stopped network capture ignores deferred text " + (reject ? "rejection" : "completion"), async (t) => {
    const f = await networkFixture(t), text = deferred();
    const pending = f.emit("response", responseFixture({ request: requestFixture({ url: streamUrl }), text: () => text.promise }));
    f.capture.stop();
    const stopped = structuredClone(f.state);
    if (reject) text.reject(new Error("old page closed")); else text.resolve(pairText);
    await pending;
    assert.deepEqual(f.state, stopped);
  });
}

for (const [operation, mime, extension] of [["music", "audio/wav", "wav"], ["video", "video/mp4", "mp4"]]) {
  for (const reject of [false, true]) {
    test("standalone stopped network capture ignores deferred " + operation + " " + (reject ? "rejection" : "completion"), async (t) => {
      const f = await networkFixture(t, operation), text = deferred();
      const request = requestFixture({ url: "https://files.googleusercontent.com/stale." + extension, method: "GET" });
      const pending = f.emit("response", responseFixture({ request, mime, text: () => text.promise }));
      f.capture.stop();
      const stopped = structuredClone(f.state);
      if (reject) text.reject(new Error("old media closed")); else text.resolve(pairText);
      await pending;
      assert.deepEqual(f.state, stopped);
    });
  }
}

test("standalone stopped network capture ignores deferred Cookie completion", async (t) => {
  const cookies = deferred(), f = await networkFixture(t, "text", { cookies: () => cookies.promise });
  await f.emit("request", requestFixture({ url: streamUrl }));
  f.capture.stop();
  const stopped = structuredClone(f.state);
  cookies.resolve([{ name: "fixture", value: "stale" }]);
  await turn();
  assert.deepEqual(f.state, stopped);
});

test("standalone stopped old capture cannot overwrite state adopted by a live page", async (t) => {
  const cookies = deferred(), text = deferred();
  const first = await networkFixture(t, "music", { cookies: () => cookies.promise });
  await first.emit("request", requestFixture({ url: streamUrl }));
  const pending = first.emit("response", responseFixture({ request: requestFixture({ url: "https://files.googleusercontent.com/stale.wav", method: "GET" }), mime: "audio/wav", text: () => text.promise }));
  first.capture.stop();
  const second = await networkFixture(t, "music", { state: first.state });
  assert.equal(second.state, first.state);
  await second.emit("request", requestFixture({ url: streamUrl, data: '"c_feedface", "r_1234abcd"', headers: { Cookie: "fixture=current" } }));
  await second.emit("response", responseFixture({ request: requestFixture({ url: "https://files.googleusercontent.com/current.wav", method: "GET" }), mime: "audio/wav" }));
  assert.equal(second.state.cookieHeader, "fixture=current");
  assert.equal(second.state.audioUrls.length, 1);
  const adopted = structuredClone(second.state);
  cookies.resolve([{ name: "fixture", value: "stale" }]);
  text.resolve(pairText);
  await pending;
  await turn();
  assert.deepEqual(second.state, adopted);
});

for (const event of ["request", "response", "websocket"]) {
  test("standalone stopped network capture ignores queued " + event + " before payload access", async (t) => {
    const f = await networkFixture(t), [queued] = f.page.listeners(event);
    f.capture.stop();
    const stopped = structuredClone(f.state);
    await queued({ url() { throw new Error("stale payload accessor called"); } });
    assert.deepEqual(f.state, stopped);
  });
}
