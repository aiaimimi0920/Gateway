import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import test from "node:test";
import { captureFixture, requestFixture, responseFixture } from "./gemini-canvas-image-edit-broad.fixtures.mjs";

const pageEvents = ["request", "response", "websocket"];
const cdpEvents = ["Network.requestWillBeSent", "Network.responseReceived", "Network.dataReceived", "Network.loadingFailed", "Network.loadingFinished"];
const socketEvents = ["framesent", "framereceived", "close"];
const uploadUrl = "https://push.clients6.google.com/upload/fixture";
const headers = { "x-goog-upload-command": "upload, finalize" };
const state = (f) => structuredClone(Object.fromEntries(Object.entries(f.capture).filter(([name]) => !["attach", "stop"].includes(name))));

for (const boundary of ["request headers", "response headers", "request body", "CDP finalize", "CDP response", "response text"]) {
  for (const reject of [false, true]) {
    test("broad stopped capture ignores late " + boundary + " " + (reject ? "rejection" : "completion"), async (t) => {
      const gate = Promise.withResolvers(), entered = Promise.withResolvers();
      const hold = () => { entered.resolve(); return gate.promise; };
      const f = captureFixture(t, { send: async (command) => {
        if (boundary === "CDP response" || command === "Network.getRequestPostData") return hold();
        return { body: "fixture-marker", base64Encoded: false };
      } });
      let pending, value, headerReads = 0;
      if (boundary === "request headers") {
        value = {};
        pending = f.emit("request", requestFixture({ data: "fixture-marker", overrides: { headers: () => { headerReads += 1; return {}; }, allHeaders: hold } }));
      } else if (boundary === "response headers") {
        value = { "content-type": "text/plain" };
        pending = f.emit("response", responseFixture(undefined, { headers: () => { headerReads += 1; return {}; }, allHeaders: hold }));
      } else if (boundary === "request body") {
        value = Buffer.from([0, 255, 128]);
        pending = f.emit("request", requestFixture({ url: uploadUrl, headers, overrides: { postDataBuffer: hold } }));
      } else if (boundary.startsWith("CDP")) {
        const url = boundary === "CDP finalize" ? uploadUrl : "https://gemini.google.com/StreamGenerate";
        await f.emitCdp("Network.requestWillBeSent", { requestId: "late", request: { url, method: "POST", headers } });
        await f.emitCdp("Network.responseReceived", { requestId: "late", response: { status: 200, headers: { "content-length": "14" } } });
        await f.emitCdp("Network.dataReceived", { requestId: "late", dataLength: 14 });
        value = boundary === "CDP finalize" ? { postData: "\x00\xff\x80" } : { body: "fixture-marker", base64Encoded: false };
        pending = f.emitCdp("Network.loadingFinished", { requestId: "late" });
      } else {
        value = "fixture-marker";
        pending = f.emit("response", responseFixture(undefined, { text: hold }));
      }
      await entered.promise;
      f.capture.stop?.();
      const stopped = state(f), files = fs.readdirSync(f.outDir), calls = f.calls.length;
      if (reject) gate.reject(new Error("fixture stopped read")); else gate.resolve(value);
      await pending;
      assert.deepEqual(state(f), stopped);
      assert.deepEqual(fs.readdirSync(f.outDir), files);
      assert.equal(f.calls.length, calls);
      if (boundary.endsWith("headers")) assert.equal(headerReads, 1);
    });
  }
}

test("broad capture stop removes only owned page CDP and socket listeners once", async (t) => {
  const f = captureFixture(t), socket = new EventEmitter(), inherited = () => {};
  socket.url = () => "wss://fixture.invalid/socket";
  for (const [emitter, events] of [[f.page, pageEvents], [f.cdp, cdpEvents], [socket, socketEvents]]) {
    for (const event of events) emitter.on(event, inherited);
  }
  await f.emit("websocket", socket);
  let removals = 0;
  for (const emitter of [f.page, f.cdp, socket]) {
    const off = emitter.off;
    emitter.off = function (...args) { removals += 1; return off.apply(this, args); };
  }
  f.capture.stop?.();
  f.capture.stop?.();
  for (const [emitter, events] of [[f.page, pageEvents], [f.cdp, cdpEvents], [socket, socketEvents]]) {
    for (const event of events) assert.deepEqual(emitter.listeners(event), [inherited]);
  }
  assert.equal(removals, 11);
});

test("broad stopped capture ignores all queued handlers before payload access", async (t) => {
  const f = captureFixture(t), socket = new EventEmitter();
  socket.url = () => "wss://fixture.invalid/socket";
  await f.emit("websocket", socket);
  const queued = [[f.page, pageEvents], [f.cdp, cdpEvents], [socket, socketEvents]].flatMap(([emitter, events]) => events.flatMap((event) => emitter.listeners(event)));
  assert.equal(queued.length, 11);
  f.capture.stop?.();
  const stopped = state(f);
  const stale = new Proxy({}, { get() { throw new Error("stale payload accessed"); } });
  for (const listener of queued) await listener(stale);
  assert.deepEqual(state(f), stopped);
});

test("broad capture stop continues detach after one emitter rejects cleanup", async (t) => {
  const f = captureFixture(t);
  let attempts = 0;
  f.page.off = () => { attempts += 1; throw new Error("fixture page detach failure"); };
  const queued = f.page.listeners("request")[0];
  f.capture.stop?.();
  assert.equal(attempts, 3);
  for (const event of cdpEvents) assert.equal(f.cdp.listenerCount(event), 0);
  const stopped = state(f);
  await queued({ url() { throw new Error("stale request accessed"); } });
  assert.deepEqual(state(f), stopped);
});

test("broad stopped capture cannot attach new listeners", (t) => {
  const f = captureFixture(t);
  f.capture.stop?.();
  const closed = { on() { throw new Error("listener added after stop"); } };
  f.capture.attach(closed, closed);
});
