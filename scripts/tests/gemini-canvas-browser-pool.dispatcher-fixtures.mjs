import vm from "node:vm";

export const dispatchBaseUrl = "https://fixture.invalid";
export const dispatchProgramUrl = `${dispatchBaseUrl}/app/0123456789abcdef`;
export const callsAt = (h, name) => h.calls.filter(([stage]) => stage === name);

export function dispatcherHarness(app, options = {}) {
  const calls = [], counts = new Map();
  const failure = options.failure ?? new Error("fixture dispatcher dependency failure");
  let now = 1000, scopedArgs;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    counts.set(stage, (counts.get(stage) ?? 0) + 1);
    if (stage === options.failureAt && counts.get(stage) === (options.failureCall ?? 1)) throw failure;
  };
  const page = { isClosed: () => options.closedPage === true };
  const replacement = { async bringToFront() { record("front"); } };
  const context = { async newPage() { record("new-page"); return replacement; } };
  const entry = {
    context, page: options.missingPage ? null : page, busy: false, lastUsedAt: 17,
    runtimeStateMode: "profile_dir", runtimeStatePath: "fixture-profile", ...options.entry,
  };
  const operationResult = { marker: "fixture result" };
  const dependencies = {
    ...app, Error, Date: { now: () => now }, DEFAULT_TIMEOUT_MS: 5000,
    // Isolate environment overrides from the host's live browser configuration.
    process: { env: { ...options.env } },
    applyGeminiAccountScope(args) { record("scope", args); scopedArgs = app.applyGeminiAccountScope(args); return scopedArgs; },
    async ensureContext(...args) { record("context", ...args); return entry; },
    async closeContext(...args) { record("close", ...args); },
    async contextHasGeminiAuthCookies(...args) {
      record("auth", ...args);
      const values = options.authCookies ?? [true];
      return values[Math.min(counts.get("auth") - 1, values.length - 1)];
    },
    async syncEmbeddedStorageStateIntoContext(...args) { record("embedded", ...args); return options.embeddedCount ?? 0; },
    async syncCookieHeaderIntoContext(...args) { record("cookies", ...args); return 2; },
    resolveProgramPageUrl(...args) { record("resolve", ...args); return options.programUrl ?? null; },
    async ensureProgramPage(...args) { record("program-page", ...args); },
    async ensureAppPage(...args) { record("app-page", ...args); },
    log(...args) { calls.push(["log", ...args]); },
  };
  for (const [name, stage] of [
    ["runBootstrapProgramOperation", "bootstrap"], ["runFetchOperation", "fetch"],
    ["runTextOperation", "text"], ["runTtsOperation", "tts"],
    ["runDebugOperation", "debug"], ["runMediaOperation", "media"],
  ]) {
    dependencies[name] = async (...args) => {
      record(stage, ...args);
      if (options.onOperation) return await options.onOperation(stage, ...args);
      return operationResult;
    };
  }
  const invoke = vm.runInNewContext(`(${app.invokeGeminiCanvas.toString()})`, dependencies, { timeout: 1000 });
  return {
    calls, entry, page, replacement, failure, operationResult,
    get now() { return now; }, get scopedArgs() { return scopedArgs; },
    advance(ms) { now += ms; },
    run(args = {}) { return invoke({ runtimeStateObjectKey: "fixture-key", operation: "text", baseUrl: dispatchBaseUrl, ...args }); },
  };
}
