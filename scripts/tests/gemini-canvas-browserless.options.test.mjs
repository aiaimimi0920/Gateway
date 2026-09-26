import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import test from "node:test";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();
async function invoke(args = {}, state = {}, handle = {}, missingStorage = false) {
  const writes = [], copies = [], contexts = [], output = [], calls = [];
  const fakeProcess = { argv: [], exitCode: 0, stdout: { write(text) { output.push(text); } } };
  const browserState = { ...state };
  const reply = async (context) => { contexts.push(context); return { ok: true, kind: "fixture", status: 200 }; };
  const deps = { ...app, path, fileURLToPath, process: fakeProcess,
    parseArgs: () => ({ "browser-state": "fixture-browser.json", "out-dir": "fixture-output", ...args }),
    nowStamp: () => "fixture", fileExists: () => true,
    async readJson() { return browserState; },
    async resolveProgramHandle(value) { calls.push(["handle", value.__path]); return { path: "fixture-handle.json", json: handle }; },
    resolveProfileDir: () => "fixture-profile", resolveStorageStatePath: () => missingStorage ? null : "fixture-storage.json",
    async ensureDir() {}, copyFileSync(...values) { copies.push(values); },
    async writeJson(name, value) { writes.push({ name: path.basename(name), value }); },
    buildPureHttpSession: () => ({ marker: "fixture-session" }),
    probeText: reply, probeTts: reply, probeImage: reply, probeMusic: reply, probeVideoCreate: reply,
  };
  const source = app.main.toString();
  assert.equal(source.split("import.meta.url").length, 2);
  const main = vm.runInNewContext("(" + source.replace("import.meta.url", JSON.stringify(import.meta.url)) + ")", deps, { timeout: 1000 });
  await main();
  return { material: writes.find((item) => item.name === "resolved-material.json").value,
    summary: writes.find((item) => item.name === "summary.json").value,
    context: contexts[0], writes, copies, calls, output, fakeProcess };
}

for (const [operation, aliases] of [["text", ["text", "chat", "", null]], ["tts", ["tts", "audio-speech", "speech"]], ["image", ["image", "image-create", "images"]], ["music", ["music", "music-create", "audio"]], ["video-create", ["video", "video-create", "videos"]]]) {
  test("browserless options normalize aliases for " + operation, () => {
    for (const alias of aliases) assert.equal(app.normalizeOperation(alias == null ? alias : " " + alias.toUpperCase() + " "), operation);
  });
}

test("browserless options unknown operations retain normalized value and default model", async () => {
  assert.equal(app.normalizeOperation(" UNKNOWN "), "unknown");
  const r = await invoke({ operation: "unknown", prompt: "fixed" });
  assert.equal(r.material.operation, "unknown");
  assert.equal(r.material.model, "gemini-3-flash-preview");
  assert.equal(r.context, undefined);
  assert.equal(r.summary.error, "unsupported_operation");
  assert.equal(r.fakeProcess.exitCode, 1);
});

test("browserless options URLs preserve field precedence and invalid origin fallback", () => {
  const state = { baseUrl: " https://base.test/custom/ ", base_url: "https://snake.test", shareUrl: "https://share.test/x", canvasProgramUrl: "https://canvas.test/x", pageUrl: "https://page.test/x" };
  assert.equal(app.inferBaseUrl(state), "https://base.test/custom/");
  delete state.baseUrl; assert.equal(app.inferBaseUrl(state), "https://snake.test");
  delete state.base_url; assert.equal(app.inferBaseUrl(state), "https://share.test");
  state.shareUrl = "bad"; assert.equal(app.inferBaseUrl(state), "https://canvas.test");
  state.canvasProgramUrl = "bad"; assert.equal(app.inferBaseUrl(state), "https://page.test");
  state.pageUrl = "bad"; assert.equal(app.inferBaseUrl(state), "https://gemini.google.com");
  assert.equal(app.originFromUrl("bad"), null);
  assert.equal(app.inferApiBaseUrl({ apiBaseUrl: " camel ", api_base_url: "snake" }, { "api-base-url": " explicit " }), "explicit");
  assert.equal(app.inferApiBaseUrl({ apiBaseUrl: " camel ", api_base_url: "snake" }, {}), "camel");
  assert.equal(app.inferApiBaseUrl({ api_base_url: "snake" }, {}), "snake");
});

test("browserless options main archives matching material and operation context", async () => {
  const r = await invoke({ "base-url": "https://base.test///", "api-base-url": "https://api.test", "page-referer": "https://refer.test/x", locale: " fr ", prompt: " CLI prompt ", model: "custom", voice: " voice ", "api-key": "synthetic-key", "timeout-ms": "7000", "video-poll-timeout-ms": "6000" }, { canvasProgramInvokeContract: { operation: "speech", prompt: "material" } }, { bootstrap: { language: "de" } });
  const expected = { baseUrl: "https://base.test///", apiBaseUrl: "https://api.test", pageReferer: "https://refer.test/x", pageOrigin: "https://refer.test", locale: "fr", operation: "tts", prompt: "CLI prompt", model: "custom", voiceName: "voice", aspectRatio: "1:1", durationSeconds: null };
  for (const [key, value] of Object.entries(expected)) { assert.equal(r.material[key], value, key); assert.equal(r.context[key], value, key); }
  assert.equal(r.context.apiKey, "synthetic-key"); assert.equal(r.material.googleApiKeyPresent, true);
  assert.equal(r.context.timeoutMs, 7000); assert.equal(r.context.videoPollTimeoutMs, 7000);
  assert.deepEqual(r.writes.map((x) => x.name), ["resolved-material.json", "summary.json"]);
  assert.equal(r.copies.length, 2); assert.equal(r.calls[0][1], r.material.browserStatePath);
  assert.equal(r.summary.ok, true); assert.equal(r.fakeProcess.exitCode, 0);
});

test("browserless options prompt matching follows normalized material operation", async () => {
  const state = { canvasProgramInvokeContract: { operation: "chat", prompt: " material prompt " } };
  assert.equal((await invoke({}, state)).material.prompt, "material prompt");
  const changed = await invoke({ operation: "video" }, state);
  assert.equal(changed.material.prompt, "A four second shot of a red cube slowly rotating on a white table.");
  assert.equal(changed.material.model, "veo-3.1-generate-preview");
  assert.equal(changed.material.aspectRatio, "16:9"); assert.equal(changed.material.durationSeconds, 4);
});

test("browserless options defaults retain referer locale and runtime prompt generation", async () => {
  const r = await invoke({ "base-url": "https://base.test///" });
  assert.equal(r.material.pageReferer, "https://base.test/canvas");
  assert.equal(r.material.pageOrigin, "https://base.test"); assert.equal(r.material.locale, "zh-CN");
  assert.match(r.material.prompt, /^BROWSERLESS_CANVAS_TEXT_[0-9]+ Reply with exactly: ok$/);
  assert.equal(r.context.timeoutMs, 120000); assert.equal(r.context.videoPollTimeoutMs, 600000);
  assert.equal(r.material.googleApiKeyPresent, false); assert.equal(r.context.apiKey, null);
  for (const [operation, prefix] of [["tts", "TTS"], ["image", "IMAGE"], ["music", "MUSIC"]]) assert.match(app.defaultPromptForOperation(operation), new RegExp("^BROWSERLESS_CANVAS_" + prefix + "_[0-9]+ "));
});

test("browserless options material fields keep referer language aspect and key fallbacks", async () => {
  const state = { canvasProgramUrl: "https://canvas.test/x", pageUrl: "https://page.test/x", shareUrl: "https://share.test/x", googleApiKey: " google ", apiKeys: ["array"], canvasProgramInvokeContract: { aspectRatio: " 4:3 ", aspect_ratio: "2:1", durationSeconds: "2.5", duration_seconds: "9" } };
  const r = await invoke({ prompt: "fixed" }, state, { bootstrap: { language: " en " } });
  assert.equal(r.material.pageReferer, state.canvasProgramUrl); assert.equal(r.material.locale, "en");
  assert.equal(r.material.aspectRatio, "4:3"); assert.equal(r.material.durationSeconds, 2.5); assert.equal(r.context.apiKey, "google");
  const snake = await invoke({ prompt: "fixed" }, { apiKeys: [null, " ", " first ", "second"], canvasProgramInvokeContract: { aspect_ratio: "2:1", duration_seconds: "3" } });
  assert.equal(snake.material.aspectRatio, "2:1"); assert.equal(snake.material.durationSeconds, 3); assert.equal(snake.context.apiKey, "first");
  const bad = await invoke({ "base-url": "bad", "page-referer": "also bad", prompt: "fixed" });
  assert.equal(bad.material.pageOrigin, "https://gemini.google.com");
});

for (const value of ["0", "invalid", ""]) {
  test("browserless options duration " + JSON.stringify(value) + " retains null and minimum timeout", async () => {
    const r = await invoke({ "duration-seconds": value, "timeout-ms": "1", prompt: "fixed" });
    assert.equal(r.material.durationSeconds, null); assert.equal(r.context.timeoutMs, 5000);
  });
}

test("browserless options invalid timeouts retain existing NaN semantics", async () => {
  const r = await invoke({ "timeout-ms": "bad", "video-poll-timeout-ms": "bad", prompt: "fixed" });
  assert.equal(Number.isNaN(r.context.timeoutMs), true); assert.equal(Number.isNaN(r.context.videoPollTimeoutMs), true);
});

test("browserless options missing storage archives resolved material before failure", async () => {
  const r = await invoke({ prompt: "fixed" }, {}, {}, true);
  assert.equal(r.context, undefined); assert.equal(r.summary.error, "missing_storage_state");
  assert.deepEqual(r.writes.map((x) => x.name), ["resolved-material.json", "summary.json"]);
  assert.equal(r.fakeProcess.exitCode, 1); assert.equal(r.output.length, 1);
});
