import assert from "node:assert/strict";
import { realpathSync } from "node:fs";
import { access } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { packageLockedWorkerDependencies } from "./locked-worker-dependencies.mjs";

const source = path.resolve(process.argv[2]), destination = path.resolve(process.argv[3]);
if (await packageLockedWorkerDependencies(source, destination)) {
  const require = createRequire(path.join(destination, "package.json"));
  const owned = realpathSync(path.join(destination, "node_modules", "playwright-core"));
  assert.ok(realpathSync(require.resolve("playwright-core")).startsWith(owned + path.sep), "browser runtime escaped the package");
  assert.equal(typeof require("playwright-core").chromium.launch, "function");
  process.stdout.write("[gateway-package] locked ChatGPT browser runtime import passed\n");
  const worker = path.join(destination, "gemini-canvas-browser-pool.mjs");
  let hasWorker = false;
  try { await access(worker); hasWorker = true; } catch (error) { if (error.code !== "ENOENT") throw error; }
  if (hasWorker) {
    // Import only. Never bind a port, open a browser, or read account material.
    process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN = "1";
    await import(pathToFileURL(worker).href);
    process.stdout.write("[gateway-package] locked Gemini worker import passed (main suppressed)\n");
  }
}
