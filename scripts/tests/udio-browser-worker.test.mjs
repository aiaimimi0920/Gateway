import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import * as fs from "node:fs";
import * as io from "node:fs/promises";
import path from "node:path";
import os from "node:os";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const entry = fileURLToPath(new URL("../udio-browser-worker.mjs", import.meta.url));
async function loadApi() {
  if (process.env.UDIO_WORKER_TEST_BASELINE) {
    const source = await io.readFile(process.env.UDIO_WORKER_TEST_BASELINE, "utf8");
    const context = { ...fs, ...io, path, Buffer, URL, setTimeout, clearTimeout,
      process: { env: {}, cwd: () => process.cwd(), platform: process.platform } };
    vm.createContext(context);
    vm.runInContext(source.replace(/^import .*;\r?\n/gm, "").replace(/^main\(\);\s*$/m, ""), context);
    return context;
  }
  const modules = await Promise.all(["request", "responses", "auth", "storage", "browser", "transport"]
    .map((name) => import("../udio-browser/" + name + ".mjs")));
  return Object.assign({}, ...modules);
}
const api = await loadApi();

test("request validation rejects shape, base URL, body, asset kind and missing authentication", () => {
  for (const [input, code] of [
    [null, "udio_browser_invalid_input"], [[], "udio_browser_invalid_input"],
    [{}, "udio_browser_missing_base_url"],
    [{ baseUrl: "https://www.udio.com" }, "udio_browser_missing_request_body"],
    [{ baseUrl: "https://www.udio.com", requestBody: {}, targetAssetKind: "other" }, "udio_browser_invalid_target_asset_kind"],
    [{ baseUrl: "https://www.udio.com", requestBody: {} }, "udio_browser_missing_runtime_auth"],
  ]) assert.throws(() => api.validateInput(input), (error) => error.code === code);
});

test("asset readiness requires requested assets, not readyToStream flags", () => {
  assert.equal(api.songsReadyForTarget([], "audio"), false);
  assert.equal(api.songsReadyForTarget([{ readyToStream: true }], "audio"), false);
  assert.equal(api.songsReadyForTarget([{ audio_url: "synthetic" }], "audio"), true);
  assert.equal(api.songsReadyForTarget([{ cover_art_url: "synthetic" }], "image"), true);
  assert.equal(api.songsReadyForTarget([{ videoPath: "synthetic" }], "video"), true);
  assert.equal(api.songsReadyForTarget([{ audio_url: "synthetic" }, {}], "audio"), false);
  assert.equal(JSON.stringify(api.extractTrackIds({ track_ids: [" a ", ""] })), '["a"]');
  assert.equal(JSON.stringify(api.extractTrackIds({ songs: [{ id: " b " }] })), '["b"]');
  assert.throws(() => api.extractTrackIds({}), (error) => error.code === "udio_missing_track_ids");
});

test("HTTP error classification preserves unauthorized, captcha and invalid JSON cases", () => {
  for (const [outcome, code] of [
    [{ ok: false, status: 401, text: "" }, "udio_session_unauthorized"],
    [{ ok: false, status: 403, text: "captcha required" }, "udio_captcha_verification_failed"],
    [{ ok: false, status: 403, text: "user disallowed" }, "udio_browser_challenge_required"],
    [{ ok: true, status: 200, text: "{" }, "invalid_json"],
  ]) assert.throws(() => api.requireJsonResponse(outcome, "invalid_json", "Invalid JSON"),
    (error) => error.code === code);
  assert.equal(api.isChallengeOutcome({ status: 403, text: "user disallowed" }), true);
  assert.equal(api.isChallengeOutcome({ status: 200, text: "user disallowed" }), false);
  assert.equal(api.trimBody("x".repeat(9000)).length, 8000);
});

test("chunked auth cookies are ordered and decoded without printing tokens", () => {
  const encoded = "base64-" + Buffer.from(JSON.stringify({ access_token: "synthetic" })).toString("base64url");
  const middle = Math.floor(encoded.length / 2);
  assert.equal(api.extractSupabaseAccessTokenFromCookies([
    { name: "sb-ssr-production-auth-token.1", value: encoded.slice(middle) },
    { name: "sb-ssr-production-auth-token.0", value: encoded.slice(0, middle) },
  ]), "synthetic");
  assert.equal(api.extractSupabaseAccessTokenFromCookies([]), null);
});

test("auth fallback detaches CDP session and does not evaluate storage after cookie success", async () => {
  const calls = [];
  const page = {
    context: () => ({
      cookies: async (...args) => { calls.push(args.length ? "filtered" : "all"); return []; },
      newCDPSession: async () => ({
        send: async (name) => { calls.push(name); return { cookies: [
          { name: "sb-ssr-production-auth-token", domain: ".udio.com", value: '{"access_token":"synthetic"}' },
        ] }; },
        detach: async () => { calls.push("detach"); },
      }),
    }),
    evaluate: async () => { throw new Error("Unexpected page evaluation"); },
  };
  assert.equal(await api.resolveUdioAccessToken(page, "https://www.udio.com"), "synthetic");
  assert.deepEqual(calls, ["filtered", "all", "Network.getAllCookies", "detach"]);
});

test("empty state cannot replace authenticated runtime state", () => {
  const authenticated = { cookies: [{ name: "sb-ssr-production-auth-token", value: "synthetic" }] };
  assert.equal(api.hasPersistableRuntimeState(authenticated), true);
  assert.equal(api.shouldPersistRuntimeState(authenticated, { cookies: [] }), false);
  assert.equal(api.shouldPersistRuntimeState({}, authenticated), true);
});

test("borrowed browser page is reused without navigation or new page", async () => {
  const calls = [];
  const page = { url: () => "https://www.udio.com/create/",
    waitForLoadState: async () => { calls.push("load"); },
    waitForTimeout: async () => { calls.push("wait"); } };
  const context = { pages: () => [page], newPage: async () => { throw new Error("Unexpected new page"); } };
  assert.equal(await api.resolveWorkerPage(context, {
    targetUrl: "https://www.udio.com/create", borrowedContext: true, navigationTimeoutMs: 10,
  }), page);
  assert.deepEqual(calls, ["load", "wait"]);
  await assert.rejects(api.resolveWorkerPage({ pages: () => [] }, {
    targetUrl: "https://www.udio.com/create", borrowedContext: true,
  }), /does not contain/);
});

test("challenge callback runs once and transport failure does not retry", async () => {
  let attempts = 0;
  let challenges = 0;
  const result = await api.runWithChallengeRetries({
    overallDeadline: Date.now() + 1000, retryIntervalMs: 1,
    action: async () => ++attempts < 3
      ? { status: 403, text: "user disallowed" } : { ok: true, status: 200, text: "{}" },
    onChallenge: async () => { challenges += 1; },
  });
  assert.equal(result.status, 200);
  assert.equal(attempts, 3);
  assert.equal(challenges, 1);
  await assert.rejects(api.runWithChallengeRetries({
    overallDeadline: Date.now() + 1000,
    action: async () => ({ transportError: true, status: 504, text: "synthetic" }),
  }), (error) => error.code === "udio_browser_transport_failed");
});

test("safe invalid stdin exits with the preserved JSON contract without launching a browser", () => {
  for (const [input, code] of [
    ["null", "udio_browser_invalid_input"], ["[]", "udio_browser_invalid_input"],
    ["{}", "udio_browser_missing_base_url"],
    ['{"baseUrl":"https://www.udio.com"}', "udio_browser_missing_request_body"],
  ]) {
    const result = spawnSync(process.execPath, [entry], {
      input, encoding: "utf8", timeout: 10000,
      env: { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot },
    });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0);
    assert.equal(JSON.parse(result.stdout).error.code, code);
  }
});

for (const [scenario, ok, expectedCalls] of [
  ["owned-success", true, ["persist", "close"]],
  ["owned-request-error", false, ["persist", "close"]],
  ["persist-error", true, ["persist", "close"]],
  ["borrowed-success", true, ["persist"]],
  ["borrowed-lookalike", true, ["persist"]],
  ["borrowed-missing", false, []],
  ["launch-error", false, []],
  ["context-error", false, ["close"]],
  ["close-error", true, ["persist", "close"]],
]) {
test("CLI cleanup before exit: " + scenario, async (t) => {
  const root = await io.mkdtemp(path.join(os.tmpdir(), "udio-lifecycle-contract-"));
  t.after(() => io.rm(root, { recursive: true, force: true }));
  const marker = path.join(root, "lifecycle.json");
  const child = `
    import vm from "node:vm";
    import { readFileSync, writeFileSync } from "node:fs";
    import { pathToFileURL } from "node:url";
    const entry = ${JSON.stringify(entry)};
    const marker = ${JSON.stringify(marker)};
    const scenario = ${JSON.stringify(scenario)};
    const bindings = {};
    for (const name of ["request", "responses", "auth", "storage", "browser", "transport", "diagnostics", "captcha", "native-flow"]) {
      Object.assign(bindings, await import(new URL("./udio-browser/" + name + ".mjs", pathToFileURL(entry))));
    }
    const calls = [];
    const record = (name) => { calls.push(name); writeFileSync(marker, JSON.stringify(calls)); };
    const context = { pages: () => [{ url: () => "https://www.udio.com/create" }] };
    const outcome = { ok: true, status: 200,
      text: '{"track_ids":["synthetic"],"songs":[{"audio_url":"synthetic"}]}' };
    const browser = {
      contexts: () => {
        const unrelated = { pages: () => [{ url: () => "https://www.udio.com.attacker.test/create" }] };
        return scenario === "borrowed-lookalike" ? [unrelated, context]
          : scenario === "borrowed-missing" ? [unrelated] : [context];
      },
      newContext: async () => {
        if (scenario === "context-error") throw new Error("synthetic context error");
        return context;
      },
      close: async () => {
        record("close");
        if (scenario === "close-error") throw new Error("synthetic close error");
      },
    };
    Object.assign(bindings, {
      process, Buffer, URL,
      chromium: {
        launch: async () => {
          if (scenario === "launch-error") throw new Error("synthetic launch error");
          return browser;
        },
        connectOverCDP: async () => browser,
      },
      resolveExecutablePath: () => "synthetic-browser",
      maybeReadRuntimeState: async () => ({ cookies: [{ name: "sb-ssr-production-auth-token" }] }),
      maybePersistRuntimeState: async () => {
        record("persist");
        if (scenario === "persist-error") throw new Error("synthetic persist error");
      },
      createWorkerPage: async () => {
        if (scenario === "owned-request-error") throw new Error("synthetic request error");
        return {};
      },
      resolveWorkerPage: async (selected) => {
        if (selected !== context) throw new Error("Wrong borrowed context selected");
        return {};
      },
      submitGenerationThroughPageUi: async () => outcome,
      resolveUdioAccessToken: async () => "synthetic",
      debugLog: async () => {},
      browserFetch: async (_page, args) => ({
        ok: true, status: 200,
        text: args.requestUrl.endsWith("/captcha") ? '{"required":false}'
          : outcome.text,
      }),
    });
    vm.createContext(bindings);
    const source = readFileSync(entry, "utf8").replace(/^import[\\s\\S]*?from "[^"]+";\\r?\\n/gm, "");
    vm.runInContext(source, bindings);
  `;
  const result = spawnSync(process.execPath, ["--input-type=module", "-e", child], {
    input: JSON.stringify({ baseUrl: "https://www.udio.com", requestBody: {},
      runtimeStateObjectKey: "synthetic-state", waitAudio: false,
      browserCdpUrl: scenario.startsWith("borrowed-") ? "http://127.0.0.1:1" : undefined }),
    encoding: "utf8", timeout: 10000,
    env: { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot },
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).ok, ok);
  const calls = fs.existsSync(marker) ? JSON.parse(await io.readFile(marker, "utf8")) : [];
  assert.deepEqual(calls, expectedCalls);
});
}
