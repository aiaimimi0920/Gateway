import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { createContextOwner } from "../gemini-canvas-browser-pool-context.mjs";
import { resolveExecutablePath } from "../gemini-canvas-browser-pool-executable.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";
import { state } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";

const executable = resolveExecutablePath(process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH);
const enabled = process.env.GATEWAY_TEST_OFFLINE_BROWSER === "1" && executable;

test("real isolated browser preserves rotated cookies until a new JSON source revision replaces the context", {
  skip: enabled ? false : "Opt-in offline browser check", timeout: 60000,
}, async t => {
  const root = storageFixture(t), file = path.join(root, "state.json");
  const owner = createContextOwner({ log: () => {}, isFixtureCanvasBaseUrl: () => false,
    DEFAULT_TIMEOUT_MS: 10000, DEFAULT_LOCALE: "en-US", DEFAULT_IDLE_TIMEOUT_MS: 60000,
    cloneProfileDirectory: () => { throw new Error("No cloning is allowed"); },
    removeManagedLaunchProfileClone: async () => {}, cleanupClonedLaunchProfile: async () => {} });
  const oldHeadless = process.env.GEMINI_CANVAS_BROWSER_HEADLESS;
  process.env.GEMINI_CANVAS_BROWSER_HEADLESS = "true";
  t.after(async () => {
    try { for (const key of owner.contexts.keys()) await owner.closeContext(key); }
    finally {
      if (oldHeadless === undefined) delete process.env.GEMINI_CANVAS_BROWSER_HEADLESS;
      else process.env.GEMINI_CANVAS_BROWSER_HEADLESS = oldHeadless;
    }
  });
  fs.writeFileSync(file, JSON.stringify(state("seed")), "utf8");
  const args = { runtimeStateObjectKey: "state.json", browserExecutablePath: executable };
  const first = await owner.ensureContext(args);
  await first.context.setOffline(true);
  assert.equal(first.page.url(), "about:blank");
  await first.context.addCookies(state("rotated").cookies);
  assert.equal(await owner.ensureContext(args), first);
  assert.equal((await first.context.cookies())[0].value, "rotated");
  assert.equal(JSON.parse(fs.readFileSync(file, "utf8")).cookies[0].value, "seed");
  fs.writeFileSync(file, JSON.stringify(state("reimported")), "utf8");
  const second = await owner.ensureContext(args);
  await second.context.setOffline(true);
  assert.notEqual(second, first);
  assert.equal(first.contextClosed, true);
  assert.equal(first.browser.isConnected(), false);
  assert.equal(second.page.url(), "about:blank");
  assert.equal((await second.context.cookies())[0].value, "reimported");
  await owner.closeContext("state.json");
  assert.equal(second.browser.isConnected(), false);
  assert.equal(owner.contexts.size, 0);
  assert.equal(owner.initializingContexts.size, 0);
});
