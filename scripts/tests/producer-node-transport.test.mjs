import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import test, { mock } from "node:test";
import { nodeSideFetch } from "../producer-browser/transport.mjs";

test("Producer Node transport does not accept a failed response body or expose its error text", async () => {
  const fetch = mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
    start(controller) { controller.error(new Error("CANARY_TRANSPORT_ERROR")); },
  })));
  try {
    const result = await nodeSideFetch("https://producer.invalid/status");
    assert.equal(result.ok, false);
    assert.equal(result.fetchError, "producer_node_fetch_failed");
    assert.equal(JSON.stringify(result).includes("CANARY_TRANSPORT_ERROR"), false);
  } finally { fetch.mock.restore(); }
});

test("Producer Node transport rejects response bodies above 16 MiB and cancels the reader", async () => {
  let cancelled = false;
  const fetch = mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
    start(controller) { controller.enqueue(new Uint8Array(16 * 1024 * 1024 + 1)); },
    cancel() { cancelled = true; },
  })));
  try {
    const result = await nodeSideFetch("https://producer.invalid/status");
    assert.equal(result.ok, false);
    assert.equal(result.fetchError, "producer_node_response_too_large");
    assert.equal(cancelled, true);
  } finally { fetch.mock.restore(); }
});

test("Producer Node transport refuses credential-bearing redirects", async () => {
  let redirect;
  const fetch = mock.method(globalThis, "fetch", async (_url, options) => {
    redirect = options.redirect;
    return new Response("fixture");
  });
  try {
    const result = await nodeSideFetch("https://producer.invalid/status", { headers: { authorization: "Bearer synthetic" } });
    assert.equal(result.ok, true);
    assert.equal(redirect, "error");
  } finally { fetch.mock.restore(); }
});

test("Producer Node transport accepts an exact-limit body and split UTF8", async () => {
  const payloads = [Buffer.alloc(16 * 1024 * 1024, 32), Buffer.from("\u4f60\u597d")];
  for (const payload of payloads) {
    const fetch = mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
      start(controller) {
        controller.enqueue(payload.subarray(0, 1));
        controller.enqueue(payload.subarray(1));
        controller.close();
      },
    })));
    try {
      const result = await nodeSideFetch("https://producer.invalid/status");
      assert.equal(result.ok, true);
      assert.equal(result.text, payload.toString("utf8"));
    } finally { fetch.mock.restore(); }
  }
});

test("Producer native Node fetch aborts a stalled body after headers and closes its socket", { timeout: 5000 }, async () => {
  let closeSocket;
  const closed = new Promise((resolve) => { closeSocket = resolve; });
  const server = createServer((_request, response) => {
    response.writeHead(200, { "content-type": "application/json" });
    response.write("{");
  });
  server.on("connection", (socket) => socket.once("close", closeSocket));
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    const result = await nodeSideFetch(`http://127.0.0.1:${server.address().port}/status`, {}, 250);
    assert.equal(result.ok, false);
    assert.equal(result.status, 504);
    assert.equal(result.fetchError, "producer_node_fetch_timeout");
    await closed;
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});

test("Producer native Node fetch never visits a credential redirect target", async () => {
  let redirected = 0;
  const server = createServer((request, response) => {
    if (request.url === "/target") {
      redirected++;
      response.end("unexpected target");
    } else {
      response.writeHead(302, { location: "/target" });
      response.end();
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    const result = await nodeSideFetch(`http://127.0.0.1:${server.address().port}/status`, {
      headers: { authorization: "Bearer synthetic", cookie: "fixture=value" },
      redirect: "follow",
    });
    assert.equal(result.ok, false);
    assert.equal(result.fetchError, "producer_node_fetch_failed");
    assert.equal(redirected, 0);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});
