import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { pathToFileURL } from "node:url";

const { executeLumaOperation } = await import(process.env.LUMALABS_TEST_OPERATION
  ? pathToFileURL(process.env.LUMALABS_TEST_OPERATION).href
  : "../lumalabs-session/browser-operation.mjs");

async function runInBrowser(fetchImpl, overrides = {}) {
  const timers = new Set();
  let expire;
  const context = vm.createContext({
    window: { location: { origin: "https://luma.example.test" } },
    crypto: { randomUUID: () => "fixture-id" },
    AbortController,
    TextDecoder,
    setTimeout: (callback) => { expire = callback; timers.add(1); return 1; },
    clearTimeout: (id) => timers.delete(id),
    fetch: (url, init) => fetchImpl(url, init, () => expire()),
  });
  const result = await vm.runInContext(`(${executeLumaOperation.toString()})`, context)({
    realmId: "realm-1", mediaOperation: "image", artifactField: "image", locale: "zh-CN",
    actionBody: { type: "create_image_uni_1" }, autoDiscoverActionType: false,
    timeoutMs: 1000, clientCapabilities: "retry,upgrade_plan", ...overrides,
  });
  assert.equal(timers.size, 0, "operation must release its deadline on every return path");
  return JSON.parse(JSON.stringify(result));
}

test("serialized operation preserves the signature error contract and clears its timer", async () => {
  const calls = [];
  const result = await runInBrowser(async (url, init) => {
    calls.push({ url, init });
    return { ok: false, status: 401, text: async () => "private response" };
  });
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "lumalabs_signature_failed");
  assert.equal(result.error.status, 401);
  assert.ok(!JSON.stringify(result).includes("private response"));
  assert.equal(calls.length, 1);
  assert.match(calls[0].url, /\/signature$/);
  assert.equal(calls[0].init.credentials, "include");
});

test("deadline aborts the request and maps a timeout error", async () => {
  let signal;
  const result = await runInBrowser(async (_url, init, expire) => {
    signal = init.signal;
    expire();
    throw new Error("timeout");
  });
  assert.equal(result.ok, false);
  assert.equal(result.error.code, "lumalabs_browser_timeout");
  assert.ok(signal);
  assert.equal(signal.aborted, true);
});

const jsonResponse = (body, status = 200) => new Response(JSON.stringify(body), { status });

function protocol({ signature, action, frames, menus, actionStatus = 200, menusStatus = 200 } = {}) {
  const calls = [];
  let cancelled = false;
  const chunks = (frames ?? []).map((value) => new TextEncoder().encode(value));
  const fetch = async (url, init) => {
    calls.push({ url, init });
    if (url.endsWith("/signature")) return jsonResponse(signature ?? {
      ws_token: "fixture-ws", cdn_url: "https://cdn.example.test/", query_params: "?fixture=1",
    });
    if (url.includes("/events/stream/")) return {
      ok: true, status: 200, headers: new Headers({"content-type":"text/event-stream"}),
      body: { getReader: () => ({
        read: async () => chunks.length ? { done: false, value: chunks.shift() } : { done: true },
        cancel: async () => { cancelled = true; },
        releaseLock: () => {},
      }) },
    };
    if (url.includes("/menus.json")) return jsonResponse(menus ?? {}, menusStatus);
    if (url.endsWith("/actions")) return jsonResponse(action ?? {
      output_artifacts: { image: ["output-1"] }, action: { id: " action-1 " },
    }, actionStatus);
    throw new Error(`Unexpected fixture URL: ${url}`);
  };
  return { fetch, calls, wasCancelled: () => cancelled };
}

test("split SSE chunks select the matching completed artifact and cancel the reader", async () => {
  const event = `data: ${JSON.stringify({ items: [
    { artifact: { id: "other", object_ref: "wrong.png", state: "completed" } },
    { artifact: { id: "output-1", object_ref: "/right.png", state: "completed" } },
  ] })}\n\n`;
  const fixture = protocol({ frames: ["data: keepalive\n\n", event.slice(0, 40), event.slice(40)] });
  const result = await runInBrowser(fixture.fetch);
  assert.deepEqual(result, {
    ok: true, signedUrl: "https://cdn.example.test/right.png?fixture=1", outputId: "output-1",
    resolvedActionType: "create_image_uni_1", actionId: "action-1",
    realmId: "realm-1", mediaOperation: "image", artifactField: "image", state: "completed",
  });
  assert.equal(fixture.wasCancelled(), true);
  assert.deepEqual(fixture.calls.map(({ url }) => new URL(url).pathname), [
    "/api/vespa/realms/realm-1/signature", "/events/api/vespa/realms/realm-1/events/stream/grip",
    "/api/vespa/realms/realm-1/actions",
  ]);
  assert.equal(fixture.calls[1].init.headers.authorization, "Bearer fixture-ws");
});

test("menu discovery prefers the exact operation id and does not mutate caller input", async () => {
  const fixture = protocol({ menus: { menus: [{ type: "action", id: "create_image_fallback" },
    { type: "action", id: "create_image_uni_1" }] } });
  const actionBody = { type: "old", fields: { prompt: "fixture" } };
  const result = await runInBrowser(fixture.fetch, { actionBody, autoDiscoverActionType: true });
  const sent = JSON.parse(fixture.calls.find(({ url }) => url.endsWith("/actions")).init.body);
  assert.deepEqual(sent, { type: "create_image_uni_1", fields: { prompt: "fixture" } });
  assert.equal(actionBody.type, "old");
  assert.equal(result.error.code, "lumalabs_missing_signed_url");
});

test("failed menu discovery preserves the caller action and action-error metadata", async () => {
  const fixture = protocol({ menusStatus: 503, actionStatus: 429, action: { error: "busy" } });
  const result = await runInBrowser(fixture.fetch, { actionBody: { type: "old" }, autoDiscoverActionType: true });
  assert.equal(result.error.code, "lumalabs_action_failed");
  assert.equal(result.error.status, 429);
  assert.equal(result.error.resolvedActionType, "old");
  assert.equal(JSON.parse(fixture.calls.at(-1).init.body).type, "old");
});

test("missing signature token stops before subscribing or dispatching", async () => {
  const fixture = protocol({ signature: {} });
  const result = await runInBrowser(fixture.fetch);
  assert.equal(result.error.code, "lumalabs_missing_ws_token");
  assert.equal(fixture.calls.length, 1);
});

test("missing output id preserves the action-stage error", async () => {
  const fixture = protocol({ action: { output_artifacts: {} } });
  const result = await runInBrowser(fixture.fetch);
  assert.equal(result.error.code, "lumalabs_missing_output_id");
  assert.equal(result.error.stage, "action");
});

for (const mediaOperation of ["image", "video", "audio"]) {
  test(`${mediaOperation} discovery rejects a nonempty menu without a matching action before POST`, async () => {
    const fixture = protocol({ menus: { menus: [{ type: "action", id: "delete_board" }] } });
    const result = await runInBrowser(fixture.fetch, { mediaOperation, autoDiscoverActionType: true });
    assert.equal(fixture.calls.filter(({ init }) => init.method === "POST").length, 0);
    assert.equal(result.ok, false);
    assert.equal(result.error.code, "lumalabs_action_unavailable");
    assert.equal(result.error.stage, "action_discovery");
    assert.equal(fixture.wasCancelled(), true);
    assert.ok(fixture.calls.every(({ init }) => init.signal.aborted));
  });
}

test("explicit action bypasses optional discovery", async () => {
  const fixture = protocol();
  await runInBrowser(fixture.fetch, { actionBody: { type: "configured_action" } });
  assert.ok(!fixture.calls.some(({ url }) => url.includes("/menus.json")));
  assert.equal(JSON.parse(fixture.calls.at(-1).init.body).type, "configured_action");
});

for (const [ids, expected] of [
  [[], "old"],
  [["old", "create_image_uni_1"], "old"],
  [["create_image_fixture"], "create_image_fixture"],
]) {
  test(`discovery preserves existing selection policy: ${JSON.stringify(ids)}`, async () => {
    const fixture = protocol({ menus: { menus: ids.map((id) => ({ type: "action", id })) } });
    await runInBrowser(fixture.fetch, { actionBody: { type: "old" }, autoDiscoverActionType: true });
    assert.equal(JSON.parse(fixture.calls.at(-1).init.body).type, expected);
    assert.equal(fixture.wasCancelled(), true);
  });
}
