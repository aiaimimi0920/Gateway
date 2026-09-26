import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { debugHarness } from "./gemini-canvas-browser-pool.debug-fixtures.mjs";

const app = await importTestableScript();
const stages = (h, name) => h.calls.filter(([stage]) => stage === name);

function assertReleased(h, expectedStops = 1) {
  for (const name of ["request", "response", "websocket"]) {
    assert.equal(h.page.listenerCount(name), 1, "debug listeners must return to inherited count: " + name);
    assert.deepEqual(h.page.listeners(name), [h.inherited], "only inherited listeners remain: " + name);
  }
  assert.equal(h.stops, expectedStops, "debug must stop its acquired capture exactly once");
  const state = structuredClone(h.state);
  h.page.emit("websocket", { url: () => "wss://fixture.invalid/late" });
  assert.deepEqual(h.state, state, "later page events cannot mutate completed debug capture");
}

test("Debug success stops real capture once and preserves diagnostics and handle schema", async (t) => {
  const h = debugHarness(t, app), result = await h.run({ timeoutMs: 10, networkWaitMs: 1 });
  assert.deepEqual(result, {
    operation: "debug", pageUrl: "https://fixture.invalid/debug", appPath: "/app/fixture", conversationId: "fixture-conversation",
    title: "Fixture debug page", bodyText: "fixture body", buttons: [], textboxes: [], matchingButtonHtml: [],
    media: [{ kind: "image", src: "fixture-image" }], apiKeys: [], snippets: {}, interestingKeys: {}, networkEvents: [], rpcCaptures: [],
  });
  assert.deepEqual(stages(h, "reset")[0].slice(2), ["https://gemini.google.com", 5000, "https://fixture.invalid/preferred"]);
  assert.deepEqual(stages(h, "wait"), [["wait", 1200]]);
  assert.ok(h.calls.findIndex(([s]) => s === "reset") < h.calls.findIndex(([s]) => s === "capture"));
  assertReleased(h);
});

test("Debug disabled capture and reset preserve unowned page listeners", async (t) => {
  const h = debugHarness(t, app);
  const result = await h.run({ captureNetwork: "false", resetConversation: false, clickOperation: "" });
  assert.deepEqual([stages(h, "capture"), stages(h, "reset"), stages(h, "click"), stages(h, "wait")], [[], [], [], []]);
  assert.deepEqual([result.networkEvents, result.rpcCaptures], [[], []]);
  assertReleased(h, 0);
});

test("Debug capture without mode selection keeps text traffic and avoids synthetic wait", async (t) => {
  const h = debugHarness(t, app);
  await h.run({ clickOperation: "", captureNetwork: "true", resetConversation: false });
  assert.equal(stages(h, "capture")[0][2], "text");
  assert.equal(stages(h, "wait").length, 0);
  assertReleased(h);
});

for (const failureAt of ["reset", "capture"]) {
  test(`Debug ${failureAt} failure before acquisition leaves inherited listeners untouched`, async (t) => {
    const h = debugHarness(t, app, { failureAt });
    await assert.rejects(h.run(), (error) => error === h.failure);
    assertReleased(h, 0);
  });
}

for (const failureAt of ["buttons", "content"]) {
  test(`Debug tolerated ${failureAt} failure still returns and releases capture`, async (t) => {
    const h = debugHarness(t, app, { failureAt });
    assert.equal((await h.run()).operation, "debug");
    assertReleased(h);
  });
}

for (const [failureAt, failureCall] of [["click", 1], ["log", 1], ["wait", 1], ["snapshot", 1], ["evaluate", 3], ["extract", 1], ["merge", 1], ["handle", 1]]) {
  test(`Debug regression releases real capture after ${failureAt} failure`, async (t) => {
    const h = debugHarness(t, app, { failureAt, failureCall });
    await assert.rejects(h.run(), (error) => error === h.failure);
    assertReleased(h);
  });
}
