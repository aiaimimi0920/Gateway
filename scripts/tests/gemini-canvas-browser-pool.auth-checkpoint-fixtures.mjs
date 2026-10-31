import fs from "node:fs";
import path from "node:path";
import { refreshFixture } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";
import { state } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";
import { createInvocationDispatcher } from "../gemini-canvas-browser-pool-dispatcher.mjs";
import { settleAuthCheckpoint } from "../gemini-canvas-browser-pool-auth-checkpoint.mjs";

export { state };
export function checkpointEnv(t, root, values = {}) {
  for (const [suffix, value] of Object.entries({ DRIVER: "local", DIR: path.join(root, "checkpoints"),
    KEY: Buffer.alloc(32, 7).toString("base64"), PREFIX: undefined, ...values })) {
    const name = `GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_${suffix}`, previous = process.env[name];
    if (value === undefined) delete process.env[name]; else process.env[name] = value;
    t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  }
}

export function checkpointFixture(t) {
  const f = refreshFixture(t);
  checkpointEnv(t, f.root);
  const seed = f.write("seed-sensitive-fixture");
  const sourceBytes = fs.readFileSync(seed);
  const counters = { operations: 0, exports: 0 };
  const hooks = { auth: true };
  const log = (...args) => f.logs.push(args);
  const contextHasGeminiAuthCookies = async () => hooks.auth;
  const run = async entry => {
    counters.operations++;
    entry.context.storageState = async options => {
      counters.exports++;
      if (options.indexedDB !== true) throw new Error("IndexedDB capture required");
      await hooks.capture?.();
      return state("rotated-sensitive-fixture");
    };
    if (hooks.failure) throw hooks.failure;
    await hooks.operation?.(entry);
    return hooks.result ?? { text: "accepted once" };
  };
  const dispatcher = createInvocationDispatcher({ DEFAULT_TIMEOUT_MS: 1000, log,
    ensureContext: f.owner.ensureContext, closeContext: f.owner.closeContext, contextHasGeminiAuthCookies,
    syncCookieHeaderIntoContext: async () => 0, syncEmbeddedStorageStateIntoContext: async () => 0,
    resolveProgramPageUrl: () => null, ensureAppPage: async () => {}, ensureProgramPage: async () => {},
    runBootstrapProgramOperation: run, runTextOperation: run, runMediaOperation: run,
    runTtsOperation: run, runDebugOperation: run, runFetchOperation: run,
    settleAuthCheckpoint: (entry, outcome) => settleAuthCheckpoint(entry, outcome,
      { closeContext: f.owner.closeContext, contextHasGeminiAuthCookies, log }),
  });
  return { ...f, seed, sourceBytes, counters, checkpointHooks: hooks,
    invoke: args => dispatcher.invokeGeminiCanvas({ ...f.args(), operation: "text", ...args }) };
}
