import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { exportMaterial } from "../chatgpt-web-import-extension/material.mjs";
import { captureSelectedTab } from "../chatgpt-web-import-extension/capture.mjs";

const token = exp => `e30.${Buffer.from(JSON.stringify({ exp })).toString("base64url")}.sig`;
const session = { accessToken: token(4070908800), userAgent: "Browser/1", refreshToken: "must-not-export", expires: "2100-01-01" };
const cookies = [{ name: "oai-did", value: "device", domain: ".chatgpt.com" },
  { name: "session", value: "synthetic-session", domain: "chatgpt.com", httpOnly: true },
  { name: "google", value: "must-not-export", domain: ".google.com" }];

test("extension has ChatGPT-only permissions and no automatic content or background capture", async () => {
  const manifest = JSON.parse(await readFile(new URL("../chatgpt-web-import-extension/manifest.json", import.meta.url), "utf8"));
  assert.deepEqual(manifest.permissions, ["cookies", "scripting"]);
  assert.deepEqual(manifest.host_permissions, ["https://chatgpt.com/*"]);
  for (const key of ["background", "content_scripts", "externally_connectable"]) assert.equal(manifest[key], undefined);
});
test("export whitelists Web secrets and uses token expiry instead of webpage expiry", () => {
  const material = exportMaterial(session, cookies);
  assert.equal(material.expiresAt, "2099-01-01T00:00:00.000Z");
  assert.equal(material.cookieHeader, "oai-did=device; session=synthetic-session");
  assert.equal(JSON.stringify(material).includes("must-not-export"), false);
  assert.deepEqual(Object.keys(material).sort(), ["authToken", "cookieHeader", "deviceId", "expiresAt", "userAgent"]);
});
test("export rejects expired, oversized, malformed and incomplete material", () => {
  assert.throws(() => exportMaterial({ ...session, accessToken: token(1) }, cookies));
  assert.throws(() => exportMaterial({ ...session, accessToken: "x".repeat(32769) }, cookies));
  assert.throws(() => exportMaterial(session, []));
  assert.throws(() => exportMaterial(session, [{ name: "oai-did", value: "bad\tvalue", domain: "chatgpt.com" }]));
});
test("capture selects the active tab cookie store and never reads Google", async () => {
  const api = {
    tabs: { query: async () => [{ id: 7, url: "https://chatgpt.com/" }], get: async () => ({ url: "https://chatgpt.com/" }) },
    scripting: { executeScript: async request => {
      assert.equal(request.target.tabId, 7); assert.equal(request.world, "ISOLATED");
      return [{ result: session }];
    } },
    cookies: { getAllCookieStores: async () => [{ id: "wrong", tabIds: [1] }, { id: "selected", tabIds: [7] }],
      getAll: async request => { assert.deepEqual(request, { url: "https://chatgpt.com/", storeId: "selected" }); return cookies; } },
  };
  assert.equal((await captureSelectedTab(api)).deviceId, "device");
  api.tabs.get = async () => ({ url: "https://chatgpt.com/other" });
  await assert.rejects(captureSelectedTab(api), /tab_changed/);
  api.tabs.query = async () => [{ id: 7, url: "https://accounts.google.com/" }];
  await assert.rejects(captureSelectedTab(api), /select_chatgpt/);
});
