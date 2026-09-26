import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { app, networkFixture, responseFixture, deferred, pairText } from "./gemini-canvas-browser-pool.network-fixtures.mjs";

test("failed network capture startup rolls back listeners and revokes an in-flight response", async (t) => {
  const previous = networkFixture(t);
  previous.capture.stop();
  const state = previous.state, snapshot = JSON.stringify(state);
  const page = new EventEmitter(), inherited = () => {};
  const text = deferred(), startupError = new Error("page registration failed");
  const subscribe = page.on.bind(page);
  let response;
  for (const event of ["request", "response", "websocket"]) subscribe(event, inherited);
  page.on = (event, listener) => {
    subscribe(event, listener);
    if (event === "response") response = listener(responseFixture({
      headers: { "content-length": String(Buffer.byteLength(pairText)) }, text: () => text.promise,
    }));
    if (event === "websocket") throw startupError;
    return page;
  };
  try {
    assert.throws(() => app.startNetworkCapture(page, "bootstrap_program", state), (error) => error === startupError);
    for (const event of ["request", "response", "websocket"]) assert.deepEqual(page.listeners(event), [inherited]);
  } finally {
    text.resolve(pairText);
    await response;
  }
  assert.equal(JSON.stringify(state), snapshot);
});

test("network stop attempts every owned detachment and preserves the first cleanup error", (t) => {
  const page = new EventEmitter(), inherited = () => {};
  for (const event of ["request", "response", "websocket"]) page.on(event, inherited);
  const { capture } = networkFixture(t, "video", { page });
  const detach = page.off.bind(page), cleanupError = new Error("request detachment failed");
  page.off = (event, listener) => {
    detach(event, listener);
    if (event === "request") throw cleanupError;
    if (event === "response") throw new Error("response detachment also failed");
    return page;
  };
  try {
    assert.throws(() => capture.stop(), (error) => error === cleanupError);
    for (const event of ["request", "response", "websocket"]) assert.deepEqual(page.listeners(event), [inherited]);
  } finally {
    page.off = detach;
    capture.stop();
  }
});
