import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { pageSession } from "../chatgpt-web-session/browser-import-cdp.mjs";

function fixture({ readyState = "complete", origin = "https://chatgpt.com", fetch: read } = {}) {
  let requests = 0;
  const context = vm.createContext({
    location: { origin }, document: { readyState, querySelectorAll: () => [],
      documentElement: { getAttribute: () => null } },
    navigator: { userAgent: "Fixture", language: "zh-CN" },
    AbortSignal, TextDecoder, Uint8Array, Intl,
    fetch: async (...args) => { requests++; return await read(...args); },
  });
  return { run: () => vm.runInContext(`(${pageSession.toString()})()`, context),
    requests: () => requests };
}

test("a navigating page is retried without prematurely requesting its session", async () => {
  for (const options of [{ readyState: "loading" }, { origin: "https://auth.openai.com" }]) {
    const f = fixture(options);
    assert.equal((await f.run()).failure, "session_page_unavailable");
    assert.equal(f.requests(), 0);
  }
});

test("network failure is not misreported as the user being logged out", async () => {
  const f = fixture({ fetch: async () => { throw new Error("secret-bearing upstream detail"); } });
  assert.equal((await f.run()).failure, "session_request_failed");
});

for (const [status, contentType, expected] of [
  [401, "application/json", "login_required"],
  [403, "text/html", "session_access_forbidden"],
  [429, "application/json", "session_rate_limited"],
  [503, "text/html", "session_service_unavailable"],
  [200, "text/html", "session_response_not_json"],
]) {
  test(`session HTTP ${status} / ${contentType} has a bounded, non-secret classification`, async () => {
    const f = fixture({ fetch: async () => new Response("private response body", {
      status, headers: { "Content-Type": contentType },
    }) });
    const result = await f.run();
    assert.equal(result.failure, expected);
    assert.deepEqual(Object.keys(result), ["failure"]);
    assert.ok(!JSON.stringify(result).includes("private"));
  });
}

test("an explicit browser challenge is distinct from an ordinary forbidden session", async () => {
  for (const status of [403, 503]) {
    const f = fixture({ fetch: async () => new Response("private challenge body", {
      status, headers: { "Content-Type": "text/html", "cf-mitigated": "challenge" },
    }) });
    const result = await f.run();
    assert.equal(result.failure, "session_verification_required");
    assert.deepEqual(Object.keys(result), ["failure"]);
  }
});

test("valid session material still comes from the exact same-origin endpoint only", async () => {
  const f = fixture({ fetch: async (url, options) => {
    assert.equal(url, "https://chatgpt.com/api/auth/session");
    assert.equal(options.credentials, "same-origin");
    assert.equal(options.redirect, "error");
    return Response.json({ accessToken: "fixture-token", user: { email: "fixture@example.test" } });
  } });
  const result = await f.run();
  assert.equal(result.accessToken, "fixture-token");
  assert.equal(result.accountName, "fixture@example.test");
  assert.equal(f.requests(), 1);
});

test("invalid JSON and oversized responses never echo response contents", async () => {
  for (const body of ["private malformed JSON", "x".repeat(128 * 1024 + 1)]) {
    const f = fixture({ fetch: async () => new Response(body, {
      headers: { "Content-Type": "application/json" },
    }) });
    assert.equal((await f.run()).failure, "session_response_invalid");
  }
});
