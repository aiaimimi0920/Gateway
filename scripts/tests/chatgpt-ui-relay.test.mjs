import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import vm from "node:vm";
import { collectChatGptAssistantTexts, extractRelayPrompt, runChatGptBrowserUiRelay } from "../chatgpt-web-session/ui-relay.mjs";
import { createChatGptUiRequestCapture, finalizeChatGptUiRelayCapture, redactCapturedRequestHeaders } from "../chatgpt-web-session/ui-capture.mjs";
import { navigateWithChallengeSettle } from "../chatgpt-web-session/navigation.mjs";

test("UI prompt selects the first user message and skips non-text parts", () => {
  assert.equal(extractRelayPrompt({ messages: [
    { author: { role: "assistant" }, content: { parts: ["old"] } },
    { author: { role: "user" }, content: { parts: [" hello ", {}, "world"] } },
  ] }), "hello\nworld");
  assert.equal(extractRelayPrompt({ messages: [] }), null);
});

test("missing UI prompt returns the existing failure without touching browser", async () => {
  const response = await runChatGptBrowserUiRelay(null, { requestBody: {} });
  assert.equal(response.status, 500);
  assert.deepEqual(JSON.parse(response.bodyText), { detail: "ui relay missing prompt" });
});

test("assistant collection runs its serialized callback and deduplicates selector overlap", async () => {
  const texts = await collectChatGptAssistantTexts({ evaluate: async (callback) => vm.runInNewContext(`(${callback})()`, {
    document: { querySelectorAll: () => [{ innerText: " first " }, { textContent: "second" }, { innerText: "" }] },
  }) });
  assert.deepEqual(Array.from(texts), ["first", "second"]);
});

test("settled navigation keeps timeout clamp and avoids unnecessary reload", async () => {
  const calls = [];
  await navigateWithChallengeSettle({
    goto: async (url, options) => { calls.push({ url, options }); },
    waitForTimeout: async (delay) => { calls.push(delay); },
    content: async () => "plain page", url: () => "https://chatgpt.com/",
  }, "https://chatgpt.com/", 90_000);
  assert.deepEqual(calls, [{ url: "https://chatgpt.com/", options: { waitUntil: "domcontentloaded", timeout: 60_000 } }, 2500]);
});

test("UI diagnostic capture pairs responses and detaches both event listeners", async (t) => {
  const variables = ["RAW_HEADERS", "RAW_REQUESTS", "RAW_RESPONSES", "ALL_POSTS", "PATH"].map((name) => `CHATGPT_WEB_UI_REQUEST_CAPTURE_${name}`);
  for (const name of variables) {
    const previous = process.env[name];
    delete process.env[name];
    t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  }
  const page = new EventEmitter();
  const capture = createChatGptUiRequestCapture(page);
  const request = {
    url: () => "https://chatgpt.com/backend-api/f/conversation",
    method: () => "POST", postData: () => '{"fixture":true}',
    headers: () => ({ authorization: "synthetic", cookie: "session=synthetic" }),
  };
  page.emit("request", request);
  await capture.onResponse({ request: () => request, headers: () => ({}), text: async () => "fixture", status: () => 200 });
  const result = await finalizeChatGptUiRelayCapture(capture, { status: 200 });
  assert.equal(page.listenerCount("request"), 0);
  assert.equal(page.listenerCount("response"), 0);
  assert.equal(result.uiRequestCapture.requests.length, 1);
  assert.equal(result.uiRequestCapture.droppedRequests, 0);
  assert.equal(result.uiRequestCapture.requests[0].headers.authorization.redacted, true);
  assert.equal(result.uiRequestCapture.requests[0].response.bodyJson, null);
  assert.equal(result.uiRequestCapture.requests[0].response.bodyLength, null);
});

test("existing diagnostic header contract redacts credential categories", () => {
  const headers = redactCapturedRequestHeaders({ Authorization: "synthetic", "X-Conduit-Token": "synthetic", "Content-Type": "application/json" });
  assert.equal(headers.Authorization.kind, "authorization");
  assert.equal(headers["X-Conduit-Token"].kind, "token");
  assert.equal(headers["Content-Type"], "application/json");
});

test("UI submission failure detaches diagnostic listeners before propagating", async () => {
  const page = Object.assign(new EventEmitter(), {
    bringToFront: async () => {}, waitForLoadState: async () => {},
    waitForTimeout: async () => {}, keyboard: { press: async () => {} },
    locator: () => ({ first: () => ({ count: async () => 0 }) }),
    evaluate: async (callback) => String(callback).includes("const selectors") ? [] : {},
    waitForFunction: async () => { throw new Error("fixture editor unavailable"); },
  });
  await assert.rejects(runChatGptBrowserUiRelay(page, {
    requestBody: { messages: [{ author: { role: "user" }, content: { parts: ["fixture"] } }] },
  }), /fixture editor unavailable/);
  assert.equal(page.listenerCount("request"), 0);
  assert.equal(page.listenerCount("response"), 0);
});

test("detached capture ignores late responses and remains idempotent", async (t) => {
  const name = "CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_RESPONSES";
  const previous = process.env[name];
  process.env[name] = "true";
  t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  const page = new EventEmitter();
  const capture = createChatGptUiRequestCapture(page);
  const request = {
    url: () => "https://chatgpt.com/backend-api/f/conversation",
    method: () => "POST", headers: () => ({}), postData: () => "{}",
  };
  page.emit("request", request);
  let finishBody;
  const body = new Promise((resolve) => { finishBody = resolve; });
  const pending = capture.onResponse({
    request: () => request, headers: () => ({}), text: () => body, status: () => 200,
  });
  capture.detach();
  capture.detach();
  finishBody("late fixture");
  await pending;
  capture.onRequest(request);
  assert.equal(capture.records.length, 1);
  assert.equal(capture.records[0].response, undefined);
  assert.equal(page.listenerCount("request"), 0);
  assert.equal(page.listenerCount("response"), 0);
});

test("default UI diagnostics never read request bodies; raw capture requires explicit opt-in", (t) => {
  const name = "CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_REQUESTS";
  const previous = process.env[name];
  t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  let reads = 0;
  const request = {
    url: () => "https://chatgpt.com/backend-api/f/conversation",
    method: () => "POST", headers: () => ({}),
    postData: () => { reads += 1; return '{"prompt":"synthetic-private-message"}'; },
  };
  delete process.env[name];
  const safe = createChatGptUiRequestCapture(new EventEmitter());
  t.after(() => safe.detach());
  safe.onRequest(request);
  assert.equal(reads, 0);
  assert.equal(safe.records[0].postDataJson, null);
  assert.equal(safe.records[0].postDataPreview, null);
  assert.equal(JSON.stringify(safe.records).includes("synthetic-private-message"), false);

  process.env[name] = "true";
  const raw = createChatGptUiRequestCapture(new EventEmitter());
  t.after(() => raw.detach());
  raw.onRequest(request);
  assert.equal(reads, 1);
  assert.equal(raw.records[0].postDataJson.prompt, "synthetic-private-message");
});

test("default UI response diagnostics do not materialize response text", async (t) => {
  const name = "CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_RESPONSES";
  const previous = process.env[name];
  delete process.env[name];
  t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  const capture = createChatGptUiRequestCapture(new EventEmitter());
  t.after(() => capture.detach());
  const request = { url: () => "https://chatgpt.com/backend-api/f/conversation", method: () => "POST", headers: () => ({}), postData: () => "{}" };
  let reads = 0;
  capture.onRequest(request);
  await capture.onResponse({
    request: () => request, headers: () => ({}), status: () => 200,
    text: async () => { reads += 1; return "synthetic-private-response"; },
  });
  assert.equal(reads, 0);
  assert.equal(capture.records[0].response.bodyLength, null);
  assert.equal(capture.records[0].response.bodyPreview, null);
});

test("diagnostic capture caps retained requests and skips excess header/body reads", () => {
  const capture = createChatGptUiRequestCapture(new EventEmitter());
  let headerReads = 0;
  try {
    for (let index = 0; index < 10_000; index += 1) {
      capture.onRequest({
        url: () => "https://chatgpt.com/backend-api/f/conversation", method: () => "POST",
        headers: () => { headerReads += 1; return {}; }, postData: () => "{}",
      });
    }
    assert.equal(capture.records.length, 128);
    assert.equal(capture.droppedRequests, 9872);
    assert.equal(headerReads, 128);
  } finally {
    capture.detach();
  }
});

test("each tracked request can start at most one raw response read", async (t) => {
  const name = "CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_RESPONSES";
  const previous = process.env[name];
  process.env[name] = "true";
  t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  const capture = createChatGptUiRequestCapture(new EventEmitter());
  t.after(() => capture.detach());
  const request = { url: () => "https://chatgpt.com/backend-api/f/conversation", method: () => "POST", headers: () => ({}), postData: () => "{}" };
  capture.onRequest(request);
  let reads = 0;
  let finish;
  const body = new Promise((resolve) => { finish = resolve; });
  const response = { request: () => request, headers: () => ({}), status: () => 200, text: () => { reads += 1; return body; } };
  const first = capture.onResponse(response);
  const second = capture.onResponse(response);
  finish("fixture");
  await Promise.all([first, second]);
  assert.equal(reads, 1);
});
