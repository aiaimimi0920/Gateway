import vm from "node:vm";

const names = [
  "isTransientAssistantStatusLine", "normalizeAssistantText", "promptLooksLikeGreeting",
  "textLooksLikeGenericWelcome", "bodyContainsSubmittedPrompt", "bodyIndicatesGenerationInProgress",
  "extractExactAnswerDirective", "augmentToolHistoryPrompt", "collectTextSnapshot", "runTextOperation",
];

export function textSnapshot(text, extra = {}) {
  return { url: "https://fixture.invalid/result", bodyText: "fixture prompt", primaryTexts: [text], fallbackTexts: [], ...extra };
}

export function textPage(nextSnapshot) {
  return {
    url: () => "https://fixture.invalid/page",
    async evaluate(callback) {
      const snapshot = nextSnapshot();
      const controls = snapshot.buttons ?? [{ text: "Send", disabled: snapshot.sendDisabled }];
      const document = {
        body: snapshot.bodyText === null ? null : { innerText: snapshot.bodyText },
        querySelectorAll(selector) {
          if (selector === "button") return controls.map(({ text, aria, disabled }) => ({ innerText: text, disabled, getAttribute: () => aria }));
          if (selector === "message-content") return snapshot.primaryTexts.map((innerText) => ({ innerText }));
          if (selector === ".markdown.markdown-main-panel, model-response") return snapshot.fallbackTexts.map((innerText) => ({ innerText }));
          throw new Error("Unexpected fixture selector: " + selector);
        },
      };
      // Execute the serialized browser callback without host lexical bindings.
      return structuredClone(vm.runInNewContext(`(${callback.toString()})()`, {
        document, location: { href: snapshot.url },
      }, { timeout: 1000 }));
    },
  };
}

export function textHarness(app, {
  snapshots = [textSnapshot(""), textSnapshot("A complete fixture answer.")],
  failureAt = null, failureCall = 1, failure = new Error("fixture dependency failed"),
} = {}) {
  const calls = [], stageCounts = new Map();
  let now = 0, snapshotIndex = 0;
  const state = { events: [{ kind: "fixture-network" }], rpcCaptures: [{ kind: "fixture-rpc" }] };
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    stageCounts.set(stage, (stageCounts.get(stage) ?? 0) + 1);
    if (stage === failureAt && stageCounts.get(stage) === failureCall) throw failure;
  };
  const page = textPage(() => {
    record("snapshot");
    return snapshots[Math.min(snapshotIndex++, snapshots.length - 1)];
  });
  page.waitForTimeout = async (ms) => {
    record("wait", ms);
    now += ms;
    if (now > 50000) throw new Error("Fixture virtual-clock budget exceeded");
  };
  const context = {
    Error, Date: { now: () => now }, normalizeString: app.normalizeString,
    DEFAULT_TIMEOUT_MS: 5000, operationConfig: () => ({ resultTimeoutMs: 3000 }),
    resolveProgramPageUrl(...args) { record("resolve", ...args); return "https://fixture.invalid/preferred"; },
    startNetworkCapture(...args) { record("capture", ...args); return { state, stop() { record("stop"); } }; },
    async resetConversation(...args) { record("reset", ...args); },
    async submitPrompt(...args) { record("submit", ...args); },
    buildProgramHandleState(...args) { record("handle", ...args); return { appPath: "/app/fixture", conversationId: "fixture-conversation" }; },
    log(...args) { record("log", ...args); },
  };
  // Execute the original exported bodies from either root or owner with controlled
  // dependencies and time. Source projection separately verifies factory wiring.
  const source = names.map((name) => app[name].toString()).join("\n\n");
  const api = vm.runInNewContext(`${source}\n({ runTextOperation })`, context, { timeout: 1000 });
  const entry = { page };
  return {
    calls, state, page, get now() { return now; },
    async run(args = {}) { return structuredClone(await api.runTextOperation(entry, { prompt: "  fixture prompt  ", timeoutMs: 100, ...args })); },
  };
}
