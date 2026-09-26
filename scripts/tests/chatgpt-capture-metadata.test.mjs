import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { createChatGptUiRequestCapture, redactCapturedRequestHeaders } from "../chatgpt-web-session/ui-capture.mjs";

test("default headers redact response cookies and unknown credential carriers", () => {
  const result = redactCapturedRequestHeaders({
    "set-cookie": "session=synthetic-secret; HttpOnly",
    "x-api-key": "synthetic-secret",
    location: "https://example.test/?code=synthetic-secret",
    referer: "https://example.test/?token=synthetic-secret",
    "x-provider-session": "synthetic-secret",
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": "42",
  });
  assert.equal(JSON.stringify(result).includes("synthetic-secret"), false);
  assert.equal(result["Content-Type"], "application/json; charset=utf-8");
  assert.equal(result["Content-Length"], "42");
  assert.equal(result["set-cookie"].redacted, true);
});

test("default metadata header output has bounded cardinality and cookie names", () => {
  const result = redactCapturedRequestHeaders(Object.fromEntries([
    ["cookie", Array.from({ length: 500 }, (_, index) => `${"n".repeat(200)}${index}=value`).join("; ")],
    ...Array.from({ length: 1000 }, (_, index) => [`x-${index}-${"k".repeat(500)}`, "v".repeat(1000)]),
  ]));
  assert.ok(Object.keys(result).length <= 64);
  assert.ok(Object.keys(result).every((name) => name.length <= 128));
  assert.ok(result.cookie.cookieNames.length <= 32);
  assert.ok(result.cookie.cookieNames.every((name) => name.length <= 128));
  assert.ok(JSON.stringify(result).length < 20_000);
});

test("capture URL drops userinfo, query and fragment before retention", (t) => {
  const capture = createChatGptUiRequestCapture(new EventEmitter());
  t.after(() => capture.detach());
  capture.onRequest({
    url: () => "https://synthetic-user:synthetic-pass@chatgpt.com/backend-api/conversation?token=synthetic-secret#synthetic-fragment",
    method: () => "POST", headers: () => ({}), postData: () => null,
  });
  assert.equal(capture.records[0].url, "https://chatgpt.com/backend-api/conversation");
});

test("header projection ignores inherited fields and safely handles prototype-like names", () => {
  const input = Object.assign(Object.create({ inherited: "synthetic-secret" }), {
    "content-type": "application/json; private=synthetic-secret",
  });
  Object.defineProperty(input, "__proto__", { enumerable: true, value: "synthetic-secret" });
  const result = redactCapturedRequestHeaders(input);
  assert.equal(Object.getPrototypeOf(result), null);
  assert.equal(Object.hasOwn(result, "inherited"), false);
  assert.equal(result.__proto__.redacted, true);
  assert.equal(JSON.stringify(result).includes("synthetic-secret"), false);
});

test("oversized capture URLs are replaced rather than retaining arbitrary query data", (t) => {
  const capture = createChatGptUiRequestCapture(new EventEmitter());
  t.after(() => capture.detach());
  capture.onRequest({
    url: () => `https://chatgpt.com/backend-api/conversation?q=${"x".repeat(10_000)}`,
    method: () => "POST", headers: () => ({}), postData: () => null,
  });
  assert.equal(capture.records[0].url, "[redacted-url]");
});
