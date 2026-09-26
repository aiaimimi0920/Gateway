import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

test("header restoration filters noise and retains final values in a real cookie jar", browserTest, async (t) => {
  const { context } = await appFixture(t);
  const header = " SID = superseded ; malformed ; =invalid ; SID = synthetic=a=b ; HSID= ; NID=synthetic-nid ; COMPASS=synthetic-compass ; unknown=synthetic-ignored ; __Host-SID=synthetic-ignored ";
  assert.equal(await app.syncCookieHeaderIntoContext(context, header, baseUrl), 3);
  const stored = await context.cookies([baseUrl]);
  assert.deepEqual(Object.fromEntries(stored.map((cookie) => [cookie.name, cookie.value])), {
    SID: "synthetic=a=b", NID: "synthetic-nid", COMPASS: "synthetic-compass",
  });
  assert.ok(stored.every((cookie) => cookie.secure && cookie.domain === ".google.com" && cookie.path === "/"));
  assert.equal(await app.contextHasGeminiAuthCookies(context, baseUrl), true);
});

test("non-auth cookies remain insufficient until a secure auth cookie is restored", browserTest, async (t) => {
  const { context } = await appFixture(t);
  assert.equal(await app.contextHasGeminiAuthCookies(context, baseUrl), false);
  assert.equal(await app.syncCookieHeaderIntoContext(context, "NID=synthetic-nid; COMPASS=synthetic-compass", baseUrl), 2);
  assert.equal(await app.contextHasGeminiAuthCookies(context, baseUrl), false);
  assert.equal(await app.syncCookieHeaderIntoContext(context, "__Secure-3PSID=synthetic-auth", baseUrl), 1);
  assert.equal(await app.contextHasGeminiAuthCookies(context, baseUrl), true);
});

test("embedded hydration restores valid empty cookies without applying stored origins", browserTest, async (t) => {
  const profile = storageFixture(t), { context, page } = await appFixture(t);
  const file = path.join(profile, "storage-state.json"), source = JSON.stringify({
    cookies: [null, { name: "invalid", value: 3, domain: ".google.com" },
      { name: "fixture-empty", value: "", domain: ".google.com", sameSite: "sTRiCT", expires: "invalid" }],
    origins: [{ origin: baseUrl, localStorage: [{ name: "stored-origin", value: "synthetic-origin" }] }],
  });
  fs.writeFileSync(file, source);
  assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, profile), 1);
  const [stored] = await context.cookies([baseUrl]);
  assert.equal(stored.name, "fixture-empty");
  assert.equal(stored.value, "");
  assert.equal(stored.path, "/");
  assert.equal(stored.sameSite, "Strict");
  assert.equal(stored.expires, -1);
  assert.equal(await page.evaluate(() => localStorage.getItem("stored-origin")), null);
  assert.equal(fs.readFileSync(file, "utf8"), source);
});
