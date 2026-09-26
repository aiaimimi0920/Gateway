import assert from "node:assert/strict";
import test from "node:test";
import { fixtureResponse, importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();

test("Browserless text response preserves headers, cookie array identity and final URL", async () => {
  const cookies = ["SAPISID=fixture-rotated; Path=/"];
  const response = fixtureResponse({ status: 403, text: "fixture body", setCookie: cookies, headers: { "x-fixture": "retained" }, url: "https://fixture.test/redirected" });
  const record = await app.collectTextResponse(response);
  assert.deepEqual(record, { status: 403, ok: false, finalUrl: "https://fixture.test/redirected", headers: { "x-fixture": "retained" }, setCookie: cookies, text: "fixture body" });
  assert.equal(record.setCookie, cookies);
});

test("Browserless response supports legacy single Set-Cookie and absent cookies", () => {
  assert.deepEqual(app.getSetCookieValues(fixtureResponse({ legacyCookies: true, headers: { "set-cookie": "SID=fixture; Secure" } })), ["SID=fixture; Secure"]);
  assert.deepEqual(app.getSetCookieValues(fixtureResponse({ legacyCookies: true })), []);
});

test("Browserless binary response retains bytes without text decoding", async () => {
  const record = await app.collectBytesResponse(fixtureResponse({ bytes: [0, 128, 255, 10] }));
  assert.ok(Buffer.isBuffer(record.bytes));
  assert.deepEqual([...record.bytes], [0, 128, 255, 10]);
  assert.equal(record.ok, true);
  assert.equal("text" in record, false);
});

for (const name of ["collectTextResponse", "collectBytesResponse"]) {
  test(`Browserless ${name} preserves body read failure identity`, async () => {
    const failure = new Error("fixture response body failed");
    await assert.rejects(app[name](fixtureResponse({ bodyError: failure })), (error) => error === failure);
  });
}

test("Browserless diagnostic redaction is case insensitive and does not alter sending headers", () => {
  const headers = { Cookie: "fixture-cookie", AUTHORIZATION: "fixture-auth", "X-Goog-Api-Key": "fixture-key", Accept: "application/json" };
  assert.deepEqual(app.redactHeaders(headers), { Cookie: "<redacted>", AUTHORIZATION: "<redacted>", "X-Goog-Api-Key": "<redacted>", Accept: "application/json" });
  assert.equal(headers.Cookie, "fixture-cookie");
  assert.equal(headers.AUTHORIZATION, "fixture-auth");
  assert.deepEqual(app.redactHeaders(null), {});
});

test("Browserless response JSON and preview preserve null and truncation boundaries", () => {
  assert.deepEqual(app.tryParseJson('{"fixture":true}'), { fixture: true });
  assert.equal(app.tryParseJson("invalid"), null);
  assert.equal(app.tryParseJson("null"), null);
  assert.equal(app.textPreview(null), "");
  assert.equal(app.textPreview("abcd", 4), "abcd");
  assert.equal(app.textPreview("abcde", 4), "abcd...[truncated]");
});
