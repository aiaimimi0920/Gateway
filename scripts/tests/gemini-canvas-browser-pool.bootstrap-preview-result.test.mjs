import assert from "node:assert/strict";
import test from "node:test";
import { createBootstrapPreviewResultOwner } from "../gemini-canvas-browser-pool-bootstrap-preview-result.mjs";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const appPath = "/app/0123456789abcdef", programUrl = "https://fixture.invalid" + appPath;

function resultHarness({ handle = {}, failureAt = null } = {}) {
  const calls = [], failure = new Error("fixture result dependency failure");
  const captureState = {
    actionContract: { canvasProgramAction: null, canvasProgramActionInput: null },
    transportHints: { invokeBaseUrls: [], musicWsUrls: [], videoInvokePaths: [] },
    invokeContract: null, events: [{ type: "fixture-event" }], rpcCaptures: [{ type: "fixture-rpc" }],
  };
  const input = {
    baseUrl: "https://fixture.invalid///", args: { runtimeStateObjectKey: "  fixture-key  " },
    previewSnapshot: { url: programUrl, bodyText: '"action": "preview-action"\n"action_input": "preview-input"' },
    afterPreview: { url: "https://fixture.invalid/fallback", bodyText: '"action": "fallback-action"' },
    captureState,
    aggregateHints: { appPaths: [appPath], conversationIds: ["c_0123456789abcdef"], responseIds: ["old", "latest"], sharePaths: [] },
    bootstrapOperation: "text", bootstrapPrompt: "fixture prompt", shareUrl: "https://fixture.invalid/share/fixture",
    shareId: "fixture", shareFollow: { kind: "popup" }, discoveryOnly: true,
    canvasProxyPreview: { directLaunch: { launched: true } }, shareMaterializationRetry: { attempted: true },
    before: { url: "https://fixture.invalid/before" },
  };
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    if (stage === failureAt) throw failure;
  };
  const { buildBootstrapPreviewResult } = createBootstrapPreviewResultOwner({
    buildProgramHandleState(...args) { record("handle", ...args); return handle; },
    buildCanvasProgramInvokeContract(...args) {
      record("build", ...args);
      return { operation: args[0], actionName: args[1].canvasProgramAction, uiState: "fixture-preview" };
    },
    mergeInvokeContract(...args) { record("merge", ...args); return app.mergeInvokeContract(...args); },
  });
  return { input, calls, failure, run: () => buildBootstrapPreviewResult(input) };
}

test("Bootstrap preview result preserves exact schema and snapshot/state identity at dependency boundaries", () => {
  const h = resultHarness(), result = h.run(), input = h.input;
  assert.deepEqual(Object.keys(result).sort(), [
    "operation", "runtimeStateObjectKey", "shareUrl", "shareId", "shareFollowKind", "bootstrapOperation", "bootstrapPrompt", "discoveryOnly",
    "beforeUrl", "finalUrl", "pageUrl", "bodyText", "canvasProxyPreview", "shareMaterializationRetry", "newChatClicked", "modeSelected",
    "canvasProgramUrl", "appPath", "conversationId", "responseId", "lastSeenConversationId", "lastSeenResponseId", "candidatePairs",
    "stableProgramPair", "latestResponsePair", "aggregateHints", "transportHints", "invokeBaseUrl", "musicWsUrl", "videoInvokePath",
    "canvasProgramAction", "canvasProgramActionInput", "canvasProgramInvokeContract", "networkEvents",
  ].sort());
  assert.deepEqual(h.calls.map(([stage]) => stage), ["handle", "build", "merge"]);
  assert.equal(h.calls[0][2], input.args);
  assert.equal(h.calls[0][3], input.previewSnapshot.url);
  assert.equal(h.calls[0][4], input.captureState);
  assert.equal(h.calls[1][3], input.captureState.transportHints);
  assert.equal(h.calls[1][4], input.previewSnapshot);
  assert.equal(h.calls[1][5], input.bootstrapPrompt);
  assert.equal(h.calls[1][6], input.captureState);
  assert.equal(result.aggregateHints, input.aggregateHints);
  assert.equal(result.networkEvents, input.captureState.events);
  assert.equal(result.transportHints, input.captureState.transportHints);
  assert.equal(result.canvasProxyPreview, input.canvasProxyPreview);
  assert.equal(result.shareMaterializationRetry, input.shareMaterializationRetry);
  assert.equal(result.runtimeStateObjectKey, "fixture-key");
  assert.equal(result.newChatClicked, false);
  assert.equal(result.modeSelected, false);
  assert.equal(result.shareFollowKind, "popup");
});

test("Bootstrap preview result falls back to the latest concrete hint without reversing shared arrays", () => {
  const h = resultHarness();
  h.input.aggregateHints.appPaths.push("/app/not-concrete", "/app/fedcba9876543210", "/app/latest-invalid");
  h.input.aggregateHints.conversationIds.push("c_fedcba9876543210");
  const before = structuredClone(h.input), result = h.run();
  assert.equal(result.appPath, "/app/fedcba9876543210");
  assert.equal(result.canvasProgramUrl, "https://fixture.invalid/app/fedcba9876543210");
  assert.equal(result.conversationId, "c_fedcba9876543210");
  assert.equal(result.responseId, "latest");
  assert.equal(result.lastSeenConversationId, "c_fedcba9876543210");
  assert.equal(result.lastSeenResponseId, "latest");
  assert.deepEqual(h.input, before);
});

test("Bootstrap preview result gives canonical handle and last-seen state precedence over hints", () => {
  const handle = {
    canvasProgramUrl: "https://fixture.invalid/canonical", appPath: "/app/canonical", conversationId: "canonical-conversation",
    responseId: "canonical-response", lastSeenConversationId: "seen-conversation", lastSeenResponseId: "seen-response",
    candidatePairs: [{ appPath }], stableProgramPair: { appPath }, latestResponsePair: { responseId: "latest" },
    invokeBaseUrl: "https://fixture.invalid/invoke", musicWsUrl: "wss://fixture.invalid/music", videoInvokePath: "/video",
  };
  const result = resultHarness({ handle }).run();
  for (const [key, value] of Object.entries(handle)) assert.equal(result[key], value);
});

test("Bootstrap preview result fills action fields independently while retaining captured precedence", () => {
  const h = resultHarness();
  h.input.captureState.actionContract.canvasProgramAction = "captured-action";
  const result = h.run();
  assert.equal(result.canvasProgramAction, "captured-action");
  assert.equal(result.canvasProgramActionInput, "preview-input");
  assert.deepEqual(h.calls[1][2], { canvasProgramAction: "captured-action", canvasProgramActionInput: "preview-input" });
  assert.equal(h.input.captureState.actionContract.canvasProgramActionInput, null);
  assert.equal(h.input.captureState.invokeContract, null);
});

test("Bootstrap preview result preserves explicit empty action values instead of replacing them", () => {
  const h = resultHarness();
  h.input.captureState.actionContract = { canvasProgramAction: "", canvasProgramActionInput: "" };
  const result = h.run();
  assert.equal(result.canvasProgramAction, "");
  assert.equal(result.canvasProgramActionInput, "");
});

test("Bootstrap absent preview uses afterPreview for URL body and invoke snapshot but not action extraction", () => {
  const h = resultHarness();
  h.input.previewSnapshot = null;
  const result = h.run();
  assert.equal(result.pageUrl, h.input.afterPreview.url);
  assert.equal(result.finalUrl, h.input.afterPreview.url);
  assert.equal(result.bodyText, h.input.afterPreview.bodyText);
  assert.equal(h.calls[0][3], h.input.afterPreview.url);
  assert.equal(h.calls[1][4], h.input.afterPreview);
  assert.equal(result.canvasProgramAction, null);
  assert.equal(result.canvasProgramActionInput, null);
});

test("Bootstrap preview result retains empty snapshot fields through nullish fallbacks", () => {
  const h = resultHarness();
  h.input.previewSnapshot = { url: "", bodyText: "" };
  const result = h.run();
  assert.equal(result.pageUrl, "");
  assert.equal(result.bodyText, "");
  assert.equal(h.calls[0][3], "");
  assert.equal(h.calls[1][4], h.input.previewSnapshot);
});

for (const absent of ["appPath", "canvasProgramUrl", "conversationId"]) {
  test(`Bootstrap preview missing ${absent} retains its error and computes invoke before rejection`, () => {
    const handle = { appPath, canvasProgramUrl: programUrl, conversationId: "c_fixture" };
    handle[absent] = null;
    if (absent === "canvasProgramUrl") handle.appPath = null;
    const h = resultHarness({ handle });
    h.input.aggregateHints = { appPaths: [], conversationIds: [], responseIds: [], sharePaths: [] };
    assert.throws(h.run, { status: 504, code: "gemini_canvas_program_bootstrap_handle_missing", bodyText: h.input.previewSnapshot.bodyText });
    assert.deepEqual(h.calls.map(([stage]) => stage), ["handle", "build", "merge"]);
  });
}

for (const failureAt of ["handle", "build", "merge"]) {
  test(`Bootstrap preview result propagates ${failureAt} failure without later dependency calls`, () => {
    const h = resultHarness({ failureAt });
    assert.throws(h.run, (error) => error === h.failure);
    assert.equal(h.calls.at(-1)[0], failureAt);
  });
}

test("Bootstrap preview result composes with the actual root handle and invoke owners", () => {
  const { buildBootstrapPreviewResult } = createBootstrapPreviewResultOwner(app);
  const h = resultHarness();
  h.input.args.canvasProgramUrl = programUrl;
  h.input.args.appPath = appPath;
  h.input.args.conversationId = "c_0123456789abcdef";
  const result = buildBootstrapPreviewResult(h.input);
  assert.equal(result.operation, "bootstrap_program");
  assert.equal(result.appPath, appPath);
  assert.equal(result.canvasProgramUrl, programUrl);
  assert.equal(result.conversationId, "c_0123456789abcdef");
  assert.equal(result.canvasProgramAction, "preview-action");
});
