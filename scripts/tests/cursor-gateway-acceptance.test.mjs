import assert from "node:assert/strict";
import test from "node:test";
import { cursorErrorCodes, exactCompletedResponse, exactRoute, safeCursorErrorCode } from "../../tools/cursor_remote/gateway_acceptance.mjs";
import { httpOnce } from "../../tools/cursor_remote/gateway_http_acceptance.mjs";

const fixture = () => ({
  revision: { id: "r-fixture" },
  document: {
    providers: [
      { id: "other", auth_mode: "api_key", credentials: [{ id: "unrelated", enabled: true }] },
      { id: "cursor", auth_mode: "cursor_remote", credentials: [
        { id: "mine", enabled: true, scheduled_probe_enabled: false, supported_models: [] },
        { id: "disabled", enabled: false },
      ] },
    ],
    model_routes: [{ model: "unrelated-model", providers: ["other"] }],
  },
});

test("shared Gateway preserves unrelated routes while binding one authorized Cursor account", () => {
  const value = fixture(), before = JSON.stringify(value);
  assert.deepEqual(exactRoute(value, "cursor", "mine"),
    { revision: "r-fixture", provider: "cursor", credential: "mine", enabled: true });
  assert.equal(JSON.stringify(value), before);
});

test("shared Gateway rejects ambiguity, another credential, probes and generic model declarations", () => {
  for (const mutate of [
    (p) => { p.credentials[1].enabled = true; },
    (p) => { p.credentials[0].id = "someone-else"; },
    (p) => { p.credentials[0].enabled = false; },
    (p) => { p.credentials[0].scheduled_probe_enabled = true; },
    (p) => { p.credentials[0].supported_models = ["cursor-personal"]; },
  ]) {
    const value = fixture();
    mutate(value.document.providers[1]);
    assert.throws(() => exactRoute(value, "cursor", "mine"), /exact_private_account_required/);
  }
  const value = fixture();
  value.document.providers[1].auth_mode = "cursor_acp";
  assert.throws(() => exactRoute(value, "cursor", "mine"), /provider_mismatch/);
});

test("only a completed, attributable and confirmed delivery is valid receipt evidence", () => {
  const route = exactRoute(fixture(), "cursor", "mine");
  const trace = { routeAttemptCount: 1, routeRevision: route.revision, realCredentialRef: "mine",
    selectedPipelineMode: "cursor_remote", cursorRemoteDeliveryUnknown: false };
  const row = { completedAt: "2026-11-26T01:00:00Z", status: "completed", endpointKind: "responses",
    upstreamStatus: 200, providerAccountId: "cursor", routeTrace: trace };
  assert.equal(exactCompletedResponse(row, route), true);
  for (const change of [{ completedAt: null }, { status: "failed" }, { endpointKind: "chat_completions" },
    { upstreamStatus: null }, { providerAccountId: "other" }, { routeTrace: null }]) {
    assert.equal(exactCompletedResponse({ ...row, ...change }, route), false);
  }
  for (const change of [{ cursorRemoteDeliveryUnknown: undefined }, { cursorRemoteDeliveryUnknown: true },
    { routeAttemptCount: 0 }, { routeAttemptCount: 2 }, { realCredentialRef: "other" },
    { routeRevision: "old" }, { selectedPipelineMode: "other" }, { fallbackEligible: true },
    { errorCode: "fixture_failure" }]) {
    assert.equal(exactCompletedResponse({ ...row, routeTrace: { ...trace, ...change } }, route), false);
  }
});

test("HTTP acceptance sends one stateless Responses POST with redirects and retries disabled", async () => {
  let calls = 0;
  const receipt = await httpOnce("http://127.0.0.1:4200/", "fixture", "expected", async (url, request) => {
    calls++;
    assert.equal(url, "http://127.0.0.1:4200/v1/responses");
    assert.equal(request.method, "POST");
    assert.equal(request.redirect, "error");
    const body = JSON.parse(request.body);
    assert.equal(body.store, false);
    assert.equal(body.stream, false);
    return Response.json({ status: "completed", output: [
      { content: [{ type: "output_text", text: "expected" }] },
    ] });
  });
  assert.equal(calls, 1);
  assert.equal(receipt.expectedReplyObserved, true);
  assert.equal(receipt.completed, true);
  calls = 0;
  await assert.rejects(httpOnce("http://127.0.0.1:4200", "fixture", "expected", async () => {
    calls++;
    throw new Error("unknown_delivery");
  }), /unknown_delivery/);
  assert.equal(calls, 1);
});

test("HTTP acceptance bounds output and never returns upstream secrets or arbitrary text", async () => {
  const receipt = await httpOnce("http://127.0.0.1:4200", "fixture", "expected", async () =>
    Response.json({ error: { code: "cursor_remote_draining", message: "secret" } }, { status: 503 }));
  assert.equal(receipt.httpStatus, 503);
  assert.equal(receipt.errorCode, "cursor_remote_draining");
  assert.equal(JSON.stringify(receipt).includes("secret"), false);
  await assert.rejects(httpOnce("http://127.0.0.1:4200", "fixture", "expected", async () =>
    new Response("x".repeat(128 * 1024 + 1))), /acceptance_response_too_large/);
});

test("numeric Cursor error codes survive HTTP and CLI receipts without truncation", async () => {
  const code = "cursor_remote_verified_http2_required";
  const receipt = await httpOnce("http://127.0.0.1:4200", "fixture", "expected", async () =>
    Response.json({ error: { code, message: "do not publish this" } }, { status: 503 }));
  assert.equal(receipt.errorCode, code);
  assert.equal(JSON.stringify(receipt).includes("do not publish this"), false);
  assert.deepEqual(cursorErrorCodes(`error="${code}" repeated ${code}`), [code]);
  for (const invalid of [null, {}, `${code}\nsecret`, `x${code}`, `${code}UPPER`,
    `cursor_remote_${"x".repeat(97)}`]) {
    assert.equal(safeCursorErrorCode(invalid), null);
    assert.deepEqual(cursorErrorCodes(String(invalid)), invalid === `${code}\nsecret` ? [code] : []);
  }
});
