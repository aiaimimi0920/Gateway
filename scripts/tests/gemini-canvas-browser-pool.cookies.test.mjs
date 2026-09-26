import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

const app = await importTestableScript();
const baseUrl = "https://gemini.google.com";

function restoration(t, kind) {
  if (kind === "header") {
    return (context) => app.syncCookieHeaderIntoContext(context, "SID=synthetic-sid; NID=synthetic-nid", baseUrl);
  }
  const profile = storageFixture(t), file = path.join(profile, "storage-state.json");
  const source = JSON.stringify({ cookies: [
    { name: "SID", value: "synthetic-sid", domain: ".google.com" },
    { name: "NID", value: "synthetic-nid", domain: ".google.com" },
  ] });
  fs.writeFileSync(file, source);
  return async (context) => {
    try {
      return await app.syncEmbeddedStorageStateIntoContext(context, profile);
    } finally {
      assert.equal(fs.readFileSync(file, "utf8"), source);
    }
  };
}

for (const kind of ["header", "embedded"]) {
  test(`${kind} restoration retains a usable cookie when bulk import and another cookie fail`, async (t) => {
    const restore = restoration(t, kind), accepted = [], logs = [];
    t.mock.method(console, "log", (...parts) => logs.push(parts.join(" ")));
    const context = { async addCookies(cookies) {
      if (cookies.length > 1) throw new Error("synthetic bulk rejection");
      const [cookie] = cookies;
      if (cookie.name !== "SID" || cookie.domain !== "google.com") throw new Error("synthetic candidate rejection");
      accepted.push(cookie);
    } };
    assert.equal(await restore(context), 1);
    assert.equal(accepted.length, 1);
    assert.equal(accepted[0].value, "synthetic-sid");
    assert.equal(accepted[0].path, "/");
    assert.equal(logs.length, 1);
    assert.match(logs[0], /NID/);
    assert.doesNotMatch(logs[0], /synthetic-(sid|nid)/);
  });

  test(`${kind} restoration propagates the original bulk error after all candidates fail`, async (t) => {
    const restore = restoration(t, kind), original = new Error("synthetic original bulk error");
    let calls = 0;
    t.mock.method(console, "log", () => {});
    const context = { async addCookies() {
      calls += 1;
      throw calls === 1 ? original : new Error("synthetic later candidate error");
    } };
    await assert.rejects(restore(context), (error) => error === original);
    assert.ok(calls > 1, "The bulk error must survive recovery attempts");
  });
}

test("cookie auth detection treats a failed cookie read as unauthenticated", async () => {
  assert.equal(await app.contextHasGeminiAuthCookies({ async cookies() { throw new Error("closed context"); } }, baseUrl), false);
});

test("empty and unrelated cookie headers do not call the browser", async () => {
  const context = { async addCookies() { assert.fail("No eligible cookies should be imported"); } };
  for (const value of [null, " ", "unknown=synthetic; SID=; malformed; =invalid; __Host-SID=synthetic"]) {
    assert.equal(await app.syncCookieHeaderIntoContext(context, value, baseUrl), 0);
  }
});

test("embedded restoration ignores missing, non-file and non-cookie storage state", async (t) => {
  const profile = storageFixture(t), file = path.join(profile, "storage-state.json");
  const context = { async addCookies() { assert.fail("No valid cookies should be imported"); } };
  assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, null), 0);
  assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, profile), 0);
  fs.mkdirSync(file);
  assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, profile), 0);
  fs.rmdirSync(file);
  for (const state of [{}, { cookies: {} }, { cookies: [] }, { cookies: [null, {}, { name: "SID", value: 1, domain: ".google.com" }, { name: "SID", value: "synthetic" }] }]) {
    const source = JSON.stringify(state);
    fs.writeFileSync(file, source);
    assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, profile), 0);
    assert.equal(fs.readFileSync(file, "utf8"), source);
  }
});

test("embedded restoration preserves malformed source and propagates its parse error", async (t) => {
  const profile = storageFixture(t), file = path.join(profile, "storage-state.json"), source = "{ invalid fixture JSON";
  fs.writeFileSync(file, source);
  await assert.rejects(app.syncEmbeddedStorageStateIntoContext({ async addCookies() { assert.fail("Invalid state must not reach the browser"); } }, profile), SyntaxError);
  assert.equal(fs.readFileSync(file, "utf8"), source);
});
