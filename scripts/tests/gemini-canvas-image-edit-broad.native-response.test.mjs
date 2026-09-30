import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { createImageEditBroadCapture } from "../gemini-canvas-image-edit-broad-capture.mjs";
import { createProgramResponseCapture } from "../gemini-canvas-program-handle-response-capture.mjs";
import { NativeSession, deferred, turn } from "./gemini-canvas-program-handle.response-fixtures.mjs";
import { captureFixture } from "./gemini-canvas-image-edit-broad.fixtures.mjs";

async function fixture(t) {
  const native = new NativeSession(), cdp = new EventEmitter(), page = new EventEmitter();
  const controller = new AbortController();
  const capture = createImageEditBroadCapture({
    marker: "fixture-marker", outDir: "unused",
    createResponseCapture: (target, options) => createProgramResponseCapture(target, {
      ...options,
      createSessions: (_page, { configure }) => ({
        ready: configure(native, controller.signal), async stop() { controller.abort(); },
      }),
    }),
  });
  t.after(() => capture.stop());
  await capture.attach(cdp, page);
  const request = (id) => native.emit("Network.requestWillBeSent", {
    requestId: id, type: "XHR", request: { url: "https://gemini.google.com/same", method: "POST", headers: {} },
  });
  const response = (id, headers = {}) => native.emit("Network.responseReceived", {
    requestId: id, response: { url: "https://gemini.google.com/same", status: 201, headers: { "Content-Type": "application/json", ...headers } },
  });
  const data = (id, dataLength) => native.emit("Network.dataReceived", { requestId: id, dataLength });
  const finish = (id) => native.emit("Network.loadingFinished", { requestId: id });
  const reads = () => native.commands.filter(({ method }) => method === "Network.getResponseBody");
  return { native, cdp, page, capture, request, response, data, finish, reads };
}

test("broad native responses preserve same-URL request identity and unknown-length bodies", async (t) => {
  const f = await fixture(t);
  f.native.bodies.set("first", { body: "first", base64Encoded: false });
  f.native.bodies.set("second", { body: "second", base64Encoded: false });
  f.request("first"); f.request("second");
  f.response("second"); f.response("first");
  f.data("second", 6); f.finish("second"); f.data("first", 5); f.finish("first");
  await turn();
  assert.deepEqual(f.capture.events.map(event => [event.kind, event.status, event.resourceType, event.bodyText]), [
    ["response", 201, "xhr", "second"], ["response", 201, "xhr", "first"],
  ]);
  assert.equal(f.reads().length, 2);
  assert.equal(f.page.listenerCount("response"), 0);
});

test("broad native compressed overflow records failure without whole-body read", async (t) => {
  const f = await fixture(t);
  f.request("large"); f.response("large", { "Content-Length": "10", "Content-Encoding": "gzip" });
  f.data("large", 4 * 1024 * 1024 + 1); f.finish("large"); await turn();
  assert.equal(f.reads().length, 0);
  assert.deepEqual(f.capture.events.map(event => event.kind), ["capture-error"]);
  assert.equal(f.native.eventNames().length, 0);
});

test("broad native stop suppresses pending-body publication and detaches listeners", async (t) => {
  const f = await fixture(t), body = deferred();
  f.native.bodies.set("late", () => body.promise);
  f.request("late"); f.response("late"); f.data("late", 5); f.finish("late"); await turn();
  assert.equal(f.reads().length, 1);
  await f.capture.stop();
  body.resolve({ body: "small", base64Encoded: false }); await turn();
  assert.equal(f.capture.events.length, 0);
  assert.equal(f.native.eventNames().length + f.page.eventNames().length + f.cdp.eventNames().length, 0);
});

test("broad legacy CDP declaration alone never authorizes native body retrieval", async (t) => {
  const f = captureFixture(t, { send: async () => ({ body: "unexpected", base64Encoded: false }) });
  for (const length of ["0", "1", "4194304"]) {
    await f.emitCdp("Network.requestWillBeSent", { requestId: length, request: { url: "https://gemini.google.com/StreamGenerate" } });
    await f.emitCdp("Network.responseReceived", { requestId: length, response: { status: 200, headers: { "content-length": length } } });
    await f.emitCdp("Network.loadingFinished", { requestId: length });
  }
  assert.equal(f.calls.length, 0);
});
