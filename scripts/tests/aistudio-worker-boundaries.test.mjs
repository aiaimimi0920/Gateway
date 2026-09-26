import assert from "node:assert/strict";
import { readFile, mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { existsSync, lstatSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";
import * as settings from "../aistudio-web-browser/settings.mjs";
import * as storage from "../aistudio-web-browser/storage.mjs";
import * as websocket from "../aistudio-web-browser/websocket.mjs";
import * as payload from "../aistudio-web-browser/payload.mjs";
import * as prompts from "../aistudio-web-browser/prompts.mjs";
import * as parser from "../aistudio-web-browser/code-assistant-parser.mjs";

const scripts = fileURLToPath(new URL("..", import.meta.url));
const baselinePath = process.env.AISTUDIO_WORKER_TEST_BASELINE;
const baseline = baselinePath ? await readFile(baselinePath, "utf8") : null;
const names = ["normalizeString", "parseBoolean", "resolveRuntimeStateSource",
  "createWebSocketTextFrame", "parseWebSocketFrames", "isGenerateContentRequestSpec",
  "buildPromptFromGenerateContentBody", "parseToolCallsFromAssistantText",
  "buildDeterministicToolCallResponseFromGenerateContentBody", "extractFinalTextFromCodeAssistantStream"];
// Baseline declarations are evaluated without invoking main or browser/network code.
const api = baseline
  ? vm.runInNewContext(baseline.replace(/^import .*;\r?\n/gm, "")
      .replace(/\bmain\(\);\s*$/, "") + `\n({${names.join(",")}})`,
    { Buffer, process, path, existsSync, lstatSync, mkdir, writeFile })
  : { ...settings, ...storage, ...websocket, ...payload, ...prompts, ...parser };

test("AI Studio text routing excludes media and speech generation", () => {
  const url = "https://example.test/v1beta/models/gemini-test:generateContent";
  assert.equal(api.isGenerateContentRequestSpec({ url, body: "{}" }), true);
  assert.equal(api.isGenerateContentRequestSpec({ url: "https://example.test/models" }), false);
  for (const generationConfig of [{ responseModalities: ["IMAGE"] }, { speechConfig: {} }]) {
    assert.equal(api.isGenerateContentRequestSpec({ url, body: JSON.stringify({ generationConfig }) }), false);
  }
});

test("AI Studio prompt and stream parsers preserve text ordering", () => {
  const body = JSON.stringify({ contents: [{ role: "user", parts: [{ text: "hello" }, { text: "world" }] }] });
  assert.equal(api.buildPromptFromGenerateContentBody(body), "hello\nworld");
  assert.equal(api.buildPromptFromGenerateContentBody("invalid"), null);
  assert.equal(api.extractFinalTextFromCodeAssistantStream(JSON.stringify([
    [["old"], "model"], [["first", "last"], "model"],
  ])), "last");
});

test("AI Studio explicit tool choice retains synthetic function-call schema", () => {
  const response = api.buildDeterministicToolCallResponseFromGenerateContentBody(JSON.stringify({
    contents: [{ role: "user", parts: [{ text: "Weather in Hangzhou" }] }],
    tools: [{ functionDeclarations: [{ name: "weather", parameters: { properties: { city: { type: "string" } } } }] }],
    toolConfig: { functionCallingConfig: { mode: "ANY", allowedFunctionNames: ["weather"] } },
  }), "gemini-test");
  const result = JSON.parse(response);
  assert.equal(result.modelVersion, "gemini-test");
  assert.deepEqual(result.candidates[0].content.parts, [{ functionCall: {
    id: "call_weather", name: "weather", args: { city: "Hangzhou" },
  } }]);
  const parsed = api.parseToolCallsFromAssistantText('before <tool_calls><tool_call><tool_name>weather</tool_name><parameters>invalid</parameters></tool_call></tool_calls>');
  assert.equal(parsed.cleanText, "before");
  assert.equal(JSON.stringify(parsed.toolCalls), '[{"id":"call_weather","name":"weather","args":{}}]');
});

test("AI Studio settings preserve string and boolean defaults", () => {
  assert.equal(api.normalizeString("  value  "), "value");
  assert.equal(api.normalizeString(1), null);
  for (const value of ["1", "TRUE", " yes ", "on"]) assert.equal(api.parseBoolean(value, false), true);
  for (const value of ["0", "FALSE", " no ", "off"]) assert.equal(api.parseBoolean(value, true), false);
  assert.equal(api.parseBoolean("unknown", true), true);
});

for (const length of [0, 125, 126, 65535, 65536]) {
  test(`AI Studio WebSocket text frame round trip at ${length} bytes`, () => {
    const text = "x".repeat(length);
    const encoded = api.createWebSocketTextFrame(text);
    const parsed = api.parseWebSocketFrames(encoded);
    assert.equal(parsed.frames.length, 1);
    assert.equal(parsed.frames[0].opcode, 1);
    assert.equal(parsed.frames[0].payload.toString("utf8"), text);
    assert.equal(parsed.rest.length, 0);
    const partial = encoded.subarray(0, encoded.length - 1);
    const pending = api.parseWebSocketFrames(partial);
    assert.equal(pending.frames.length, 0);
    assert.equal(Buffer.compare(pending.rest, partial), 0);
  });
}

test("AI Studio storage distinguishes profile, JSON, missing and invalid paths", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "aistudio-state-contract-"));
  const keys = ["AI_GATEWAY_OBJECT_STORAGE_DRIVER", "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR"];
  const old = keys.map((key) => process.env[key]);
  try {
    process.env[keys[0]] = "local";
    process.env[keys[1]] = root;
    await mkdir(path.join(root, "profile"));
    await writeFile(path.join(root, "state.json"), "{}", "utf8");
    await writeFile(path.join(root, "state.txt"), "{}", "utf8");
    assert.equal((await api.resolveRuntimeStateSource("profile")).mode, "profile_dir");
    assert.equal((await api.resolveRuntimeStateSource("state.json")).mode, "storage_state_file");
    await assert.rejects(api.resolveRuntimeStateSource("missing"), { code: "aistudio_runtime_state_unavailable" });
    await assert.rejects(api.resolveRuntimeStateSource("state.txt"), { code: "aistudio_runtime_state_invalid_path" });
  } finally {
    keys.forEach((key, index) => old[index] === undefined ? delete process.env[key] : process.env[key] = old[index]);
    await rm(root, { recursive: true, force: true });
  }
});

test("AI Studio CLI validates input before any browser launch", () => {
  const baselineLoader = `
    const { readFile } = await import('node:fs/promises');
    let source = await readFile(process.env.AISTUDIO_WORKER_TEST_BASELINE, 'utf8');
    source = source.replace(/from "(playwright-core|@aws-sdk\\/client-s3)"/g,
      (_, name) => 'from ' + JSON.stringify(import.meta.resolve(name)));
    await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
  `;
  const args = baseline ? ["--input-type=module", "--eval", baselineLoader]
    : [path.join(scripts, "aistudio-web-browser-worker.mjs")];
  const result = spawnSync(process.execPath, args, { cwd: scripts, input: "{}", encoding: "utf8", timeout: 10_000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 1);
  const payload = JSON.parse(result.stdout);
  assert.equal(payload.ok, false);
  assert.equal(payload.error.message, "runtimeStateObjectKey is required.");
});
