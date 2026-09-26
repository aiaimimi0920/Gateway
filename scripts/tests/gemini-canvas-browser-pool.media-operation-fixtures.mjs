import { EventEmitter } from "node:events";
import vm from "node:vm";
import { createMediaPollingOwner } from "../gemini-canvas-browser-pool-media-polling.mjs";

export const mediaPageUrl = "https://fixture.invalid/app/0123456789abcdef";
export const imageAsset = { kind: "image", url: "https://lh3.googleusercontent.com/generated.png", mimeType: "image/png" };

export function mediaOperationHarness(t, app, options = {}) {
  const calls = [], counts = new Map(), page = new EventEmitter();
  const failure = new Error("fixture media dependency failure"), inherited = () => {};
  let now = 1000, capture = null, stops = 0, closed = false, snapshotIndex = 0;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    counts.set(stage, (counts.get(stage) ?? 0) + 1);
    if (stage === options.failureAt && counts.get(stage) === (options.failureCall ?? 1)) throw failure;
  };
  const response = (stage, fallback) => {
    const values = options.responses?.[stage];
    return values ? values[Math.min((counts.get(stage) ?? 1) - 1, values.length - 1)] : fallback;
  };
  for (const event of ["request", "response", "websocket"]) page.on(event, inherited);
  page.url = () => mediaPageUrl;
  page.waitForTimeout = async (ms) => { record("wait", ms); now += ms; };
  page.getByText = (pattern) => ({ first: () => ({
    async isVisible() { return Boolean(options.quotaText && pattern.test(options.quotaText)); },
    async textContent() { return options.quotaText; },
  }) });
  const snapshots = options.snapshots ?? [{ pageState: { url: mediaPageUrl, bodyText: "fixture result" }, assets: [imageAsset] }];
  const context = {
    ...app, Error, Date: { now: () => now }, DEFAULT_TIMEOUT_MS: 5000,
    operationConfig: () => ({ resultTimeoutMs: options.resultTimeoutMs ?? 5000 }),
    isFixtureCanvasBaseUrl: () => false,
    async acquireMediaOperationPage(...args) { record("lease", ...args); return { page, closeWhenDone: options.closeWhenDone ?? true }; },
    resolveProgramPageUrl(...args) { record("resolve", ...args); return mediaPageUrl; },
    async resetConversation(...args) { record("reset", ...args); },
    startNetworkCapture(...args) {
      record("capture", ...args);
      capture = app.startNetworkCapture(...args);
      return { state: capture.state, stop() { stops += 1; record("stop"); capture.stop(); } };
    },
    async closePageSafely(...args) { record("close", ...args); closed = true; },
    async clickOperationMode(...args) { record("mode", ...args); return response("mode", true); },
    async submitPrompt(...args) { record("submit", ...args); },
    async collectPageSnapshot(...args) {
      record("snapshot", ...args);
      if (snapshotIndex >= 200) throw new Error("Media fixture snapshot budget exceeded");
      return snapshots[Math.min(snapshotIndex++, snapshots.length - 1)];
    },
    mergeActionContract(...args) { record("action", ...args); return app.mergeActionContract(...args); },
    mergeInvokeContract(...args) { record("invoke", ...args); return app.mergeInvokeContract(...args); },
    buildCanvasProgramInvokeContract(...args) { record("build", ...args); return response("build", options.invokeContract ?? null); },
    selectMediaAssetsForOperation(...args) { record("select", ...args); return response("select", args[1].assets ?? []); },
    shouldRetryMediaPromptSubmission(...args) { record("retry-check", ...args); return response("retry-check", app.shouldRetryMediaPromptSubmission(...args)); },
    async retryMediaPromptSubmission(...args) { record("retry", ...args); },
    async trySelectVideoTemplateCard(...args) { record("template", ...args); return response("template", { clicked: false }); },
    async tryClickVideoCreateAction(...args) { record("create", ...args); return response("create", false); },
    detectMediaProviderGate(operation, text) { return options.gates ? options.gates[text] ?? null : app.detectMediaProviderGate(operation, text); },
    recentMediaProviderGateText: () => options.recentGateText ?? "",
    async clickMediaActionButton(_page, operation, action, timeout) { record(action, _page, operation, timeout); return response(action, false); },
    bodyIndicatesMusicPendingOrBusy(...args) { record("music-pending", ...args); return response("music-pending", false); },
    async extractImageBytes(...args) {
      record("extract", ...args);
      if (options.extractionError) throw options.extractionError;
      return { mimeType: "image/png", bodyBase64: "Zml4dHVyZQ==" };
    },
    async extractAudioBytes(...args) {
      record("audio", ...args);
      if (options.extractionError) throw options.extractionError;
      return { mimeType: "audio/wav", bodyBase64: "Zml4dHVyZQ==" };
    },
    buildProgramHandleState(...args) { record("handle", ...args); return { appPath: "/app/0123456789abcdef" }; },
    log(...args) { record("log", ...args); },
  };
  const mediaPollingOwner = createMediaPollingOwner(context);
  context.finalizeMediaOperation = mediaPollingOwner.finalizeMediaOperation;
  context.pollMediaOperation = vm.runInNewContext(`(${mediaPollingOwner.pollMediaOperation.toString()})`, context, { timeout: 1000 });
  const run = vm.runInNewContext(`(${app.runMediaOperation.toString()})`, context, { timeout: 1000 });
  // Keep deliberately failing pre-fix assertions from leaking fixture listeners.
  t.after(() => { capture?.stop(); page.removeAllListeners(); });
  return {
    calls, page, failure, inherited, snapshots,
    get stops() { return stops; }, get state() { return capture?.state; },
    get closed() { return closed; }, get now() { return now; },
    async run(args = {}) { return structuredClone(await run({ page }, { operation: "image", prompt: "fixture prompt", baseUrl: "https://fixture.invalid", ...args })); },
  };
}
