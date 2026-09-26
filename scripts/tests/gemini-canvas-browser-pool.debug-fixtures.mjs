import { EventEmitter } from "node:events";
import vm from "node:vm";

export function debugHarness(t, app, { failureAt = null, failureCall = 1 } = {}) {
  const calls = [], counts = new Map(), page = new EventEmitter();
  const failure = new Error("fixture debug dependency failure");
  let capture = null, stops = 0;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    counts.set(stage, (counts.get(stage) ?? 0) + 1);
    if (stage === failureAt && counts.get(stage) === failureCall) throw failure;
  };
  const inherited = () => {};
  for (const name of ["request", "response", "websocket"]) page.on(name, inherited);
  page.waitForTimeout = async (ms) => { record("wait", ms); };
  page.content = async () => { record("content"); return "fixture page content"; };
  page.evaluate = async (callback, argument) => {
    record("evaluate");
    const document = {
      querySelectorAll: () => [], scripts: [], title: "Fixture debug page",
      documentElement: { outerHTML: "<html>fixture</html>" }, body: { innerText: "fixture body" },
    };
    return structuredClone(vm.runInNewContext(`(${callback.toString()})(__argument)`, {
      __argument: argument, document, window: {}, location: { href: "https://fixture.invalid/debug" },
    }, { timeout: 1000 }));
  };
  const context = {
    normalizeString: app.normalizeString, parseBoolean: app.parseBoolean, DEFAULT_TIMEOUT_MS: 5000,
    resolveProgramPageUrl(...args) { record("resolve", ...args); return "https://fixture.invalid/preferred"; },
    async resetConversation(...args) { record("reset", ...args); },
    startNetworkCapture(...args) {
      record("capture", ...args);
      capture = app.startNetworkCapture(...args);
      // The real owner installs/removes listeners; only count its stop calls here.
      return { state: capture.state, stop() { stops += 1; record("stop"); capture.stop(); } };
    },
    async clickOperationMode(...args) { record("click", ...args); return true; },
    log(...args) { record("log", ...args); },
    async collectPageSnapshot() { record("snapshot"); return { mediaNodes: [{ kind: "image", src: "fixture-image" }] }; },
    async collectButtonSnapshot() { record("buttons"); return []; },
    extractProgramHandleHintsFromText(...args) { record("extract", ...args); return app.extractProgramHandleHintsFromText(...args); },
    mergeProgramHandleHints(...args) { record("merge", ...args); return app.mergeProgramHandleHints(...args); },
    buildProgramHandleState(...args) { record("handle", ...args); return { appPath: "/app/fixture", conversationId: "fixture-conversation" }; },
  };
  const run = vm.runInNewContext(`(${app.runDebugOperation.toString()})`, context, { timeout: 1000 });
  // Failed pre-fix regression assertions must not leak even isolated fixture listeners.
  t.after(() => { capture?.stop(); page.removeAllListeners(); });
  return {
    page, calls, failure, inherited,
    get stops() { return stops; }, get state() { return capture?.state; },
    async run(args = {}) { return structuredClone(await run({ page }, { captureNetwork: "true", clickOperation: "image", ...args })); },
  };
}
