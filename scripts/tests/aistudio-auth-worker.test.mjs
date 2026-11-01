import assert from "node:assert/strict";
import { test } from "node:test";
import { managedImport, readImportModels } from "../aistudio-auth-worker.mjs";

const cookies = ["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"].map(name => ({
  name, value: "private-fixture", domain: ".google.com", path: "/", expires: -1, secure: true, httpOnly: true,
}));
const bundle = { schemaVersion: 1, provider: "aistudio-web", authUser: "0", cookies };

test("managed sign-in returns only scoped material to the pipe and reads a non-generative catalog", async () => {
  const calls = [];
  const result = await managedImport({ tempRoot: "C:/private/owned-test" }, {
    capture: async options => { assert.equal(options.tempRoot, "C:/private/owned-test"); return bundle; },
    send: async (stage, url, options) => {
      calls.push({ stage, url, options });
      return { text: stage === "bootstrap" ? '"WIu0Nc":"public-page-key"' : '[[["models/gemini-3.5-flash-lite"]]]' };
    },
  });
  assert.equal(result.ok, true);
  assert.deepEqual(result.models, ["gemini-3.5-flash-lite"]);
  assert.equal(result.bundle, bundle);
  assert.deepEqual(calls.map(c => c.stage), ["bootstrap", "models"]);
  assert.equal(calls[1].options.body, "[]");
  assert.ok(!calls.some(c => /GenerateContent|GenerateAccessToken|Upload|Waa/.test(c.url)));
  assert.equal(result.bundle.origins, undefined);
});

test("managed import rejects arbitrary relative paths before opening a browser", async () => {
  let captured = false;
  await assert.rejects(managedImport({ tempRoot: "relative" }, { capture: async () => { captured = true; } }), /temp_root_required/);
  assert.equal(captured, false);
});

test("catalog errors do not fall back to generated defaults or send a prompt", async () => {
  await assert.rejects(readImportModels(bundle, async () => { throw new Error("aistudio_http_login_required"); }), /login_required/);
  for (const text of ['[]', '[[["models/invalid*model"]]]', '[[["models/valid"],[null]]]']) {
    await assert.rejects(readImportModels(bundle, async stage => ({ text: stage === "bootstrap" ? '"WIu0Nc":"public"' : text })), /catalog_invalid/);
  }
});
