import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { probeSession } from "../chataibot-session/probe.mjs";

// Execute the serialized browser function, so Node imports cannot mask a
// closure dependency that would fail inside Playwright's page.evaluate.
const source = probeSession.toString();

function token(payload) {
  return `header.${Buffer.from(JSON.stringify(payload)).toString("base64url")}.signature`;
}

async function probe({ storage = {}, cookieToken = null, rows = [], body = "{}", status = 200 } = {}) {
  const calls = [];
  let closes = 0;
  const localStorage = { ...storage };
  Object.defineProperty(localStorage, "getItem", { value: (key) => storage[key] ?? null });
  const indexedDB = {
    databases: async () => [{ name: "session", version: 1 }],
    open() {
      const request = {};
      queueMicrotask(() => {
        request.result = {
          objectStoreNames: ["auth"],
          close() { closes += 1; },
          transaction() {
            return { objectStore: () => ({ openCursor() {
              const cursorRequest = {};
              let index = 0;
              const advance = () => queueMicrotask(() => {
                cursorRequest.result = index < rows.length
                  ? { value: rows[index++], continue: advance } : null;
                cursorRequest.onsuccess();
              });
              advance();
              return cursorRequest;
            } }) };
          },
        };
        request.onsuccess();
      });
      return request;
    },
  };
  const browser = vm.createContext({
    localStorage, indexedDB, atob,
    fetch: async (url, init) => {
      calls.push({ url, init });
      return { status, ok: status < 400, text: async () => body };
    },
  });
  const callback = vm.runInContext(`(${source})`, browser);
  const result = await callback({ preferredModels: ["qwen-lora", "gpt-image-1.5"], cookieToken });
  return { result: JSON.parse(JSON.stringify(result)), calls, closes };
}

test("missing authentication fails without making a quota request and closes IndexedDB", async () => {
  const { result, calls, closes } = await probe();
  assert.equal(result.ok, false);
  assert.equal(result.error.status, 401);
  assert.equal(result.error.code, "chataibot_session_missing_token_cookie");
  assert.equal(calls.length, 0);
  assert.equal(closes, 1);
});

test("cookie wins over both stores and preserves account, expiry, model and quota mapping", async () => {
  const cookieToken = token({ exp: 1700000000, sub: "user-7", email: "fixture@example.test" });
  const { result, calls, closes } = await probe({
    cookieToken, storage: { token: token({ sub: "other" }), models: '["gpt-image-1.5"]' },
    rows: [{ auth: token({ sub: "db" }) }], body: '{"data":{"leftAnswersCount":0}}',
  });
  assert.equal(result.authToken, cookieToken);
  assert.equal(result.tokenSource, "cookie:token");
  assert.equal(result.userId, "user-7");
  assert.equal(result.accountName, "fixture@example.test");
  assert.equal(result.expiresAt, "2023-11-14T22:13:20.000Z");
  assert.equal(result.selectedModel, "gpt-image-1.5");
  assert.deepEqual(result.availableModels, ["gpt-image-1.5", "qwen-lora"]);
  assert.equal(result.quotaProbe.leftAnswersCount, 0);
  assert.equal(calls[0].url, "/api/user/answers-count/v2");
  assert.equal(calls[0].init.credentials, "include");
  assert.equal(closes, 1);
});

test("preferred localStorage key precedes arbitrary keys and IndexedDB", async () => {
  const preferred = token({ username: "preferred" });
  const { result } = await probe({
    storage: { unrelated: token({ sub: "first" }), active_token: preferred },
    rows: [token({ sub: "db" })],
  });
  assert.equal(result.authToken, preferred);
  assert.equal(result.tokenSource, "localStorage:active_token");
  assert.equal(result.selectedModel, "qwen-lora");
});

test("IndexedDB nested token survives a non-JSON failed quota response", async () => {
  const dbToken = token({ userId: "db-user" });
  const { result, closes } = await probe({ rows: [{ nested: [dbToken] }], status: 503, body: "offline" });
  assert.equal(result.ok, true);
  assert.equal(result.authToken, dbToken);
  assert.equal(result.tokenSource, "indexedDB");
  assert.deepEqual(result.quotaProbe, { status: 503, ok: false, leftAnswersCount: null, message: "offline" });
  assert.equal(closes, 1);
});

test("IndexedDB scans at most forty rows per store", async () => {
  const rows = Array.from({ length: 40 }, () => ({ ignored: true }));
  rows.push({ token: token({ sub: "too-late" }) });
  const { result, closes } = await probe({ rows });
  assert.equal(result.ok, false);
  assert.equal(closes, 1);
});
