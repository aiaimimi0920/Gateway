import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  extractPrompt, parseBoolean, clipsReadyForTarget, clipsTerminal,
  normalizeTargetAssetKind, normalizeTimeoutMs, createError, normalizeError,
} from "../suno-browser/pure.mjs";
import {
  parseCookieHeader, resolveExecutablePath, resolveWorkerPage, dismissKnownBlockingOverlays,
  resolveBrowserExecutionTarget, releaseEasyBrowserLease,
} from "../suno-browser/browser.mjs";

test("extracts prompt-compatible fields", () => {
  assert.equal(extractPrompt({ prompt: "  hello  " }), "hello");
  assert.equal(extractPrompt({ parts: [{ text: "song" }] }), "song");
  assert.equal(extractPrompt({}), null);
});

test("cookie parsing preserves values and ignores malformed entries", () => {
  assert.deepEqual(parseCookieHeader("sid=abc; bad; theme=dark"), [
    { name: "sid", value: "abc", domain: ".suno.com", path: "/", httpOnly: false, secure: true, sameSite: "Lax" },
    { name: "theme", value: "dark", domain: ".suno.com", path: "/", httpOnly: false, secure: true, sameSite: "Lax" },
  ]);
});

test("clip completion requires terminal success and requested asset", () => {
  assert.equal(clipsReadyForTarget([{ status: "complete", audio_url: "https://a" }], "audio"), true);
  assert.equal(clipsReadyForTarget([{ status: "complete" }], "audio"), false);
  assert.equal(clipsReadyForTarget([{ status: "error", audio_url: "https://a" }], "audio"), false);
  assert.equal(parseBoolean("off", true), false);
});

test("pending, failed, camelCase media, and structured errors remain distinct", () => {
  assert.equal(clipsTerminal([]), false);
  assert.equal(clipsTerminal([{ status: "streaming" }]), false);
  assert.equal(clipsTerminal([{ status: "failed" }]), true);
  assert.equal(clipsReadyForTarget([{ status: "completed", videoUrl: "fixture" }], "video"), true);
  assert.equal(clipsReadyForTarget([{ status: "completed", audio_url: "fixture" }], "video"), false);
  assert.equal(normalizeTimeoutMs("1", 100, 10, 1000), 10);
  assert.equal(normalizeTimeoutMs("invalid", 100, 10, 1000), 100);
  assert.throws(() => normalizeTargetAssetKind("unknown"), { code: "suno_browser_invalid_target_asset_kind" });
  assert.deepEqual(normalizeError(createError(409, "fixture", "message", " body ")), {
    status: 409, code: "fixture", message: "message", body: "body",
  });
});

test("browser executable lookup supports an explicit path and platform defaults", () => {
  assert.equal(resolveExecutablePath(process.execPath), process.execPath);
  const fallback = resolveExecutablePath("");
  assert.ok(fallback === null || typeof fallback === "string");
});

function fixturePage(url, actions) {
  return {
    url: () => url,
    goto: async (target, options) => actions.push(["goto", target, options.timeout]),
    waitForLoadState: async (state) => actions.push(["load", state]),
    waitForTimeout: async (timeout) => actions.push(["wait", timeout]),
  };
}

test("borrowed CDP page is reused without navigation or new pages", async () => {
  const actions = [];
  const page = fixturePage("https://suno.fixture.invalid/create/?view=one", actions);
  const context = { pages: () => [page], newPage: async () => assert.fail("borrowed page created") };
  const options = {
    targetUrl: "https://suno.fixture.invalid/create", fallbackUrl: "https://suno.fixture.invalid/create",
    navigationTimeoutMs: 50, forceNavigate: true, borrowedContext: true,
  };
  assert.equal(await resolveWorkerPage(context, options), page);
  assert.deepEqual(actions, [["load", "domcontentloaded"], ["wait", 1500]]);
  await assert.rejects(resolveWorkerPage({ ...context, pages: () => [] }, options), /expected create page/);
});

test("owned context creates and navigates its page with the original timeout", async () => {
  const actions = [];
  const page = fixturePage("about:blank", actions);
  const targetUrl = "https://suno.fixture.invalid/create";
  const actual = await resolveWorkerPage({ pages: () => [], newPage: async () => page }, {
    targetUrl, fallbackUrl: targetUrl, navigationTimeoutMs: 50,
    forceNavigate: false, borrowedContext: false,
  });
  assert.equal(actual, page);
  assert.deepEqual(actions, [["goto", targetUrl, 50], ["wait", 1500]]);
});

test("overlay cleanup does not send Escape through a visible security challenge", async () => {
  const actions = [];
  const page = {
    evaluate: async () => true,
    keyboard: { press: async (key) => actions.push(key) },
    waitForTimeout: async (timeout) => actions.push(timeout),
  };
  await dismissKnownBlockingOverlays(page);
  assert.deepEqual(actions, [300]);
});

test("EasyBrowser acquisition, CDP bypass, and release retain transport contracts", async (t) => {
  const calls = [];
  t.mock.method(globalThis, "fetch", async (url, options) => {
    calls.push({ url, ...options });
    return {
      ok: true, status: 200,
      text: async () => JSON.stringify({ data: { session: {
        session_id: "fixture/id", attach: { endpoint: "ws://fixture.invalid/cdp", page_url: "https://suno.fixture.invalid/create" },
      } } }),
    };
  });
  const options = {
    input: { easyBrowserBaseUrl: "https://browser.fixture.invalid/", easyBrowserBearerToken: "synthetic-fixture" },
    browserCdpUrl: "ws://explicit.fixture.invalid/cdp", browserCdpTargetUrl: "https://suno.fixture.invalid/create",
    referer: "https://suno.fixture.invalid", timeoutMs: 1000,
  };
  assert.equal((await resolveBrowserExecutionTarget(options)).lease, null);
  assert.equal(calls.length, 0);
  const target = await resolveBrowserExecutionTarget({ ...options, browserCdpUrl: null });
  assert.equal(target.browserCdpUrl, "ws://fixture.invalid/cdp");
  assert.equal(target.browserCdpTargetUrl, "https://suno.fixture.invalid/create");
  assert.equal(calls[0].url, "https://browser.fixture.invalid/v1/browser/sessions/acquire");
  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].headers.authorization, "Bearer synthetic-fixture");
  const body = JSON.parse(calls[0].body);
  assert.equal(body.mode, "direct");
  assert.equal(body.timeout_ms, 1000);
  await releaseEasyBrowserLease(target.lease);
  assert.equal(calls[1].url, "https://browser.fixture.invalid/v1/browser/sessions/fixture%2Fid/release");
  assert.equal(calls[1].method, "POST");
  await releaseEasyBrowserLease(null);
  assert.equal(calls.length, 2);
});

test("CLI validation preserves exit zero plus structured errors before browser access", () => {
  const worker = fileURLToPath(new URL("../suno-browser-worker.mjs", import.meta.url));
  const cases = [
    ["{", "suno_browser_worker_failed"],
    ["[]", "suno_browser_invalid_input"],
    ["{}", "suno_browser_missing_base_url"],
    [JSON.stringify({ baseUrl: "https://suno.fixture.invalid", requestBody: {} }), "suno_browser_missing_prompt"],
  ];
  for (const [input, code] of cases) {
    const child = spawnSync(process.execPath, [worker], { input, encoding: "utf8", timeout: 10000 });
    assert.ifError(child.error);
    assert.equal(child.status, 0);
    const payload = JSON.parse(child.stdout);
    assert.equal(payload.ok, false);
    assert.equal(payload.error.code, code);
  }
});
