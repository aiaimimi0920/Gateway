import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { app, appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

for (const legacyFields of [false, true]) {
  test(`embedded URL cookie hydrates a real browser${legacyFields ? " while taking precedence over legacy domain/path" : " without a domain/path pair"}`, browserTest, async (t) => {
    const profile = storageFixture(t), { context } = await appFixture(t);
    const cookie = { name: "SID", value: "fixture-url-cookie", url: "https://gemini.google.com/app/fixture", httpOnly: true };
    if (legacyFields) Object.assign(cookie, { domain: ".example.invalid", path: "/legacy" });
    const statePath = path.join(profile, "storage-state.json"), source = JSON.stringify({ cookies: [cookie] });
    fs.writeFileSync(statePath, source);
    assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, profile), 1);
    const [stored] = await context.cookies();
    assert.equal(stored.name, "SID");
    assert.equal(stored.value, "fixture-url-cookie");
    assert.equal(stored.domain, "gemini.google.com");
    assert.equal(stored.path, "/app/");
    assert.equal(stored.secure, true);
    assert.equal(stored.httpOnly, true);
    assert.equal(fs.readFileSync(statePath, "utf8"), source);
  });
}

test("embedded domain cookie retains its explicit path and attributes in a real browser", browserTest, async (t) => {
  const profile = storageFixture(t), { context } = await appFixture(t);
  const cookie = { name: "SID", value: "fixture-domain-cookie", domain: ".google.com", path: "/saved", secure: true, httpOnly: true, sameSite: "lax", expires: -1 };
  fs.writeFileSync(path.join(profile, "storage-state.json"), JSON.stringify({ cookies: [cookie] }));
  assert.equal(await app.syncEmbeddedStorageStateIntoContext(context, profile), 1);
  const [stored] = await context.cookies();
  assert.equal(stored.domain, ".google.com");
  assert.equal(stored.path, "/saved");
  assert.equal(stored.secure, true);
  assert.equal(stored.httpOnly, true);
  assert.equal(stored.sameSite, "Lax");
  assert.equal(stored.expires, -1);
});
