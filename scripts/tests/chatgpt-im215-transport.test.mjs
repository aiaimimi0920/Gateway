import assert from "node:assert/strict";
import test from "node:test";
import { createServer } from "node:http";
import { im215Request } from "../chatgpt-web-session/mailbox-im215.mjs";

const config = { baseUrl: "https://mailbox.example.test/v1", apiKey: "AC-synthetic" };

test("IM215 transport rejects oversized streamed data and cancels the body", async (t) => {
  let canceled = false;
  t.mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
    pull(controller) { controller.enqueue(new Uint8Array(1024 * 1024)); },
    cancel() { canceled = true; },
  })));
  await assert.rejects(im215Request(config, "GET", "/messages"), /im215_response_too_large/);
  assert.equal(canceled, true);
});

test("IM215 requests carry an abort signal and refuse redirect forwarding", async (t) => {
  let options;
  t.mock.method(globalThis, "fetch", async (_url, input) => { options = input; return Response.json({ messages: [] }); });
  assert.deepEqual(await im215Request(config, "GET", "/messages"), { status: 200, data: { messages: [] } });
  assert.ok(options.signal instanceof AbortSignal);
  assert.equal(options.redirect, "error");
});

test("IM215 stream failures propagate instead of masquerading as an empty mailbox", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
    start(controller) { controller.error(new Error("synthetic stream failure")); },
  })));
  await assert.rejects(im215Request(config, "GET", "/messages"), /synthetic stream failure/);
});

test("IM215 accepts exact-limit plain text and split UTF8 JSON", async (t) => {
  const mock = t.mock.method(globalThis, "fetch", async () => new Response("x".repeat(4 * 1024 * 1024)));
  assert.equal((await im215Request(config, "GET", "/messages")).data.length, 4 * 1024 * 1024);
  const bytes = new TextEncoder().encode('{"subject":"\u4f60\u597d"}');
  let offset = 0;
  mock.mock.mockImplementation(async () => new Response(new ReadableStream({
    pull(controller) {
      if (offset === bytes.length) controller.close();
      else controller.enqueue(bytes.slice(offset, ++offset));
    },
  })));
  assert.deepEqual((await im215Request(config, "GET", "/messages")).data, { subject: "\u4f60\u597d" });
});

async function localServer(t, handler) {
  const server = createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(async () => {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  });
  return { ...config, baseUrl: `http://127.0.0.1:${server.address().port}` };
}

test("native fetch deadline aborts a stalled response body", { timeout: 25_000 }, async (t) => {
  let closed = false;
  const nativeFetch = globalThis.fetch;
  t.mock.method(globalThis, "fetch", async (url, options) => {
    options.signal.addEventListener("abort", () => t.diagnostic("request signal aborted"), { once: true });
    const response = await nativeFetch(url, options);
    t.diagnostic("response headers received");
    return response;
  });
  const local = await localServer(t, (_request, response) => {
    response.on("close", () => { closed = true; t.diagnostic("response socket closed"); });
    response.writeHead(200, { "content-type": "application/json" });
    response.write('{"messages":[');
  });
  await assert.rejects(im215Request(local, "GET", "/messages"), (error) => error.name === "AbortError");
  for (let index = 0; index < 50 && !closed; index += 1) {
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.equal(closed, true);
});

test("native fetch does not follow mailbox redirects with API credentials", async (t) => {
  let redirected = false;
  const local = await localServer(t, (request, response) => {
    if (request.url === "/messages") response.writeHead(302, { location: "/target" }).end();
    else { redirected = true; response.end("unexpected"); }
  });
  await assert.rejects(im215Request(local, "GET", "/messages"));
  assert.equal(redirected, false);
});
