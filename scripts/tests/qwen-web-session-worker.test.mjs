import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { probeQwenPage } from "../qwen-web-session/page-probe.mjs";
import { maybeWriteCredentialFile } from "../qwen-web-session/credentials.mjs";
import { loginExistingAccount } from "../qwen-web-session/login.mjs";

const baseUrl = "https://qwen.fixture.invalid";

function probePage(tokens, responses, calls) {
  const localStorage = { ...tokens };
  Object.defineProperty(localStorage, "getItem", { value: (key) => tokens[key] ?? null });
  return {
    evaluate(callback, input) {
      // Playwright serializes this function: no Node-module closure may leak in.
      return vm.runInNewContext(`(${callback.toString()})(input)`, {
        input,
        localStorage,
        atob: (value) => Buffer.from(value, "base64").toString("binary"),
        fetch: async (url, options) => {
          calls.push({ url, ...options });
          const response = responses.shift();
          assert.ok(response, "unexpected request");
          return {
            status: response.status,
            ok: response.status >= 200 && response.status < 300,
            text: async () => JSON.stringify(response.body),
          };
        },
      });
    },
  };
}

test("serialized probe reports missing tokens without fetching", async () => {
  const calls = [];
  const result = await probeQwenPage(probePage({}, [], calls), baseUrl, []);
  assert.equal(result.ok, false);
  assert.equal(result.error.status, 401);
  assert.equal(result.error.code, "qwen_web_session_missing_active_token");
  assert.deepEqual(calls, []);
});

test("serialized probe falls back to token and preserves preferred model/chat contracts", async () => {
  const calls = [];
  const responses = [
    { status: 401, body: {} },
    { status: 200, body: { data: { id: "fixture-user", email: "fixture@example.invalid" } } },
    { status: 200, body: { data: ["first", { code: "preferred", name: "Preferred model" }] } },
    { status: 200, body: { data: { id: "fixture-chat" } } },
  ];
  const jwt = `header.${Buffer.from(JSON.stringify({ exp: 2000000000 })).toString("base64url")}.fixture`;
  const page = probePage({ active_token: "invalid-fixture", token: jwt }, responses, calls);
  const result = await probeQwenPage(page, baseUrl, ["preferred"]);
  assert.equal(result.ok, true);
  assert.equal(result.tokenSource, "token");
  assert.equal(result.authToken, jwt);
  assert.equal(result.expiresAt, new Date(2000000000 * 1000).toISOString());
  assert.equal(result.selectedModel, "preferred");
  assert.equal(result.selectedDisplayModel, "Preferred model");
  assert.equal(result.authProbe.userId, "fixture-user");
  assert.equal(result.createChatProbe.chatId, "fixture-chat");
  assert.equal(calls[0].headers.Authorization, "Bearer invalid-fixture");
  assert.equal(calls[3].url, `${baseUrl}/api/v2/chats/new`);
  assert.equal(calls[3].method, "POST");
  assert.deepEqual(JSON.parse(calls[3].body).models, ["preferred"]);
  assert.equal(responses.length, 0);
});

test("failed auth keeps the first probe status and does not create a chat", async () => {
  const calls = [];
  const page = probePage({ active_token: "fixture" }, [{ status: 403, body: {} }], calls);
  const result = await probeQwenPage(page, baseUrl, []);
  assert.equal(result.ok, false);
  assert.equal(result.error.status, 403);
  assert.equal(calls.length, 1);
});

test("credential writing retains default family, payload, and explicit file extension", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "gateway-qwen-test-"));
  const deps = { parseBoolean: () => false, defaultFamilyDir: "qwen-web-chat" };
  const result = {
    ok: true, authToken: "synthetic-fixture", cookieHeader: "fixture=value",
    selectedModel: "preferred", authProbe: { email: "Fixture@Example.invalid", userId: "user-1" },
  };
  try {
    assert.equal(await maybeWriteCredentialFile({}, result, deps), null);
    assert.equal(await maybeWriteCredentialFile({ credentialRootDir: root }, { ok: false }, deps), null);
    const target = await maybeWriteCredentialFile({ credentialRootDir: root }, result, deps);
    assert.equal(target, path.join(root, "qwen-web-chat", "fixture-example.invalid.json"));
    const bytes = await readFile(target);
    assert.notDeepEqual([...bytes.subarray(0, 3)], [239, 187, 191]);
    const payload = JSON.parse(bytes.toString("utf8"));
    assert.equal(payload.apiKey, result.authToken);
    assert.deepEqual(payload.headers, { Cookie: "fixture=value" });
    assert.deepEqual(payload.supportedModels, ["preferred"]);
    assert.equal(payload.credentialMaterialKey, "qwen-web-user:user-1");
    assert.equal(payload.rawSource.authProbe.email, result.authProbe.email);
    const explicit = path.join(root, "explicit");
    assert.equal(await maybeWriteCredentialFile({ credentialFilePath: explicit }, result, deps), `${explicit}.json`);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("login rejects absent seed/form and preserves existing-account action order", async () => {
  const makeError = (status, code, message) => Object.assign(new Error(message), { status, code });
  await assert.rejects(loginExistingAccount({}, baseUrl, null, 90000, makeError), {
    status: 401, code: "qwen_web_browser_login_seed_missing",
  });
  const actions = [];
  const field = (name) => ({
    click: async () => actions.push(`click:${name}`),
    fill: async (value) => actions.push(`fill:${name}:${value}`),
  });
  const page = {
    goto: async (url, options) => actions.push([url, options.timeout]),
    waitForTimeout: async (timeout) => actions.push(timeout),
    $: async () => null,
    $$: async () => [],
  };
  const seed = { email: "fixture@example.invalid", password: "synthetic-password" };
  await assert.rejects(loginExistingAccount(page, baseUrl, seed, 90000, makeError), {
    code: "qwen_web_browser_login_form_missing",
  });
  actions.length = 0;
  page.$$ = async () => [field("email"), field("password")];
  page.$ = async (selector) => selector.startsWith("button") ? field("submit") : null;
  await loginExistingAccount(page, baseUrl, seed, 90000, makeError);
  assert.deepEqual(actions, [
    [`${baseUrl}/auth`, 60000], 2000, "click:email", `fill:email:${seed.email}`,
    "click:password", `fill:password:${seed.password}`, "click:submit", 8000,
  ]);
});

test("CLI malformed JSON and null input return one structured error without a browser", () => {
  const worker = fileURLToPath(new URL("../qwen-web-session-worker.mjs", import.meta.url));
  for (const input of ["{", "null"]) {
    const child = spawnSync(process.execPath, [worker], { input, encoding: "utf8", timeout: 10000 });
    assert.ifError(child.error);
    assert.equal(child.status, 1);
    const payload = JSON.parse(child.stdout);
    assert.equal(payload.ok, false);
    assert.equal(payload.error.status, 500);
    assert.equal(payload.error.code, "qwen_web_session_worker_failed");
  }
});
