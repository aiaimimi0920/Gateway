import { EventEmitter } from "node:events";
import vm from "node:vm";
import { createBootstrapPreviewResultOwner } from "../gemini-canvas-browser-pool-bootstrap-preview-result.mjs";
import { createBootstrapPollingOwner } from "../gemini-canvas-browser-pool-bootstrap-polling.mjs";
import { createBootstrapResultOwner } from "../gemini-canvas-browser-pool-bootstrap-result.mjs";

export const programUrl = "https://fixture.invalid/app/0123456789abcdef";

export function bootstrapSnapshot(id, extra = {}) {
  return {
    id, url: programUrl, bodyText: `"action": "${id}"\n"action_input": "${id}-input"`,
    handleHints: { appPaths: ["/app/0123456789abcdef"], conversationIds: ["c_0123456789abcdef"], responseIds: [id], sharePaths: [] },
    ...extra,
  };
}

export function bootstrapHarness(t, app, options = {}) {
  const calls = [], counts = new Map(), captures = [], pages = [];
  const snapshots = options.snapshots ?? ["before", "new-chat", "poll"].map((id) => bootstrapSnapshot(id));
  const failure = new Error("fixture bootstrap dependency failure");
  let now = 1000, snapshotIndex = 0;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    counts.set(stage, (counts.get(stage) ?? 0) + 1);
    if (stage === options.failureAt && counts.get(stage) === (options.failureCall ?? 1)) throw failure;
  };
  const makePage = (name) => {
    const page = new EventEmitter();
    page.name = name;
    page.currentUrl = programUrl;
    page.url = () => page.currentUrl;
    page.closed = false;
    page.isClosed = () => page.closed;
    page.close = async () => { record("close", page); page.closed = true; };
    page.waitForTimeout = async (ms) => {
      record("wait", page, ms);
      now += ms;
      if (now > 120000) throw new Error("Fixture virtual-clock budget exceeded");
    };
    page.inherited = () => {};
    for (const event of ["request", "response", "websocket"]) page.on(event, page.inherited);
    pages.push(page);
    return page;
  };
  const page = makePage("initial"), popup = makePage("popup"), preview = makePage("preview");
  const entry = { page, context: { fixture: true } };
  const context = {
    ...app, Error, Date: { now: () => now }, DEFAULT_TIMEOUT_MS: 5000,
    operationConfig: () => ({ resultTimeoutMs: 3000 }),
    resolveProgramPageUrl(_base, args) { return args.programPageUrl ?? null; },
    startNetworkCapture(nextPage, operation, existingState) {
      record("capture", nextPage, operation, existingState);
      const real = app.startNetworkCapture(nextPage, operation, existingState);
      const capture = { page: nextPage, state: real.state, stops: 0, real };
      captures.push(capture);
      return { state: real.state, stop() { capture.stops += 1; record("stop", nextPage); real.stop(); } };
    },
    async ensureProgramPage(...args) { record("program", ...args); },
    async ensureSharePage(...args) { record("share", ...args); entry.page.currentUrl = "https://fixture.invalid/share/fixture"; },
    async ensureAppPage(...args) { record("app", ...args); entry.page.currentUrl = programUrl; },
    async waitForShareSurface(...args) { record("surface", ...args); },
    async tryFollowShareEntryPoint(nextPage) {
      record("follow", nextPage);
      if (options.followPopup) return { kind: "popup", page: popup };
      if (!options.unmaterialized) nextPage.currentUrl = programUrl;
      return { kind: "same_page", page: null };
    },
    async hasPromptTextbox(nextPage) { record("textbox", nextPage); return options.hasTextbox ?? true; },
    async collectProgramHandleSnapshot(nextPage) {
      record("snapshot", nextPage);
      return snapshots[Math.min(snapshotIndex++, snapshots.length - 1)];
    },
    mergeProgramHandleHints(...args) { record("hints", ...args); return app.mergeProgramHandleHints(...args); },
    extractCanvasProgramActionContractFromText(...args) { record("extract", ...args); return app.extractCanvasProgramActionContractFromText(...args); },
    mergeActionContract(...args) { record("action", ...args); return app.mergeActionContract(...args); },
    buildCanvasProgramInvokeContract(operation, action, transport, snapshot, prompt, state) {
      record("build", { operation, action, actionAtBuild: structuredClone(action), transport, snapshot, prompt, state });
      return { operation, actionName: action.canvasProgramAction, uiState: options.uiState ?? (operation === "music" ? "music_player_ready" : "fixture_ready"), target: snapshot.target ?? null };
    },
    mergeInvokeContract(...args) { record("invoke", ...args); return app.mergeInvokeContract(...args); },
    buildProgramHandleState(...args) {
      record("handle", ...args);
      return options.missingHandle ? {} : {
        canvasProgramUrl: programUrl, appPath: "/app/0123456789abcdef", conversationId: "c_0123456789abcdef",
        candidatePairs: [], stableProgramPair: null, latestResponsePair: null,
        capturedAt: "fixture-captured", lastValidatedAt: "fixture-validated",
      };
    },
    shouldStayOnCanvasProxyDiscoverySurface(...args) { record("stay", ...args); return options.stay ?? false; },
    async tryOpenCanvasProxyPreview(...args) { record("preview", ...args); return { clicked: true, bridge: { eventCount: options.bridgeEvents ?? 1 } }; },
    async stampCanvasProxyPreviewFrames(...args) { record("stamp", ...args); return ["fixture-frame"]; },
    collectCanvasProxyContractTexts: () => [], extractCanvasProxyClientHtmlFromTexts: () => null,
    canvasProxyPreviewNeedsDirectLaunch: () => Boolean(options.directLaunch),
    async tryLaunchCanvasProxyClientFromCapturedHtml(...args) {
      record("launch", ...args);
      return { launched: true, page: preview, marker: "fixture-launch" };
    },
    async clickNewChat(...args) { record("new-chat", ...args); return true; },
    async clickOperationMode(...args) { record("mode", ...args); return true; },
    async trySelectMusicStyleCard(...args) { record("style", ...args); return { clicked: true }; },
    async submitPrompt(...args) { record("submit", ...args); },
    invokeContractIndicatesConcreteProgress(...args) { record("progress", ...args); return options.progressReady ?? true; },
    async clickMediaActionButton(_page, _operation, action) {
      record(action, _page);
      await options.onMediaAction?.(action, _page);
      return options[action] ?? true;
    },
    log() {},
  };
  context.buildBootstrapPreviewResult = createBootstrapPreviewResultOwner(context).buildBootstrapPreviewResult;
  const { pollBootstrapProgram } = createBootstrapPollingOwner(context);
  context.pollBootstrapProgram = vm.runInNewContext(`(${pollBootstrapProgram.toString()})`, context, { timeout: 1000 });
  const { finalizeBootstrapProgram } = createBootstrapResultOwner(context);
  context.finalizeBootstrapProgram = vm.runInNewContext(`(${finalizeBootstrapProgram.toString()})`, context, { timeout: 1000 });
  const run = vm.runInNewContext(`(${app.runBootstrapProgramOperation.toString()})`, context, { timeout: 1000 });
  t.after(() => {
    for (const capture of captures) capture.real.stop();
    for (const ownedPage of pages) ownedPage.removeAllListeners();
  });
  return {
    calls, captures, pages, page, popup, preview, entry, failure, snapshots,
    get now() { return now; },
    async run(args = {}) {
      return structuredClone(await run(entry, {
        baseUrl: "https://fixture.invalid", programPageUrl: programUrl,
        bootstrapOperation: "text", bootstrapPrompt: "  fixture prompt  ", timeoutMs: 5000,
        ...args,
      }));
    },
  };
}
