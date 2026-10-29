import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { waitForGeminiWebRuntime } from "../gemini-web-capture-session.mjs";
import { extractGeminiWebRuntime, isGeminiWebStreamGenerateUrl,
  requestContainsValidationPrompt, VALIDATION_PROMPT } from "../export-gemini-web-runtime.mjs";

const sourceUrl = "https://gemini.google.com/_/BardChatUi/data/batchexecute?authuser=0&bl=b&f.sid=s";
const source = {
  url: () => sourceUrl,
  postData: () => "at=access&f.req=rpc",
  allHeaders: async () => ({referer: "https://gemini.google.com/app", "x-goog-authuser": "0"}),
};
const newRequest = {
  url: () => "https://geminiweb-pa.clients6.google.com/v1/processSession",
  postData: () => new URLSearchParams({req1___data__:
    Buffer.from(VALIDATION_PROMPT).toString("base64")}).toString(),
};
const envelope = JSON.stringify([["wrb.fr", null,
  JSON.stringify([null, null, null, null, [["candidate", ["巴黎"]]]])]]);
const response = (request, status = 200) => ({request: () => request, status: () => status});

function fixture(overrides = {}) {
  const context = new EventEmitter();
  const calls = [];
  let disposed = false;
  context.cookies = async () => [
    {name: "__Secure-1PSID", value: "primary", domain: ".google.com"},
    {name: "__Secure-1PSIDTS", value: "secondary", domain: ".google.com"},
    {name: "SID", value: "supplemental", domain: ".google.com"},
    {name: "OTHER", value: "must-not-forward", domain: "other.invalid"},
  ];
  context.request = {post: async (url, options) => {
    calls.push({url, options});
    return {status: () => 200, body: async () => Buffer.from(envelope),
      dispose: async () => { disposed = true; }};
  }};
  const promise = waitForGeminiWebRuntime(context, {
    targetUrl: "https://gemini.google.com/u/1/", timeoutMs: 1000, prompt: VALIDATION_PROMPT,
    isLegacyRequest: isGeminiWebStreamGenerateUrl,
    containsPrompt: requestContainsValidationPrompt, extractRuntime: extractGeminiWebRuntime,
    ...overrides,
  });
  return {context, calls, promise, disposed: () => disposed};
}

test("new page transport triggers exactly one validated legacy request with exact imported Google cookies", async () => {
  const f = fixture();
  f.context.emit("response", response(source));
  f.context.emit("response", response(newRequest));
  f.context.emit("response", response(newRequest));
  const result = await f.promise;
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0].options.headers.cookie,
    "__Secure-1PSID=primary; __Secure-1PSIDTS=secondary; SID=supplemental");
  assert.equal(f.calls[0].options.maxRetries, 0);
  assert.equal(f.calls[0].options.maxRedirects, 0);
  assert.equal(result.endpointPath.endsWith("/StreamGenerate"), true);
  assert.equal(result.accountIndex, "0");
  assert.equal(result.captureMode, "legacy_http_validation");
  assert.equal(result.responseContainsParis, true);
  assert.equal(f.context.listenerCount("response"), 0);
  assert.equal(f.context.listenerCount("close"), 0);
  assert.equal(f.disposed(), true);
});

test("explicit restored-session recovery proves HTTP replay rather than trusting stored cookies", async () => {
  const f = fixture({resumeValidation: true});
  f.context.emit("response", response(source));
  assert.equal((await f.promise).responseStatus, 200);
  assert.equal(f.calls.length, 1);
});

test("missing bootstrap and raw transport errors fail without credentials or request logs", async () => {
  const missing = fixture();
  missing.context.emit("response", response(newRequest));
  await assert.rejects(missing.promise, /no authenticated legacy bootstrap/);
  assert.equal(missing.calls.length, 0);
  const failed = fixture();
  failed.context.request.post = async () => { throw new Error("private-cookie-and-token"); };
  failed.context.emit("response", response(source));
  failed.context.emit("response", response(newRequest));
  await assert.rejects(failed.promise, error => !error.message.includes("private-cookie-and-token")
    && error.message.includes("compatibility validation failed"));
});

test("timeout and window close detach listeners and admit no late generation", async () => {
  for (const close of [false, true]) {
    const f = fixture({timeoutMs: 5});
    const rejected = assert.rejects(f.promise, close ? /window closed/ : /Timed out/);
    if (close) f.context.emit("close");
    await rejected;
    f.context.emit("response", response(source));
    f.context.emit("response", response(newRequest));
    assert.equal(f.calls.length, 0);
    assert.equal(f.context.listenerCount("response"), 0);
  }
});
