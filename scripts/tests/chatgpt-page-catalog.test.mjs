import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { pageWebCatalog } from "../chatgpt-web-session/browser-catalog.mjs";

function run(read, origin = "https://chatgpt.com") {
  const context = vm.createContext({ location: { origin }, fetch: read, AbortSignal, TextDecoder, TextEncoder, Uint8Array });
  return vm.runInContext(`(${pageWebCatalog.toString()})("fixture-token", "fixture-device")`, context);
}

test("reads only the fixed same-origin Web catalog and returns real unique slugs", async () => {
  const result = await run(async (url, options) => {
    assert.equal(url, "https://chatgpt.com/backend-api/models?history_and_training_disabled=false");
    assert.equal(options.redirect, "error"); assert.equal(options.credentials, "same-origin");
    assert.equal(options.headers.Authorization, "Bearer fixture-token");
    return Response.json({ models: [{ slug: "actual", name: "Display", supported_in_api: false },
      { slug: "actual" }, { name: "Not an ID" }, { slug: "*" }] });
  });
  assert.equal(JSON.stringify(result), JSON.stringify({ status: 200, models: ["actual"] }));
});

test("directory failures are bounded metadata, not login failure or response bodies", async () => {
  for (const [status, type, code] of [[401, "application/json", "catalog_session_invalid"],
    [403, "text/html", "catalog_verification_required"], [429, "text/html", "catalog_http"],
    [200, "text/html", "catalog_not_json"], [200, "application/json", "catalog_invalid"]]) {
    const result = await run(async () => new Response("private body", { status, headers: { "content-type": type } }));
    assert.equal(result.code, code); assert.equal(result.status, status);
    assert.ok(!JSON.stringify(result).includes("private"));
  }
  assert.equal((await run(async () => Response.json({ models: [] }))).code, "catalog_empty");
  assert.equal((await run(async () => { throw new Error("private network"); })).code, "catalog_unavailable");
  assert.equal((await run(() => { throw new Error("must not fetch"); }, "https://example.com")).code, "catalog_unavailable");
});
