import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { runChatGptBrowserProbe, extractBootstrapArtifacts } from "../chatgpt-web-session/browser-probe.mjs";
import { runChatGptBrowserRelay, shouldFallbackToUiRelay } from "../chatgpt-web-session/http-relay.mjs";
import { DEFAULT_POW_SCRIPT, extractChatGptPowBootstrapFromHtml, mergeChatGptPowBootstrap, powGenerate } from "../chatgpt-web-session/sentinel-pow.mjs";
import { primeBrowserWithImportedCookies, collectCookieHeader, findCookieValue } from "../chatgpt-web-session/cookies.mjs";

test("browser probe executes serialized callback and prefers refreshed session token", async () => {
  const calls = [];
  const page = {
    waitForTimeout: async () => {},
    evaluate: async (callback, input) => vm.runInNewContext(`(${callback})(input)`, {
      input, AbortSignal: { timeout: (value) => value }, atob,
      location: { href: "https://chatgpt.com/" }, localStorage: { getItem: () => null },
      document: {
        documentElement: { outerHTML: "fixture", getAttribute: () => "build" },
        querySelectorAll: () => [], querySelector: () => null,
      },
      fetch: async (url, options) => {
        calls.push({ url, options });
        if (url.endsWith("/api/auth/session")) return Response.json({ accessToken: "refreshed-synthetic", user: { email: "a@example.test" } });
        if (url.endsWith("/models")) return Response.json({ models: [{ slug: "fixture-model" }] });
        return Response.json({ default_model_slug: "fixture-model" });
      },
    }),
  };
  const result = await runChatGptBrowserProbe(page, {
    baseUrl: "https://chatgpt.com", modelsPath: "/models", accessToken: "old-synthetic", timeoutMs: 60_000,
  });
  assert.equal(result.ok, true);
  assert.equal(result.modelCount, 1);
  assert.equal(result.sessionAccessToken, "refreshed-synthetic");
  assert.equal(calls[1].options.headers.Authorization, "Bearer refreshed-synthetic");
  assert.equal(calls[1].options.signal, 30_000);
  assert.equal(result.defaultModelSlug, "fixture-model");
});

test("bootstrap extraction preserves HTML fields and stable script deduplication", () => {
  const source = `${DEFAULT_POW_SCRIPT}?fixture=1`;
  const value = extractBootstrapArtifacts(`<html data-build="build"><script>"accessToken":"synthetic"</script>${source} ${source}</html>`);
  assert.equal(value.accessToken, "synthetic");
  assert.equal(value.powDataBuild, "build");
  assert.deepEqual(value.powSources, [source]);
  assert.deepEqual(extractChatGptPowBootstrapFromHtml("").powScriptSources, [DEFAULT_POW_SCRIPT]);
  assert.deepEqual(mergeChatGptPowBootstrap({ powScriptSources: [source] }, { chatgptPowSources: [source] }).powScriptSources, [source, DEFAULT_POW_SCRIPT]);
});

test("existing proof work loop obeys its explicit zero/one-iteration boundary", () => {
  const config = Array.from({ length: 18 }, (_, index) => index);
  const before = [...config];
  assert.equal(powGenerate("fixture", "ff", config, 0)[1], false);
  assert.equal(powGenerate("fixture", "ff", config, 1)[1], true);
  assert.deepEqual(config, before);
});

test("HTTP relay passes successful transport result without UI fallback", async () => {
  const requests = [];
  const page = {
    content: async () => "",
    evaluate: async (_callback, input) => {
      requests.push(input);
      return { status: 200, contentType: "application/json", bodyText: requests.length === 1 ? '{"token":"synthetic"}' : "data: fixture\n\n" };
    },
  };
  const result = await runChatGptBrowserRelay(page, { baseUrl: "https://chatgpt.com", accessToken: "synthetic", requestBody: { messages: [] } });
  assert.equal(result.status, 200);
  assert.equal(requests.length, 2);
  assert.equal(requests[1].headers["OpenAI-Sentinel-Chat-Requirements-Token"], "synthetic");
  assert.equal(shouldFallbackToUiRelay({ requirementsStatus: 401, status: 200 }), true);
  assert.equal(shouldFallbackToUiRelay({ requirementsStatus: 200, status: 200, bodyText: "fixture" }), false);
});

test("cookie import and export keep existing browser shapes and exact name lookup", async () => {
  let imported;
  await primeBrowserWithImportedCookies({ addCookies: async (cookies) => { imported = cookies; } }, "https://chatgpt.com", {
    cookieHeader: "session=old", sessionCookies: [{ name: "session", value: "new" }],
  });
  assert.equal(imported.length, 1);
  assert.equal(imported[0].value, "new");
  assert.equal(await collectCookieHeader({ cookies: async () => imported }, "https://chatgpt.com"), "session=new");
  assert.equal(findCookieValue("session-other=wrong;session=part=two", "session"), "part=two");
});
