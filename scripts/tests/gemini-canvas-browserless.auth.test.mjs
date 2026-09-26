import assert from "node:assert/strict";
import test from "node:test";
import { fixtureRequestContext, fixtureSession, importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();
const headerInput = () => ({ ...fixtureRequestContext(), targetOrigin: "https://generativelanguage.googleapis.com" });

test("Browserless SAPISID signing preserves the fixed protocol vector", () => {
  const digest = "c92ea3ce906169829c638f2e0f2ac1fff86836c3";
  assert.equal(app.buildSapisidAuthorization("fixture-session", "https://gemini.google.com", 1700000000), `SAPISIDHASH 1700000000_${digest} SAPISID1PHASH 1700000000_${digest} SAPISID3PHASH 1700000000_${digest}`);
});

test("Browserless session uses matching base cookies, longest paths and stable input order", () => {
  const cookies = [
    { name: "SAPISID", value: "  longest  ", domain: ".google.com", path: "/app/deep" },
    { name: "__Secure-1PAPISID", value: "shorter", domain: ".google.com", path: "/app" },
    { name: "SID", value: "a=b", domain: ".google.com", path: "/app" },
    { name: "noise", value: "excluded", domain: "evilgoogle.com", path: "/" },
    { name: "empty", value: "", domain: ".google.com", path: "/" },
  ];
  const original = structuredClone(cookies);
  assert.deepEqual(app.buildPureHttpSession({ cookies }, "https://generativelanguage.googleapis.com/v1", "https://gemini.google.com/app/deep", " "), {
    cookieHeader: "SAPISID=longest; __Secure-1PAPISID=shorter; SID=a=b", sapisid: "longest", authUser: "0",
  });
  assert.deepEqual(cookies, original);
});

for (const [domain, expected] of [[" .GOOGLE.com ", true], ["gemini.google.com", true], ["evilgoogle.com", false], ["", false]]) {
  test(`Browserless cookie domain matching: ${domain || "empty"}`, () => {
    assert.equal(app.cookieMatchesUrl({ domain, path: "/app" }, new URL("https://gemini.google.com/app/fixture")), expected);
    assert.equal(app.cookieMatchesUrl({ domain, path: "/other" }, new URL("https://gemini.google.com/app/fixture")), false);
  });
}

test("Browserless session rejects missing compatible cookie and malformed URLs", () => {
  assert.throws(() => app.buildPureHttpSession({}, "https://fixture.test", "https://gemini.google.com"), /missing SAPISID-compatible cookie/);
  assert.throws(() => app.buildPureHttpSession({}, "invalid", "https://gemini.google.com"), TypeError);
});

test("Browserless cookie rotation preserves unchanged identity and applies ordered updates without mutation", () => {
  const session = fixtureSession();
  assert.equal(app.mergeSetCookie(session, []), session);
  const merged = app.mergeSetCookie(session, ["SID=a=b; Secure\nSAPISID=rotated; Path=/", "invalid", "__Secure-1PAPISID=last; HttpOnly", "SAPISID=; Max-Age=0"]);
  assert.notEqual(merged, session);
  assert.equal(merged.sapisid, "last");
  assert.equal(merged.cookieHeader, "SAPISID=; SID=a=b; __Secure-1PAPISID=last");
  assert.equal(merged.authUser, "2");
  assert.deepEqual(session, fixtureSession());
});

test("Browserless browser hints preserve existing values and fill missing defaults", () => {
  const headers = { "sec-ch-ua": "fixture-custom", "sec-fetch-mode": "same-origin" };
  app.buildBrowserClientHints(headers);
  assert.equal(headers["sec-ch-ua"], "fixture-custom");
  assert.equal(headers["sec-fetch-mode"], "same-origin");
  assert.equal(headers["sec-ch-ua-platform"], '"Windows"');
  assert.equal(headers["sec-fetch-site"], "cross-site");
});

test("Browserless API-key-only headers omit signed credentials and preserve body option", () => {
  const headers = app.buildJsonHeaders({ ...headerInput(), authMode: { kind: "api_key_only", apiKeyPlacement: "header" }, includeBody: false });
  assert.equal(headers["x-goog-api-key"], "fixture-api-key");
  assert.equal(headers["x-goog-authuser"], "2");
  assert.equal(headers.origin, "https://gemini.google.com");
  for (const name of ["cookie", "authorization", "content-type", "sec-fetch-site"]) assert.equal(name in headers, false);
});

test("Browserless signed headers retain session and same-origin hints without mutating session", () => {
  const input = { ...headerInput(), targetOrigin: "HTTPS://GEMINI.GOOGLE.COM", authMode: { kind: "signed_session", apiKeyPlacement: "none" } };
  const original = structuredClone(input.session), headers = app.buildJsonHeaders(input);
  assert.equal(headers.cookie, input.session.cookieHeader);
  assert.match(headers.authorization, /^SAPISIDHASH \d+_[a-f0-9]{40} SAPISID1PHASH /);
  assert.equal(headers["sec-fetch-site"], "same-origin");
  assert.equal(headers["content-type"], "application/json");
  assert.equal("x-goog-api-key" in headers, false);
  assert.deepEqual(input.session, original);
});

for (const signed of [false, true]) {
  for (const preserve of [false, true]) {
    test(`Browserless cross-origin options signed=${signed} preserve-referer=${preserve}`, () => {
      const headers = app.buildJsonHeadersWithOptions({ ...headerInput(), includeSignedHeaders: signed, preserveCrossOriginOrigin: false, preserveCrossOriginReferer: preserve, refererOverride: " https://fixture.test/ref ", signedOriginOverride: " https://fixture.test ", apiKeyPlacement: "none" });
      assert.equal("origin" in headers, false);
      assert.equal(headers.referer, preserve ? "https://fixture.test/ref" : undefined);
      assert.equal("authorization" in headers, signed);
      assert.equal("cookie" in headers, signed);
      if (signed) assert.equal(headers["x-origin"], "https://fixture.test");
      assert.equal("x-goog-api-key" in headers, false);
    });
  }
}

test("Browserless same-origin unsigned options retain page context despite cross-origin suppression", () => {
  const headers = app.buildJsonHeadersWithOptions({ ...headerInput(), targetOrigin: "https://gemini.google.com", includeSignedHeaders: false, preserveCrossOriginOrigin: false, preserveCrossOriginReferer: false, apiKeyPlacement: "header", includeBody: false });
  assert.equal(headers.origin, "https://gemini.google.com");
  assert.equal(headers.referer, "https://gemini.google.com/app/fixture");
  assert.equal(headers["x-goog-api-key"], "fixture-api-key");
  assert.equal(headers["x-goog-authuser"], "2");
  assert.equal("content-type" in headers, false);
});

test("Browserless form headers preserve signed session and form content type", () => {
  const headers = app.buildFormHeaders(headerInput());
  assert.equal(headers["content-type"], "application/x-www-form-urlencoded;charset=UTF-8");
  assert.equal(headers["sec-fetch-site"], "same-origin");
  assert.equal(headers.cookie, fixtureSession().cookieHeader);
  assert.equal(headers["x-origin"], "https://gemini.google.com");
  assert.match(headers.authorization, /^SAPISIDHASH /);
});

test("Browserless API key query preserves existing key and other parameters", () => {
  assert.equal(app.appendApiKeyQueryIfMissing("invalid", " "), "invalid");
  assert.equal(app.appendApiKeyQueryIfMissing("https://fixture.test/path?key=existing&keep=1", "new"), "https://fixture.test/path?key=existing&keep=1");
  const url = new URL(app.appendApiKeyQueryIfMissing("https://fixture.test/path?keep=1", " a+b "));
  assert.equal(url.searchParams.get("key"), "a+b");
  assert.equal(url.searchParams.get("keep"), "1");
});

for (const [path, labels] of [
  ["https://generativelanguage.googleapis.com/v1/models/image:generateContent", ["api_key_only_query", "api_key_only_header", "signed_session_query", "signed_session_header"]],
  ["https://clients6.google.com/models/text:predict", ["api_key_only_query", "api_key_only_header", "signed_session_query", "signed_session_header"]],
  ["https://generativelanguage.googleapis.com/v1/models/text:generateContent", ["api_key_only_header", "signed_session_header", "signed_session_query"]],
  ["https://lh3.googleusercontent.com/asset", ["signed_session_download"]],
  ["https://gemini.google.com/app", ["signed_session"]],
]) {
  test(`Browserless auth attempt ordering for ${path}`, () => {
    assert.deepEqual(app.buildAuthAttempts(path, "fixture-key").map((entry) => entry.label), labels);
    assert.deepEqual(app.buildAuthAttempts(path, null), [{ label: "signed_session", kind: "signed_session", apiKeyPlacement: "none" }]);
  });
}
