import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const pairText = '"c_deadbeef", "r_cafebabe"';
const defaultPair = {
  appPath: "/app/deadbeef", programUrl: "https://gemini.google.com/app/deadbeef",
  conversationId: "c_deadbeef", responseId: "r_cafebabe",
  sourceUrl: null, sourceRpc: null, sourceKind: null, sourceSurface: null,
  sourceWsUrl: null, sourceTargetDomain: null, ts: null,
};

test("handle strings retain first occurrence after trimming and existing coercion", () => {
  const values = Object.freeze([null, false, 0, "", "  ", "first", " first ", 3, true, "last"]);
  assert.deepEqual(app.uniqueStrings(values), ["first", "3", "true", "last"]);
  assert.deepEqual(app.uniqueStrings(null), []);
});

test("handle paths preserve order, case and existing prefix extraction", () => {
  const text = "https://gemini.google.com/app/deadbeef /app/1234567890123 /app/DEADBEEF /app/deadbeef/extra";
  assert.deepEqual(app.extractProgramAppPaths(text), ["/app/deadbeef", "/app/1234567890123", "/app/DEADBEEF"]);
  assert.equal(app.extractAppPath(text), "/app/deadbeef");
  assert.equal(app.extractAppPath(null), null);
  assert.equal(app.extractAppPath("no program"), null);
});

test("handle hints append derived paths after explicit paths without lowercasing IDs", () => {
  const text = "/app/cafebabe C_DEADBEEF R_CAFECAFE /SHARE/ABCDEF12 C_DEADBEEF /app/cafebabe";
  assert.deepEqual(app.extractProgramHandleHintsFromText(text), {
    appPaths: ["/app/cafebabe", "/app/DEADBEEF"], conversationIds: ["c_DEADBEEF"],
    responseIds: ["r_CAFECAFE"], sharePaths: ["/SHARE/ABCDEF12"],
  });
});

test("handle ID hints reject short IDs and word-embedded IDs", () => {
  assert.deepEqual(app.extractProgramHandleHintsFromText("c_abcdef0 r_abcdef0 xc_deadbeef xr_deadbeef c_deadbeefg r_deadbeefg /share/abc1234"), {
    appPaths: [], conversationIds: [], responseIds: [], sharePaths: [],
  });
});

test("handle global regex cursors reset across repeated successful and empty parses", () => {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    assert.deepEqual(app.extractProgramAppPaths("/app/deadbeef /app/cafebabe"), ["/app/deadbeef", "/app/cafebabe"]);
    assert.deepEqual(app.extractProgramHandleHintsFromText("c_deadbeef r_cafebabe /share/abcd1234"), {
      appPaths: ["/app/deadbeef"], conversationIds: ["c_deadbeef"], responseIds: ["r_cafebabe"], sharePaths: ["/share/abcd1234"],
    });
    assert.deepEqual(app.extractProgramHandlePairs(pairText), [defaultPair]);
    assert.deepEqual(app.extractProgramAppPaths(""), []);
    assert.deepEqual(app.extractProgramHandlePairs(""), []);
    assert.deepEqual(app.extractProgramHandleHintsFromText(null), { appPaths: [], conversationIds: [], responseIds: [], sharePaths: [] });
  }
});

test("handle hint merge owns fresh target arrays and leaves frozen inputs unchanged", () => {
  const originalPaths = Object.freeze(["/app/deadbeef"]);
  const target = { appPaths: originalPaths, conversationIds: Object.freeze(["c_deadbeef"]), retained: "metadata" };
  const incoming = Object.freeze({
    appPaths: Object.freeze(["/app/cafebabe", " /app/deadbeef "]),
    conversationIds: Object.freeze(["c_cafebabe"]), responseIds: Object.freeze(["r_deadbeef"]),
    sharePaths: Object.freeze(["/share/abcd1234"]),
  });
  assert.equal(app.mergeProgramHandleHints(target, incoming), undefined);
  assert.deepEqual(target, {
    appPaths: ["/app/deadbeef", "/app/cafebabe"], conversationIds: ["c_deadbeef", "c_cafebabe"],
    responseIds: ["r_deadbeef"], sharePaths: ["/share/abcd1234"], retained: "metadata",
  });
  assert.notEqual(target.appPaths, originalPaths);
  assert.notEqual(target.responseIds, incoming.responseIds);
  assert.deepEqual(originalPaths, ["/app/deadbeef"]);
  assert.deepEqual(incoming.appPaths, ["/app/cafebabe", " /app/deadbeef "]);
});

for (const [label, hints, expected] of [
  ["latest concrete app", { appPaths: ["/app/deadbeef", "/app/cafebabe", "/app/cafebabe/extra"], conversationIds: ["c_12345678"], sharePaths: ["/share/abcd1234"] }, "/app/cafebabe"],
  ["conversation fallback", { appPaths: ["/app/short"], conversationIds: ["c_deadbeef", "c_cafebabe"], sharePaths: ["/share/abcd1234"] }, "c_cafebabe"],
  ["share fallback", { sharePaths: ["/share/abcd1234", "/share/abcd5678"] }, "/share/abcd5678"],
  ["empty fallback", {}, null],
]) {
  test(`strongest handle uses ${label} without mutating arrays`, () => {
    for (const values of Object.values(hints)) Object.freeze(values);
    Object.freeze(hints);
    assert.equal(app.strongestProgramHandleHint(hints), expected);
  });
}

for (const [path, ids, expected] of [
  ["/app/deadbeef", ["c_deadbeef", "c_cafebabe"], "c_deadbeef"],
  ["/app/12345678", ["c_deadbeef", "c_cafebabe"], "c_cafebabe"],
  [null, ["c_deadbeef", "c_cafebabe"], "c_cafebabe"],
  ["/app/deadbeef", [], null],
]) {
  test(`conversation ID derivation retains match/fallback for ${path} with ${ids.length} IDs`, () => {
    assert.equal(app.deriveConversationIdForAppPath(path, Object.freeze(ids)), expected);
  });
}

test("handle pairs preserve every supplied metadata field without retaining the source object", () => {
  const source = Object.freeze({
    sourceUrl: "https://fixture.invalid/rpc", sourceRpc: "fixture", sourceKind: "response",
    sourceSurface: "canvas_proxy_client", sourceWsUrl: "wss://fixture.invalid/ws",
    sourceTargetDomain: "fixture.invalid", ts: "2026-09-14T00:00:00.000Z", unrelated: "ignored",
  });
  const { unrelated, ...metadata } = source;
  assert.deepEqual(app.extractProgramHandlePairs(pairText, source), [{ ...defaultPair, ...metadata }]);
  assert.equal(unrelated, "ignored");
});

test("handle pairs accept escaped quotes, deduplicate and retain null defaults", () => {
  const text = String.raw`\"c_deadbeef\" , \"r_cafebabe\"`;
  assert.deepEqual(app.extractProgramHandlePairs(`${text}\n${pairText}`, { sourceUrl: "", ts: 0 }), [defaultPair]);
});

test("handle pairs reject incomplete and malformed pairs", () => {
  assert.deepEqual(app.extractProgramHandlePairs('"c_abcdef0", "r_cafebabe"; "c_deadbeeg", "r_cafebabe"; c_deadbeef r_cafebabe'), []);
});

test("handle pair dedupe replaces metadata at the first key position and preserves references", () => {
  const first = Object.freeze({ ...defaultPair, sourceKind: "request" });
  const second = Object.freeze({ ...defaultPair, responseId: "r_12345678" });
  const replacement = Object.freeze({ ...defaultPair, sourceKind: "response" });
  const differentPath = Object.freeze({ ...defaultPair, appPath: "/app/cafebabe" });
  const input = Object.freeze([null, {}, first, second, replacement, differentPath, { conversationId: "c_deadbeef" }]);
  const result = app.dedupeProgramHandlePairs(input);
  assert.deepEqual(result, [replacement, second, differentPath]);
  assert.equal(result[0], replacement);
  assert.equal(result[1], second);
});

for (const [candidate, expected] of [
  ["/app/deadbeef", true], ["/app/1234567890123", true], [" /app/DEADBEEF ", true],
  ["https://gemini.google.com/app/deadbeef?authuser=2#draft", true],
  ["https://fixture.invalid/u/2/app/deadbeef", true],
  ["/app/deadbeef?authuser=2", false], ["/app/deadbeef/extra", false],
  ["/app/deadbeef/", false], ["/app/short", false], [null, false],
]) {
  test(`concrete handle candidate preserves current URL parsing for ${candidate}`, () => {
    assert.equal(app.isConcreteProgramUrlCandidate(candidate), expected);
  });
}
