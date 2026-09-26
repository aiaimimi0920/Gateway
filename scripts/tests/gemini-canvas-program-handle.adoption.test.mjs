import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { executionFixture } from "./gemini-canvas-program-handle.execution-fixtures.mjs";
import { networkFixture, responseFixture, requestFixture, deferred, turn, streamUrl, pairText } from "./gemini-canvas-program-handle.network-fixtures.mjs";

test("execution waits for capture readiness before its first navigation", async (t) => {
  const ready = deferred(), f = await executionFixture(t, { captureReady: ready.promise });
  const run = f.run();
  try {
    await turn();
    assert.equal(f.captures.length, 1);
    assert.ok(!f.calls.some(([kind]) => kind === "goto"));
  } finally { ready.resolve(); }
  await run;
  assert.ok(f.calls.findIndex(([kind]) => kind === "capture ready") < f.calls.findIndex(([kind]) => kind === "goto"));
});

test("page adoption retains pending response evidence and the same listener owner", async (t) => {
  const body = deferred(), f = await networkFixture(t);
  const pending = f.emit("response", responseFixture({ request: requestFixture({ url: streamUrl }), text: () => body.promise }));
  const next = new EventEmitter();
  next.context = () => f.page.context();
  const listeners = f.page.listeners("response");
  await f.capture.adoptPage(next);
  assert.deepEqual(f.page.listeners("response"), listeners);
  body.resolve(pairText);
  await pending;
  assert.equal(f.state.responses[0].bodyText, pairText);
  await assert.rejects(f.capture.adoptPage({ context: () => new EventEmitter() }), /outside the active capture context/);
  assert.doesNotThrow(f.capture.assertHealthy);
});
