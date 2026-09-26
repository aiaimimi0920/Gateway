import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { chromium } from "playwright-core";
import { resolveExecutablePath } from "../gemini-canvas-browser-pool-executable.mjs";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

const { contexts, ensureContext, initializingContexts } = await importTestableScript();
const executablePath = resolveExecutablePath(process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH);

test("invalid storageState releases a real headless browser before creation rejects", {
  skip: executablePath ? false : "No local Chromium-compatible browser installed",
  timeout: 60_000,
}, async (t) => {
  const root = storageFixture(t), runtimeStateObjectKey = path.join(root, "invalid-state.json");
  fs.writeFileSync(runtimeStateObjectKey, "invalid storageState fixture");
  const launch = chromium.launch.bind(chromium);
  let browser;
  t.mock.method(chromium, "launch", async (options) => {
    browser = await launch({ ...options, headless: true });
    return browser;
  });
  try {
    await assert.rejects(ensureContext({ runtimeStateObjectKey, browserExecutablePath: executablePath }), /storageState|JSON/);
    assert.ok(browser, "The real browser must have launched before the failure");
    assert.equal(initializingContexts.has(runtimeStateObjectKey), false);
    assert.equal(contexts.has(runtimeStateObjectKey), false);
    assert.equal(browser.isConnected(), false, "Creation rejection must not retain the launched browser");
  } finally {
    // Clean up the owned browser even when the regression assertion fails.
    await browser?.close();
  }
});
