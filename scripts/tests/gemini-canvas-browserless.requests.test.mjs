import assert from "node:assert/strict";
import test from "node:test";
import { fixtureResponse, importTestableProbe, requestHarness } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();
const apiUrl = "https://generativelanguage.googleapis.com/v1/models/text:generateContent";
const body = { contents: [{ parts: [{ text: "fixture" }] }] };

test("Browserless exact request keeps minimal headers, body identity and no session rotation", async () => {
  const h = requestHarness(app, { responses: [fixtureResponse({ setCookie: ["SAPISID=rotated"] })] });
  const result = await h.send.sendExactMinimalApiKeyOnlyJson(h.context, "exact", apiUrl, body, 1234);
  assert.equal(result.request.body, body);
  assert.equal(h.requests[0].body, JSON.stringify(body));
  assert.equal(h.requests[0].headers, result.request.headers);
  assert.equal(h.requests[0].method, "POST");
  assert.equal(h.requests[0].redirect, "follow");
  assert.equal(h.requests[0].signal, h.signals[0]);
  assert.equal(h.signals[0].ms, 1234);
  assert.equal(new URL(h.requests[0].url).searchParams.get("key"), "fixture-api-key");
  assert.equal("cookie" in result.request.headers, false);
  assert.equal(result.request.headersRedacted["x-goog-api-key"], "<redacted>");
  assert.equal(h.context.session, h.initialSession);
  assert.deepEqual(h.archives[0], ["fixture-output", "exact", result.request, result.response]);
});

for (const name of ["sendJsonWithAuthAttempts", "sendGetBytesWithAuthAttempts", "sendGetJsonWithAuthAttempts"]) {
  test(`Browserless ${name} rotates session before retry and stops at first success`, async () => {
    const h = requestHarness(app, { responses: [fixtureResponse({ status: 401, setCookie: ["SAPISID=rotated"] }), fixtureResponse()] });
    const args = name === "sendJsonWithAuthAttempts" ? [body, 2000] : [2000];
    const result = await h.send[name](h.context, "request", apiUrl, ...args);
    assert.equal(result.ok, true);
    assert.equal(h.requests.length, 2);
    assert.equal(h.context.session.sapisid, "rotated");
    assert.equal(h.requests[1].headers.cookie, "SAPISID=rotated");
    assert.equal(result.request, result.attempts[1].request);
    assert.equal(result.response, result.attempts[1].response);
    assert.deepEqual(h.archives.map((entry) => entry[1]), ["request.attempt-01", "request.attempt-02"]);
    assert.deepEqual(h.calls, ["timeout", "fetch", "archive", "timeout", "fetch", "archive"]);
    assert.equal(h.requests[0].signal, h.signals[0]);
    if (name === "sendJsonWithAuthAttempts") {
      assert.equal(result.request.body, body);
      assert.equal(h.requests[0].method, "POST");
    } else {
      assert.equal(h.requests[0].method, "GET");
      assert.equal("body" in h.requests[0], false);
      assert.equal("content-type" in h.requests[0].headers, false);
      assert.equal(h.requests[0].headers.accept, name === "sendGetBytesWithAuthAttempts" ? "*/*" : "application/json");
    }
    assert.equal("responseJson" in result, name !== "sendGetBytesWithAuthAttempts");
    assert.equal("url" in result, name !== "sendGetJsonWithAuthAttempts");
  });

  test(`Browserless ${name} exhausts ordered attempts and retains final failure`, async () => {
    const h = requestHarness(app, { responses: [fixtureResponse({ status: 401 }), fixtureResponse({ status: 403 }), fixtureResponse({ status: 429, text: "invalid" })] });
    const args = name === "sendJsonWithAuthAttempts" ? [body, 2000] : [2000];
    const result = await h.send[name](h.context, "failure", apiUrl, ...args);
    assert.equal(result.ok, false);
    assert.equal(result.attempts.length, 3);
    assert.equal(result.request, result.attempts[2].request);
    assert.equal(result.response.status, 429);
    assert.deepEqual(Array.from(result.attempts, (entry) => entry.request.label), ["api_key_only_header", "signed_session_header", "signed_session_query"]);
    assert.equal(new URL(h.requests[2].url).searchParams.get("key"), "fixture-api-key");
    if (name !== "sendGetBytesWithAuthAttempts") assert.equal(result.responseJson, null);
  });
}

test("Browserless custom attempts retain signed/key placement order and request body identity", async () => {
  const h = requestHarness(app, { responses: Array.from({ length: 5 }, () => fixtureResponse({ status: 403 })) });
  const attempts = [
    { label: "signed", kind: "fixture", requestVariant: "one", requestUrl: apiUrl, requestBody: body, includeSignedHeaders: true },
    { label: "unsigned", kind: "fixture", requestVariant: "two", requestUrl: apiUrl, requestBody: body, includeSignedHeaders: false },
  ];
  const result = await h.send.sendJsonWithCustomAttempts(h.context, "custom", attempts, 3000);
  assert.equal(result.ok, false);
  assert.deepEqual(Array.from(result.attempts, ({ request }) => [request.label, request.apiKeyPresent, request.apiKeyPlacement]), [
    ["signed", false, "none"], ["signed", true, "header"], ["signed", true, "query"], ["unsigned", true, "header"], ["unsigned", true, "query"],
  ]);
  assert.ok(result.attempts.every((entry) => entry.request.body === body));
  assert.equal(result.request, result.attempts[4].request);
  assert.deepEqual(h.archives.map((entry) => entry[1]), Array.from({ length: 5 }, (_, i) => `custom.attempt-0${i + 1}`));
});

test("Browserless custom attempts handle no candidates and return early on success", async () => {
  const empty = requestHarness(app);
  const result = await empty.send.sendJsonWithCustomAttempts(empty.context, "empty", [], 1000);
  assert.equal(result.ok, false);
  assert.equal(result.request, null);
  assert.equal(result.response, null);
  assert.equal(result.responseJson, null);
  assert.equal(result.attempts.length, 0);
  assert.equal(empty.requests.length, 0);
  const h = requestHarness(app, { context: { apiKey: null } });
  const success = await h.send.sendJsonWithCustomAttempts(h.context, "one", [{ label: "fixture", requestUrl: apiUrl, requestBody: body, includeSignedHeaders: true }], 1000);
  assert.equal(success.ok, true);
  assert.equal(h.requests.length, 1);
  assert.equal(success.request.apiKeyPlacement, "none");
});

test("Browserless form retry replaces XSRF once after cookie rotation and preserves trailing delimiter", async () => {
  const h = requestHarness(app, { responses: [fixtureResponse({ status: 403, text: '["xsrf","new token"]', setCookie: ["SAPISID=rotated"] }), fixtureResponse({ status: 403, text: '["xsrf","third token"]' })] });
  const result = await h.send.sendFormRequest(h.context, "form", "https://gemini.google.com/_/rpc", "keep=a%2Bb&at=old&", 4000);
  assert.equal(result.ok, false);
  assert.equal(h.requests.length, 2);
  assert.equal(h.requests[1].body, "keep=a%2Bb&at=new+token&");
  assert.equal(h.requests[1].headers.cookie, "SAPISID=rotated");
  assert.equal(result.request.body, h.requests[1].body);
  assert.deepEqual(h.archives.map((entry) => entry[1]), ["form.attempt-01", "form.attempt-02"]);
});

for (const [label, form, allow, responseText] of [["disabled", "at=old&", false, '["xsrf","new"]'], ["already present", "at=new%20token&", true, '["xsrf","new token"]'], ["no token", "at=old&", true, "denied"]]) {
  test(`Browserless form avoids XSRF retry when ${label}`, async () => {
    const h = requestHarness(app, { responses: [fixtureResponse({ status: 403, text: responseText })] });
    const result = await h.send.sendFormRequest(h.context, "form", "https://gemini.google.com/_/rpc", form, 1000, allow);
    assert.equal(result.ok, false);
    assert.equal(h.requests.length, 1);
    assert.equal(result.request.body, form);
  });
}

test("Browserless form success preserves response JSON and session update", async () => {
  const h = requestHarness(app, { responses: [fixtureResponse({ setCookie: ["SAPISID=rotated"] })] });
  const result = await h.send.sendFormRequest(h.context, "form", "https://gemini.google.com/_/rpc", "x=1&", 1000);
  assert.deepEqual(result.responseJson, { marker: "fixture" });
  assert.equal(result.ok, true);
  assert.equal(h.context.session.sapisid, "rotated");
});

for (const name of ["sendExactMinimalApiKeyOnlyJson", "sendJsonWithAuthAttempts", "sendGetBytesWithAuthAttempts", "sendGetJsonWithAuthAttempts", "sendFormRequest", "sendJsonWithCustomAttempts"]) {
  for (const failureAt of ["fetch", "body", "archive"]) {
    test(`Browserless ${name} preserves ${failureAt} failure without extra retries`, async () => {
      const bodyFailure = new Error("fixture response body failed");
      const h = requestHarness(app, { failureAt, responses: [fixtureResponse({ setCookie: ["SAPISID=rotated"], bodyError: failureAt === "body" ? bodyFailure : undefined })] });
      const args = name === "sendJsonWithCustomAttempts" ? [[{ requestUrl: apiUrl, requestBody: body }], 1000]
        : name === "sendFormRequest" ? [apiUrl, "x=1&", 1000]
          : [apiUrl, ...(/Json$|^sendJsonWithAuthAttempts$/.test(name) ? [body, 1000] : [1000])];
      await assert.rejects(h.send[name](h.context, "error", ...args), (error) => error === (failureAt === "body" ? bodyFailure : h.failure));
      assert.equal(h.requests.length, 1);
      assert.equal(h.archives.length, failureAt === "archive" ? 1 : 0);
      assert.equal(h.context.session.sapisid, failureAt === "archive" && name !== "sendExactMinimalApiKeyOnlyJson" ? "rotated" : "fixture-session");
    });
  }
}
