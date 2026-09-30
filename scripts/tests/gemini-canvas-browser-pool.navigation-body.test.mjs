import assert from "node:assert/strict";
import test from "node:test";
import { createNavigationBodyCapture, readNavigationDownload } from "../gemini-canvas-browser-pool-navigation-body.mjs";
import { createProgramResponseCapture } from "../gemini-canvas-program-handle-response-capture.mjs";
import { NativeSession, turn } from "./gemini-canvas-program-handle.response-fixtures.mjs";
import { downloadFile } from "./gemini-canvas-browser-pool.payload-fixtures.mjs";

async function fixture(t, timeout = 1000) {
  const session = new NativeSession();
  const send = session.send.bind(session);
  session.send = (method, params) => method === "Page.getFrameTree"
    ? Promise.resolve({ frameTree: { frame: { id: "main" } } }) : send(method, params);
  const controller = new AbortController();
  const capture = createNavigationBodyCapture({}, timeout, (page, options) => createProgramResponseCapture(page, {
    ...options,
    createSessions: (_page, { configure }) => ({
      ready: configure(session, controller.signal),
      async stop() { controller.abort(); },
    }),
  }));
  t.after(() => capture.stop());
  await capture.ready;
  const request = (id = "one", frameId = "main", type = "Document", redirectResponse) => session.emit("Network.requestWillBeSent", {
    requestId: id, frameId, type, redirectResponse, request: { method: "GET", url: "https://fixture.invalid/same" },
  });
  const response = (id = "one", frameId = "main", type = "Document", headers = {}, status = 200) => session.emit("Network.responseReceived", {
    requestId: id, frameId, type, response: { url: "https://fixture.invalid/same", status, headers },
  });
  const data = (dataLength, id = "one") => session.emit("Network.dataReceived", { requestId: id, dataLength });
  const finish = (id = "one") => session.emit("Network.loadingFinished", { requestId: id });
  const reads = () => session.commands.filter(({ method }) => method === "Network.getResponseBody");
  return { session, capture, request, response, data, finish, reads };
}

test("navigation native admission ignores same-URL subframes and subresources", async (t) => {
  const f = await fixture(t);
  f.request("child", "child"); f.response("child", "child"); f.data(100, "child"); f.finish("child");
  f.request("fetch", "main", "Fetch"); f.response("fetch", "main", "Fetch"); f.data(100, "fetch"); f.finish("fetch");
  f.request(); f.response(); f.data(5); f.finish();
  assert.equal((await f.capture.result).toString(), "small");
  assert.deepEqual(f.reads().map(({ params }) => params.requestId), ["one"]);
});

test("navigation redirect selects final native request without URL matching", async (t) => {
  const f = await fixture(t);
  f.request(); f.request("one", "main", "Document", { status: 302, url: "https://fixture.invalid/same", headers: {} });
  f.response(); f.data(5); f.finish();
  assert.equal((await f.capture.result).toString(), "small");
  assert.equal(f.reads().length, 1);
});

for (const status of [300, 302]) {
  test(`navigation terminal ${status} without redirect retains its response body`, async (t) => {
    const f = await fixture(t);
    f.request(); f.response("one", "main", "Document", { "content-type": "text/plain" }, status);
    f.data(5); f.finish();
    assert.equal((await f.capture.result).toString(), "small");
    assert.equal(f.reads().length, 1);
  });
}

test("navigation compressed decoded overflow rejects before native read", async (t) => {
  const f = await fixture(t);
  f.request(); f.response("one", "main", "Document", { "content-type": "text/plain", "content-length": "10", "content-encoding": "gzip" });
  f.data(4 * 1024 * 1024 + 1); f.finish();
  await assert.rejects(f.capture.result, /decoded body limit/);
  assert.equal(f.reads().length, 0);
});

test("navigation native failure cannot become a successful empty body", async (t) => {
  const f = await fixture(t);
  f.session.bodies.set("one", () => { throw new Error("protocol unavailable"); });
  f.request(); f.response(); f.data(5); f.finish();
  await assert.rejects(f.capture.result, /could not read/);
});

test("navigation non-text document retains its binary budget without MIME metadata", async (t) => {
  const f = await fixture(t), body = Buffer.alloc(4 * 1024 * 1024 + 1, 255);
  f.session.bodies.set("one", { body: body.toString("base64"), base64Encoded: true });
  f.request(); f.response(); f.data(body.length); f.finish();
  assert.deepEqual(await f.capture.result, body);
});

test("navigation body loading failure rejects and detaches listeners", async (t) => {
  const f = await fixture(t);
  f.request(); f.response();
  f.session.emit("Network.loadingFailed", { requestId: "one" });
  await assert.rejects(f.capture.result, /loading failed/);
  assert.equal(f.session.eventNames().length, 0);
});

test("navigation stop settles a pending body and prevents late native reads", async (t) => {
  const f = await fixture(t);
  f.request(); f.response();
  await f.capture.stop();
  await assert.rejects(f.capture.result, /stopped/);
  f.data(5); f.finish(); await turn();
  assert.equal(f.reads().length, 0);
  assert.equal(f.session.eventNames().length, 0);
});

test("navigation timeout disposes owner and rejects the result", async (t) => {
  const f = await fixture(t, 10);
  await assert.rejects(f.capture.result, { code: "gemini_canvas_navigation_body_timeout" });
  assert.equal(f.session.eventNames().length, 0);
});

test("navigation download admits exact limit and reads only one overflow byte", async (t) => {
  const limit = 16 * 1024 * 1024;
  const exact = await downloadFile(t, Buffer.alloc(limit, 7));
  assert.equal((await readNavigationDownload(exact)).length, limit);
  const oversized = await downloadFile(t, Buffer.alloc(limit + 1024 * 1024, 7));
  await assert.rejects(readNavigationDownload(oversized), { code: "gemini_canvas_browser_body_too_large", actual: limit + 1 });
});
