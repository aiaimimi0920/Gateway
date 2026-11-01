import assert from "node:assert/strict";
import { test } from "node:test";
import { createTransport } from "../aistudio-http/transport.mjs";

test("HTTP transport rejects foreign targets before a network request", async () => {
  const send = createTransport();
  for (const address of ["https://evil.example/", "http://aistudio.google.com/", "https://user:password@aistudio.google.com/"]) {
    await assert.rejects(send("bootstrap", address, {}), /destination_rejected/);
  }
});

test("decoded byte admission cancels an oversized response", async () => {
  const original = globalThis.fetch;
  let cancelled = false, calls = 0;
  globalThis.fetch = async (_address, options) => {
    calls++;
    assert.equal(options.redirect, "manual");
    assert.ok(options.signal instanceof AbortSignal);
    return { status: 200, body: new ReadableStream({
      start(controller) { controller.enqueue(new Uint8Array(20)); },
      cancel() { cancelled = true; },
    }) };
  };
  try {
    await assert.rejects(createTransport()("bootstrap", "https://aistudio.google.com/", {}, 10), /body_limit/);
    assert.equal(calls, 1);
    assert.equal(cancelled, true);
  } finally { globalThis.fetch = original; }
});

test("HTTP failures are terminal classified codes, never raw upstream details", async () => {
  const original = globalThis.fetch;
  try {
    for (const status of [302, 401, 403, 429, 500]) {
      let calls = 0;
      globalThis.fetch = async () => { calls++; return new Response("private-upstream-body", { status }); };
      await assert.rejects(createTransport()("generate", "https://aistudio.google.com/", {}), error => {
        assert.equal(error.status, status);
        assert.match(error.message, /^aistudio_http_[a-z_]+$/);
        assert.ok(!error.message.includes("private"));
        return true;
      });
      assert.equal(calls, 1);
    }
  } finally { globalThis.fetch = original; }
});

test("expired Google login is classified without following its redirect", async () => {
  const original = globalThis.fetch;
  let calls = 0;
  globalThis.fetch = async () => {
    calls++;
    return new Response("private-signin-body", { status: 302,
      headers: { Location: "https://accounts.google.com/ServiceLogin?continue=private-value" } });
  };
  try {
    await assert.rejects(createTransport()("bootstrap", "https://aistudio.google.com/prompts/new_chat", {}),
      error => error.status === 401 && error.message === "aistudio_http_login_required");
    assert.equal(calls, 1);
  } finally { globalThis.fetch = original; }
});

test("Drive OAuth consent failure is not confused with an expired Google login", async () => {
  const original = globalThis.fetch;
  try {
    globalThis.fetch = async () => new Response('[16,"OAuth error: unauthorized_client."]', { status: 401 });
    await assert.rejects(createTransport()("attachment_token", "https://alkalimakersuite-pa.clients6.google.com/", {}),
      error => error.message === "aistudio_http_drive_authorization_required" && error.status === 401);
    await assert.rejects(createTransport()("generate", "https://alkalimakersuite-pa.clients6.google.com/", {}), /login_required/);
  } finally { globalThis.fetch = original; }
});
