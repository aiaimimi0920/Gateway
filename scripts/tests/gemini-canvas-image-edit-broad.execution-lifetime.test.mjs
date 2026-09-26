import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { executionFixture, requestFixture } from "./gemini-canvas-image-edit-broad.fixtures.mjs";

const pageEvents = ["request", "response", "websocket"];
const cdpEvents = ["Network.requestWillBeSent", "Network.responseReceived", "Network.dataReceived", "Network.loadingFailed", "Network.loadingFinished"];
const count = (f, name) => f.calls.filter(([kind]) => kind === name).length;
const outcome = async (f, failAt) => {
  if (failAt) await assert.rejects(f.run(), (error) => error === f.failure);
  else assert.equal(await f.run(), undefined);
};

test("broad root success stops owned listeners and retains inherited listeners", async (t) => {
  const f = executionFixture(t), inherited = () => {};
  for (const [emitter, events] of [[f.page, pageEvents], [f.cdp, cdpEvents]]) {
    for (const event of events) emitter.on(event, inherited);
  }
  await f.run();
  for (const [emitter, events] of [[f.page, pageEvents], [f.cdp, cdpEvents]]) {
    for (const event of events) assert.deepEqual(emitter.listeners(event), [inherited]);
  }
  assert.equal(count(f, "close"), 1);
  assert.equal(f.artifact().ok, true);
});

for (const failAt of ["pages", "new page", "cdp session", "cdp Network.enable", "goto", "screenshot", "storage state", "write image-edit-broad.json", "stdout"]) {
  test("broad root cleanup preserves rejection and closes context at " + failAt, async (t) => {
    const f = executionFixture(t, { failAt, emptyContext: failAt === "new page" });
    await outcome(f, failAt);
    assert.equal(count(f, "close"), 1);
    for (const [emitter, events] of [[f.page, pageEvents], [f.cdp, cdpEvents]]) {
      for (const event of events) assert.equal(emitter.listenerCount(event), 0);
    }
  });
}

test("broad root launch failure owns no context or listeners", async (t) => {
  const f = executionFixture(t, { failAt: "launch" });
  await outcome(f, "launch");
  assert.equal(count(f, "close"), 0);
  assert.equal(count(f, "pages"), 0);
  for (const event of pageEvents) assert.equal(f.page.listenerCount(event), 0);
});

for (const failAt of [undefined, "screenshot"]) {
  const ending = failAt ? "rejection" : "success";
  test("broad root " + ending + " survives capture detach failure with context cleanup", async (t) => {
    const f = executionFixture(t, { failAt });
    let attempts = 0;
    f.page.off = () => { attempts += 1; throw new Error("fixture detach failure"); };
    await outcome(f, failAt);
    assert.equal(attempts, 3);
    assert.equal(count(f, "close"), 1);
    for (const event of cdpEvents) assert.equal(f.cdp.listenerCount(event), 0);
  });

  test("broad root prevents upload artifact writes after " + ending, async (t) => {
    const f = executionFixture(t, { failAt }), gate = Promise.withResolvers(), entered = Promise.withResolvers();
    const evaluate = f.page.evaluate;
    let pending;
    f.page.evaluate = async (callback) => {
      if (!pending) {
        const [request] = f.page.listeners("request");
        pending = request(requestFixture({ url: "https://push.clients6.google.com/upload/late", headers: { "x-goog-upload-command": "upload, finalize" }, overrides: { postDataBuffer: () => { entered.resolve(); return gate.promise; } } }));
        await entered.promise;
      }
      return evaluate(callback);
    };
    await outcome(f, failAt);
    const outDir = path.dirname(f.writes[0]), files = fs.readdirSync(outDir).sort();
    assert.ok(pending);
    gate.resolve(Buffer.from([0, 255, 128]));
    await pending;
    assert.deepEqual(fs.readdirSync(outDir).sort(), files);
    assert.equal(count(f, "close"), 1);
  });
}
