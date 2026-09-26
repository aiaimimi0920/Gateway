import assert from "node:assert/strict";
import test from "node:test";
import { NativeSession, responseFixture } from "./gemini-canvas-program-handle.response-fixtures.mjs";

test("a parent response follows body events into an OOPIF without losing method or prior byte charges", async (t) => {
  const root = new NativeSession(), child = new NativeSession();
  child.parent = root;
  const f = await responseFixture(t, { bodyBytes: 5,
    createSessions: (_page, { configure }) => {
      const controllers = [new AbortController(), new AbortController()];
      const ready = Promise.all([configure(root, controllers[0].signal), configure(child, controllers[1].signal)]);
      return { ready, async stop() { controllers.forEach((controller) => controller.abort()); await ready; } };
    },
  });
  root.emit("Network.requestWillBeSent", { requestId: "navigation", request: { url: "https://fixture.invalid/app/frame", method: "POST" } });
  root.emit("Network.responseReceived", { requestId: "navigation", response: { url: "https://fixture.invalid/app/frame", status: 200, headers: {} } });
  root.emit("Network.dataReceived", { requestId: "navigation", dataLength: 2 });
  child.emit("Network.dataReceived", { requestId: "navigation", dataLength: 3 });
  child.emit("Network.loadingFinished", { requestId: "navigation" });
  assert.equal(await f.rows[0].promise, "small");
  assert.equal(f.rows[0].method, "POST");
  assert.equal(root.commands.filter((command) => command.method === "Network.getResponseBody").length, 0);
  assert.equal(child.commands.filter((command) => command.method === "Network.getResponseBody").length, 1);
  assert.deepEqual(f.failures, []);
});

test("parent-to-child migration cannot reset the decoded byte budget", async (t) => {
  const root = new NativeSession(), child = new NativeSession();
  child.parent = root;
  const f = await responseFixture(t, { bodyBytes: 5,
    createSessions: (_page, { configure }) => {
      const controllers = [new AbortController(), new AbortController()];
      const ready = Promise.all([configure(root, controllers[0].signal), configure(child, controllers[1].signal)]);
      return { ready, async stop() { controllers.forEach((controller) => controller.abort()); await ready; } };
    },
  });
  root.emit("Network.requestWillBeSent", { requestId: "navigation", request: { url: "https://fixture.invalid/app/frame", method: "GET" } });
  root.emit("Network.responseReceived", { requestId: "navigation", response: { url: "https://fixture.invalid/app/frame", status: 200, headers: {} } });
  root.emit("Network.dataReceived", { requestId: "navigation", dataLength: 3 });
  child.emit("Network.dataReceived", { requestId: "navigation", dataLength: 3 });
  child.emit("Network.loadingFinished", { requestId: "navigation" });
  assert.equal(await f.rows[0].promise, "");
  assert.equal(f.failures[0].code, "gateway_program_handle_capture_limit_exceeded");
  assert.equal([...root.commands, ...child.commands].filter((command) => command.method === "Network.getResponseBody").length, 0);
});
