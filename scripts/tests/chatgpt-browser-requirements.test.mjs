import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { runChatGptBrowserRelay } from "../chatgpt-web-session/http-relay.mjs";

const root = "https://chatgpt.com";
const prepare = "/backend-api/sentinel/chat-requirements/prepare";
const finalize = "/backend-api/sentinel/chat-requirements/finalize";
function fixture(replies) {
  const requests = [];
  const page = { content: async () => "", evaluate: async (callback, input) => vm.runInNewContext(`(${callback})(input)`, {
    input, AbortSignal, TextDecoder, Uint8Array,
    fetch: async (url, options) => {
      requests.push({ path: new URL(url).pathname, ...options });
      const reply = replies[requests.length - 1];
      assert.ok(reply, "unexpected request after a failed phase");
      return reply instanceof Response ? reply : Response.json(reply);
    },
  }) };
  const input = { baseUrl: root, accessToken: "synthetic", requestBody: { model: "fixture-model", messages: [] } };
  return { requests, execute: (extra = {}) => runChatGptBrowserRelay(page, { ...input, ...extra }) };
}

test("browser relay defaults to prepare/finalize and uses only the final token for generation", async () => {
  const value = fixture([{ prepare_token: "prepared" }, { token: "final", so_token: "final-so" }, new Response("data: fixture\n\n", { headers: { "content-type": "text/event-stream" } })]);
  const result = await value.execute();
  assert.equal(result.status, 200);
  assert.deepEqual(value.requests.map((item) => item.path), [prepare, finalize, "/backend-api/conversation"]);
  assert.deepEqual(JSON.parse(value.requests[1].body), { prepare_token: "prepared", proof_token: "", turnstile_token: "" });
  for (const request of value.requests) {
    assert.equal(request.headers.Authorization, "Bearer synthetic");
    assert.equal(request.headers["X-OpenAI-Target-Path"], request.path);
    assert.equal(request.redirect, "error");
    assert.equal(request.credentials, "same-origin");
  }
  assert.equal(value.requests[2].headers["OpenAI-Sentinel-Chat-Requirements-Token"], "final");
  assert.equal(value.requests[2].headers["OpenAI-Sentinel-SO-Token"], "final-so");
  assert.ok(!JSON.stringify(result).includes("final-so"), "handshake secrets must not enter previews");
});

for (const phase of ["prepare", "finalize"]) for (const status of [302, 401, 403, 503]) {
  test(`browser ${phase} HTTP ${status} stops before generation without UI fallback`, async () => {
    const rejected = new Response("private-upstream-marker", { status, headers: { "content-type": "text/html" } });
    const value = fixture(phase === "prepare" ? [rejected] : [{ prepare_token: "prepared" }, rejected]);
    await assert.rejects(value.execute(), (error) => {
      assert.match(error.message, new RegExp(`requirements_${phase}.*${status}`));
      assert.ok(!error.message.includes("private-upstream-marker"));
      return true;
    });
    assert.equal(value.requests.length, phase === "prepare" ? 1 : 2);
  });
}

test("unknown browser requirements mode fails without sending a request", async () => {
  const value = fixture([]);
  await assert.rejects(value.execute({ requirementsMode: "unknown" }), /mode/);
  assert.equal(value.requests.length, 0);
});

for (const [label, reply] of [
  ["missing prepare token", {}], ["empty prepare token", { prepare_token: " " }],
  ["oversized token", { prepare_token: "a".repeat(32769) }],
  ["multibyte oversized token", { prepare_token: "字".repeat(12000) }],
  ["control character token", { prepare_token: "private\nmarker" }],
  ["array instead of object", []],
  ["non-JSON response", new Response("private-marker", { headers: { "content-type": "text/html" } })],
  ["invalid JSON", new Response("private-marker", { headers: { "content-type": "application/json" } })],
  ["oversized response", new Response(" ".repeat(1024 * 1024 + 1), { headers: { "content-type": "application/json" } })],
  ["Arkose required", { prepare_token: "prepared", arkose: { required: true } }],
  ["missing Turnstile material", { prepare_token: "prepared", turnstile: { required: true } }],
]) test(`browser requirements reject ${label} without another request`, async () => {
  const value = fixture([reply]);
  await assert.rejects(value.execute(), (error) => !error.message.includes("private-marker"));
  assert.equal(value.requests.length, 1);
});

test("missing final token stops before conversation and optional empty SO remains compatible", async () => {
  const invalid = fixture([{ prepare_token: "prepared" }, { so_token: "private-marker" }]);
  await assert.rejects(invalid.execute(), /requirements_finalize/);
  assert.equal(invalid.requests.length, 2);
  const valid = fixture([{ prepare_token: "prepared" }, { token: "final", so_token: null }, new Response("data: fixture\n\n")]);
  assert.equal((await valid.execute()).status, 200);
  assert.equal(valid.requests[2].headers["OpenAI-Sentinel-SO-Token"], undefined);
});
