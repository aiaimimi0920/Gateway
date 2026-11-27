// Real loopback transport with synthetic output, never evidence of Cursor upstream.
import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import { once } from "node:events";
import { createRequire } from "node:module";
import { httpTurn, websocketClient } from "../../tools/cursor_remote/protocol_client.mjs";
const require = createRequire(import.meta.url);
const { WebSocketServer } = require("ws");

async function server(handler, run) {
  const service = http.createServer(handler);
  service.listen(0, "127.0.0.1");
  await once(service, "listening");
  try { await run(`http://127.0.0.1:${service.address().port}`, service); }
  finally { service.closeAllConnections(); await new Promise((resolve) => service.close(resolve)); }
}
test("HTTP JSON accepts opaque ID and sends one authenticated request only", async () => {
  let calls = 0;
  await server(async (request, response) => {
    calls++;
    assert.equal(request.headers.authorization, "Bearer fixture");
    assert.equal(request.url, "/v1/responses");
    for await (const _ of request) {}
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ id: "relay-fixture", object: "response", status: "completed",
      output: [{ type: "message", role: "assistant", content: [{ type: "output_text", text: "ok" }] }] }));
  }, async (base) => assert.equal((await httpTurn(base, "fixture", false, { input: "hello" })).text, "ok"));
  assert.equal(calls, 1);
});
test("HTTP errors are classified without exposing server body or retrying", async () => {
  let calls = 0;
  await server((_, response) => {
    calls++;
    response.writeHead(400, { "content-type": "application/json" });
    response.end(JSON.stringify({ error: { code: "cursor_remote_unsupported_request", message: "private-body" } }));
  }, async (base) => {
    assert.deepEqual(await httpTurn(base, "fixture", false, {}), {
      rejected: true, status: 400, code: "cursor_remote_unsupported_request",
    });
  });
  assert.equal(calls, 1);
});
test("Cursor JSON rejects unproven OpenAI cached-token projections, including zero", async () => {
  for (const chat of [false, true]) {
    for (const cached of [0, 20]) {
      await server((_, response) => {
        response.writeHead(200, { "content-type": "application/json" });
        response.end(JSON.stringify({ usage: {
          [chat ? "prompt_tokens_details" : "input_tokens_details"]: { cached_tokens: cached },
        } }));
      }, async (base) => assert.rejects(httpTurn(base, "fixture", chat, {}), /cursor_unproven_cache_details/));
    }
  }
});
test("Chat SSE requires terminal DONE rather than accepting a stop chunk alone", async () => {
  await server((_, response) => {
    response.writeHead(200, { "content-type": "text/event-stream" });
    response.end('data: {"id":"c","object":"chat.completion.chunk","choices":[{"delta":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}\n\n');
  }, async (base) => assert.rejects(httpTurn(base, "fixture", true, { stream: true }), /chat_missing_done/));
});
test("WebSocket rejects upstream failures, distinguishes the exact stale-pause error", async () => {
  await server((_, response) => response.end(), async (base, service) => {
    const ws = new WebSocketServer({ server: service });
    let turns = 0;
    ws.on("connection", (socket, request) => {
      assert.equal(request.headers.authorization, "Bearer fixture");
      socket.on("message", () => {
        turns++;
        socket.send(JSON.stringify(turns === 1 ?
          { type: "error", error: { type: "invalid_request_error",
            message: "Cursor previous_response_id requires the current delivered tool pause on this WebSocket" } } :
          { type: "response.failed", error: { message: "private failure" } }));
      });
    });
    const client = await websocketClient(base, "fixture");
    try {
      assert.equal((await client.turn({})).code, "cursor_remote_previous_response_unavailable");
      await assert.rejects(client.turn({}), /websocket_upstream_terminal_failure/);
    } finally {
      await client.close().catch(() => {});
      await new Promise((resolve) => ws.close(resolve));
    }
    assert.equal(turns, 2);
  });
});
