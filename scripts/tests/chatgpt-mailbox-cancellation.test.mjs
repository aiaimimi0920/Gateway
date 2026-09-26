import assert from "node:assert/strict";
import { getEventListeners } from "node:events";
import test from "node:test";
import { fetchMailboxResponse } from "../chatgpt-web-session/mailbox-http.mjs";
import { waitForMailboxOpenAiCode } from "../chatgpt-web-session/mailbox-poll.mjs";

test("pre-canceled mailbox request performs no network call", async (t) => {
  const controller = new AbortController();
  const reason = new Error("synthetic canceled operation");
  controller.abort(reason);
  let calls = 0;
  t.mock.method(globalThis, "fetch", async () => { calls += 1; return Response.json({}); });
  await assert.rejects(fetchMailboxResponse("https://mailbox.example.test", { signal: controller.signal }), (error) => error === reason);
  assert.equal(calls, 0);
});

test("caller cancellation cancels an acquired mailbox reader and releases forwarding listener", async (t) => {
  const controller = new AbortController();
  const reason = new Error("synthetic canceled read");
  let canceled = false;
  let responseStarted;
  const started = new Promise((resolve) => { responseStarted = resolve; });
  t.mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
    pull() { responseStarted(); }, cancel() { canceled = true; },
  })));
  const pending = fetchMailboxResponse("https://mailbox.example.test", { signal: controller.signal });
  const rejected = assert.rejects(pending, (error) => error === reason);
  await started;
  controller.abort(reason);
  await rejected;
  assert.equal(canceled, true);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("successful mailbox requests remove their caller cancellation listener", async (t) => {
  const controller = new AbortController();
  t.mock.method(globalThis, "fetch", async () => Response.json({ ok: true }));
  await fetchMailboxResponse("https://mailbox.example.test", { signal: controller.signal });
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("aggregate mailbox deadline aborts its stalled first request and starts no fallback", { timeout: 10_000 }, async (t) => {
  let requests = 0;
  let canceled = false;
  t.mock.method(globalThis, "fetch", async () => {
    requests += 1;
    return new Response(new ReadableStream({ cancel() { canceled = true; } }));
  });
  await assert.rejects(waitForMailboxOpenAiCode({
    serviceBaseUrl: "https://mailbox.example.test", apiKey: "synthetic", mailboxSessionId: "session",
  }, { timeoutMs: 5000 }), (error) => error.code === "chatgpt_web_mailbox_code_timeout");
  assert.equal(requests, 1);
  assert.equal(canceled, true);
});

test("aggregate deadline reaches provider detail reads and prevents subsequent messages", { timeout: 10_000 }, async (t) => {
  const requests = [];
  let canceled = false;
  t.mock.method(globalThis, "fetch", async (url) => {
    const pathname = new URL(url).pathname;
    requests.push(pathname);
    if (pathname === "/mail/snapshot") return Response.json({ instances: [{
      id: "instance", metadata: { apiBase: "https://provider.example.test", apiKey: "AC-synthetic" },
    }] });
    if (pathname === "/messages") return Response.json([{ id: "first" }, { id: "second" }]);
    if (pathname === "/messages/first") return new Response(new ReadableStream({ cancel() { canceled = true; } }));
    return Response.json({});
  });
  await assert.rejects(waitForMailboxOpenAiCode({
    serviceBaseUrl: "https://mailbox.example.test", apiKey: "synthetic", mailboxSessionId: "session",
    mailboxProviderKey: "im215", mailboxProviderInstanceId: "instance", email: "synthetic@example.test",
  }, { timeoutMs: 5000 }), (error) => error.code === "chatgpt_web_mailbox_code_timeout");
  assert.ok(requests.includes("/messages/first"));
  assert.equal(requests.includes("/messages/second"), false);
  assert.equal(canceled, true);
});

test("caller abort terminates aggregate polling with its own reason and no fallback", async (t) => {
  const controller = new AbortController();
  const reason = new Error("synthetic canceled poll");
  let started;
  const ready = new Promise((resolve) => { started = resolve; });
  let requests = 0;
  t.mock.method(globalThis, "fetch", async () => {
    requests += 1;
    return new Response(new ReadableStream({ pull() { started(); } }));
  });
  const pending = waitForMailboxOpenAiCode({
    serviceBaseUrl: "https://mailbox.example.test", apiKey: "synthetic", mailboxSessionId: "session",
  }, { timeoutMs: 60_000, signal: controller.signal });
  const rejected = assert.rejects(pending, (error) => error === reason);
  await ready;
  controller.abort(reason);
  await rejected;
  assert.equal(requests, 1);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("a code arriving after the wall-clock budget is not accepted", async (t) => {
  let now = 1_000_000;
  t.mock.method(Date, "now", () => now);
  t.mock.method(globalThis, "fetch", async () => {
    now += 6000;
    return Response.json({ code: { code: "123456", messageId: 100 } });
  });
  await assert.rejects(waitForMailboxOpenAiCode({
    serviceBaseUrl: "https://mailbox.example.test", apiKey: "synthetic", mailboxSessionId: "session",
  }, { timeoutMs: 5000 }), (error) => error.code === "chatgpt_web_mailbox_code_timeout");
});

test("pre-canceled aggregate polling performs no fetch and retains cancellation reason", async (t) => {
  const controller = new AbortController();
  const reason = new Error("synthetic canceled before poll");
  controller.abort(reason);
  t.mock.method(globalThis, "fetch", async () => assert.fail("unexpected fetch"));
  await assert.rejects(waitForMailboxOpenAiCode({}, { timeoutMs: 5000, signal: controller.signal }), (error) => error === reason);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("mailbox timeout diagnostics never echo transport error secrets", async (t) => {
  let now = 1_000_000;
  let requests = 0;
  t.mock.method(Date, "now", () => now);
  t.mock.method(globalThis, "fetch", async () => {
    requests += 1;
    now += 6000;
    throw new Error("synthetic-private-key-and-mailbox-address");
  });
  await assert.rejects(waitForMailboxOpenAiCode({
    serviceBaseUrl: "https://mailbox.example.test", apiKey: "synthetic", mailboxSessionId: "session",
  }, { timeoutMs: 5000 }), (error) => {
    assert.equal(error.code, "chatgpt_web_mailbox_code_timeout");
    assert.equal(error.message.includes("synthetic-private"), false);
    return true;
  });
  assert.equal(requests, 1);
});
