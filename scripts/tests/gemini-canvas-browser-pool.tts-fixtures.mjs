import vm from "node:vm";
import { domPage } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";

const names = ["buildTtsDiagnostics", "listenControlCandidates", "clickListenControlWithFallback", "runTtsOperation"];
const defaultAsset = { url: "https://fixture.invalid/audio.wav", mimeType: "audio/wav", durationSeconds: 5 };
const defaultAudio = { mimeType: "audio/ogg", bodyBase64: "fixture-audio" };

export function ttsHarness(app, {
  visibleIndex = 0, clickFailure = false, scrollFailure = false, handleNode = null,
  domControls = [], assets = [defaultAsset], extractions = [defaultAudio], failureAt = null,
  failure = new Error("fixture dependency failure"), buttonFailure = false,
  snapshot = { pageState: { url: "https://fixture.invalid/current", bodyText: "response", title: "Fixture" }, mediaNodes: [], anchorNodes: [] },
} = {}) {
  const calls = [];
  let now = 0, assetIndex = 0, extractionIndex = 0;
  const state = { audioUrls: [], events: [], streamGenerateRequestAt: 1, streamGenerateResponseAt: 2 };
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    if (stage === failureAt) throw failure;
  };
  const candidates = ["role", "named", "text"].map((name, index) => ({
    name, last() { return this; },
    async waitFor(options) {
      record("listen-wait", name, options);
      if (index !== visibleIndex) throw new Error("fixture Listen control absent");
    },
    async scrollIntoViewIfNeeded() { if (scrollFailure) throw new Error("fixture scroll failed"); },
    async click(options) {
      record("listen-click", name, options);
      if (clickFailure) throw new Error("fixture click intercepted");
    },
    async elementHandle() {
      if (!handleNode) return null;
      return { async evaluate(callback) {
        // Keep the element callback isolated from both the operation and fixture closures.
        return vm.runInNewContext(`(${callback.toString()})(__node)`, {
          __node: handleNode, MouseEvent: class { constructor(type) { this.type = type; } }, window: {},
        }, { timeout: 1000 });
      } };
    },
  }));
  const { page } = domPage({ controls: domControls });
  page.url = () => "https://fixture.invalid/page-fallback";
  page.getByRole = () => candidates[0];
  page.locator = (selector) => selector.startsWith("button[aria-label") ? candidates[1] : candidates[2];
  page.waitForTimeout = async (ms) => {
    record("wait", ms);
    now += ms;
    if (now > 100000) throw new Error("fixture virtual-clock budget exceeded");
  };
  const context = {
    Buffer, Error, Date: { now: () => now },
    normalizeString: app.normalizeString, parseBoolean: app.parseBoolean,
    DEFAULT_TIMEOUT_MS: 5000, operationConfig: () => ({ resultTimeoutMs: 3000 }),
    isFixtureCanvasBaseUrl: () => false,
    resolveProgramPageUrl(base, args) { record("resolve", base, args); return "https://fixture.invalid/preferred"; },
    async resetConversation(...args) { record("reset", ...args); },
    startNetworkCapture(...args) { record("capture", ...args); return { state, stop() { record("stop"); } }; },
    async submitPrompt(...args) { record("submit", ...args); },
    async collectPageSnapshot() { record("snapshot"); return snapshot; },
    async collectButtonSnapshot() { if (buttonFailure) throw new Error("fixture buttons detached"); return [{ text: "Listen" }]; },
    selectAudioAsset() { record("asset"); return assets[Math.min(assetIndex++, assets.length - 1)] ?? null; },
    async extractAudioBytes(...args) {
      record("extract", ...args);
      const result = extractions[Math.min(extractionIndex++, extractions.length - 1)];
      if (result instanceof Error) throw result;
      return result;
    },
    buildProgramHandleState(...args) {
      record("handle", ...args);
      return { appPath: "/app/fixture", conversationId: "fixture-conversation" };
    },
    log(...args) { record("log", ...args); },
  };
  // Run the real function bodies from either root or extracted owner with controlled
  // dependencies and virtual time; no handwritten operation implementation is copied.
  const source = names.map((name) => app[name].toString()).join("\n\n");
  const api = vm.runInNewContext(`${source}\n({ ${names.join(", ")} })`, context, { timeout: 1000 });
  const entry = { page, attachedCdp: true };
  return {
    entry, page, candidates, calls, state, api,
    get now() { return now; },
    async run(args = {}) { return structuredClone(await api.runTtsOperation(entry, { prompt: "  fixture prompt  ", timeoutMs: 100, ...args })); },
  };
}
