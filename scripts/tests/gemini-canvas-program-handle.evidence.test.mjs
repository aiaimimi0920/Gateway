import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const app = await importTestableProgramHandle();
const idA = "deadbeef", idB = "cafebabe", idC = "01234567";
const pair = (id, extras = {}) => Object.freeze({ appPath: "/app/" + id, conversationId: "c_" + id, responseId: "r_" + id, ...extras });
const share = "https://gemini.google.com/batchexecute?source-path=%2Fshare%2Fabcdefgh";

test("program handle evidence dedupe keeps strict array input and scalar coercion", () => {
  assert.deepEqual(app.uniqueStrings([" a ", "a", "", null, 0, false, 2]), ["a", "2"]);
  assert.throws(() => app.uniqueStrings(null), TypeError);
});

for (const [name, input, expected] of [
  ["extractAppPathsFromText", "https://gemini.google.com/app/deadbeef /app/1234567890123 /app/short", ["/app/deadbeef", "/app/1234567890123"]],
  ["extractConversationIdsFromText", "c_deadbeef c_deadbeef c_short c_CAFEBABE", ["c_deadbeef", "c_CAFEBABE"]],
  ["extractResponseIdsFromText", "r_deadbeef r_deadbeef r_short", ["r_deadbeef"]],
  ["extractSharePathsFromText", "/share/ABCDEFGH /share/short /share/ABCDEFGH", ["/share/ABCDEFGH"]],
]) {
  test("program handle evidence " + name + " resets global regex after repeated scans", () => {
    assert.deepEqual(app[name](input), expected);
    assert.deepEqual(app[name](null), []);
    assert.deepEqual(app[name](input), expected);
  });
}

test("program handle evidence aggregates explicit and conversation-derived app paths", () => {
  assert.deepEqual(app.extractHandleHintsFromText("/app/cafebabe c_deadbeef r_01234567 /share/abcdefgh"), {
    appPaths: ["/app/cafebabe", "/app/deadbeef"], conversationIds: ["c_deadbeef"], responseIds: ["r_01234567"], sharePaths: ["/share/abcdefgh"],
  });
});

for (const [raw, expected] of [[" https://fixture.test/v1/models/x?key=synthetic#fragment ", "https://fixture.test/v1/models/x"], ["wss://fixture.test/music?key=synthetic", "wss://fixture.test/music"], ["not a URL", "not a URL"], [null, null], [" ", null]]) {
  test("program handle evidence transport sanitization " + JSON.stringify(raw), () => assert.equal(app.sanitizeTransportHintUrl(raw), expected));
}

test("program handle evidence transport paths retain model prefix and endpoint predicates", () => {
  assert.equal(app.deriveInvokeBaseUrlFromRequestUrl("https://api.test/v1/models/a:generate?key=synthetic"), "https://api.test/v1");
  assert.equal(app.deriveInvokeBaseUrlFromRequestUrl("https://api.test/models/a"), null);
  assert.equal(app.deriveVideoInvokePathFromRequestUrl("https://api.test/v1/models/a:predictLongRunning?key=synthetic"), "/v1/models/a:predictLongRunning");
  assert.equal(app.deriveVideoInvokePathFromRequestUrl("https://api.test/v1/models/a:predict"), null);
  assert.equal(app.deriveMusicWsUrlFromRequestUrl("wss://api.test/BidiGenerateMusic?key=synthetic#x"), "wss://api.test/BidiGenerateMusic");
  assert.equal(app.deriveMusicWsUrlFromRequestUrl("wss://api.test/other"), null);
  for (const name of ["deriveInvokeBaseUrlFromRequestUrl", "deriveVideoInvokePathFromRequestUrl", "deriveMusicWsUrlFromRequestUrl"]) assert.equal(app[name]("bad URL"), null);
  assert.deepEqual(app.extractTransportHintsFromUrl("https://api.test/v1/models/a:predictLongRunning?key=synthetic"), { invokeBaseUrl: "https://api.test/v1", musicWsUrl: null, videoInvokePath: "/v1/models/a:predictLongRunning" });
});

test("program handle evidence transport merge initializes empty arrays and mutates target", () => {
  const target = {}, incoming = Object.freeze({ invokeBaseUrl: "https://api.test/v1" });
  assert.equal(app.mergeTransportHints(target, incoming), undefined);
  assert.deepEqual(target, { invokeBaseUrls: [incoming.invokeBaseUrl], musicWsUrls: [], videoInvokePaths: [] });
  app.mergeTransportHints(target, incoming);
  assert.equal(target.invokeBaseUrls.length, 1);
  assert.throws(() => app.mergeTransportHints({}, null), TypeError);
});

for (const [raw, prefix, expected] of [["deadbeef", "c_", "c_deadbeef"], [" --__deadbeef ", "c_", "c_deadbeef"], ["c_deadbeef", "c_", "c_deadbeef"], ["C_DEADBEEF", "c_", null], ["r_cafebabe", "r_", "r_cafebabe"], ["short", "c_", null], [null, "c_", null]]) {
  test("program handle evidence ID normalization " + JSON.stringify(raw), () => assert.equal(app.normalizeHandleId(raw, prefix), expected));
}

test("program handle evidence pair builder preserves nullish versus falsy metadata", () => {
  const source = Object.freeze({ sourceUrl: "", sourceRpc: 0, sourceKind: false, sourceSurface: "canvas_proxy_client", sourceWsUrl: "wss://fixture.test", sourceTargetDomain: "fixture.test", ts: 0 });
  assert.deepEqual(app.buildHandlePair("deadbeef", "cafebabe", source), { appPath: "/app/deadbeef", programUrl: "https://gemini.google.com/app/deadbeef", conversationId: "c_deadbeef", responseId: "r_cafebabe", ...source });
  assert.equal(app.buildHandlePair("bad", "r_deadbeef"), null);
  assert.equal(app.buildHandlePair("c_deadbeef", "bad"), null);
});

test("program handle evidence escaped pair scan repeats and retains source metadata", () => {
  const text = String.raw`[\"c_deadbeef\",\"r_cafebabe\"] ["c_deadbeef","r_cafebabe"] ["c_short","r_short"]`;
  const expected = [app.buildHandlePair("c_deadbeef", "r_cafebabe", { ts: 0, sourceUrl: "" })];
  assert.deepEqual(app.extractHandlePairsFromText(text, { ts: 0, sourceUrl: "" }), expected);
  assert.deepEqual(app.extractHandlePairsFromText(null), []);
  assert.deepEqual(app.extractHandlePairsFromText(text, { ts: 0, sourceUrl: "" }), expected);
});

test("program handle evidence hint merge replaces arrays without mutating incoming", () => {
  const old = Object.freeze(["/app/deadbeef"]), target = { appPaths: old };
  const incoming = Object.freeze({ appPaths: Object.freeze(["/app/deadbeef", "/app/cafebabe"]), conversationIds: Object.freeze(["c_cafebabe"]) });
  app.mergeHints(target, incoming);
  assert.deepEqual(target, { appPaths: ["/app/deadbeef", "/app/cafebabe"], conversationIds: ["c_cafebabe"], responseIds: [], sharePaths: [] });
  assert.notEqual(target.appPaths, old);
});

test("program handle evidence pair dedupe replaces last metadata at first-key position", () => {
  const first = pair(idA, { ts: 1 }), other = pair(idB), replacement = pair(idA, { ts: 2 });
  const input = Object.freeze([first, null, {}, other, replacement]);
  assert.deepEqual(app.dedupeHandlePairs(input), [replacement, other]);
  const target = [first, other], identity = target;
  assert.equal(app.mergeHandlePairs(target, [replacement]), undefined);
  assert.equal(target, identity); assert.deepEqual(target, [replacement, other]);
});

test("program handle evidence concrete app path keeps first textual match and anchored grammar", () => {
  assert.equal(app.extractAppPath(null), null);
  assert.equal(app.extractAppPath("https://fixture.test/app/deadbeef /app/cafebabe"), "/app/deadbeef");
  assert.equal(app.concreteAppPathFromUrl("https://gemini.google.com/app/deadbeef?x=1"), "/app/deadbeef");
  assert.equal(app.concreteAppPathFromUrl("https://gemini.google.com/app"), null);
});

test("program handle evidence stable pair prioritizes share proxy then proxy then preferred share", () => {
  const ordinary = pair(idA), preferred = pair(idB, { sourceUrl: share });
  const proxy = pair(idC, { sourceSurface: "canvas_proxy_client" });
  const shareProxy = pair("89abcdef", { sourceSurface: "canvas_proxy_client", sourceUrl: share });
  assert.equal(app.selectProgramConversationPair(Object.freeze([ordinary, preferred, proxy, shareProxy]), preferred.appPath), shareProxy);
  assert.equal(app.selectProgramConversationPair(Object.freeze([preferred, proxy]), preferred.appPath), proxy);
  const laterShare = pair(idC, { sourceUrl: share });
  assert.equal(app.selectProgramConversationPair(Object.freeze([preferred, laterShare]), preferred.appPath), preferred);
  assert.equal(app.selectProgramConversationPair(Object.freeze([ordinary, laterShare]), "/app/missing"), laterShare);
  const latest = pair(idB);
  assert.equal(app.selectProgramConversationPair(Object.freeze([ordinary, latest])), latest);
});

test("program handle evidence selection preserves incomplete share and latest-response fallback", () => {
  const incompleteShare = Object.freeze({ sourceUrl: share, appPath: "/app/deadbeef" });
  const complete = pair(idA), responseOnly = Object.freeze({ conversationId: "c_cafebabe", responseId: "r_cafebabe" });
  assert.equal(app.selectProgramConversationPair([complete, incompleteShare]), incompleteShare);
  assert.equal(app.selectLatestResponsePair([complete, responseOnly]), responseOnly);
  assert.equal(app.selectProgramConversationPair([]), null);
  assert.equal(app.selectLatestResponsePair(null), null);
});

test("program handle evidence canonical final-page pair overrides earlier proxy", () => {
  const proxy = pair(idA, { sourceSurface: "canvas_proxy_client", sourceUrl: share }), final = pair(idB);
  const input = Object.freeze([proxy, final]);
  assert.equal(app.selectCanonicalProgramPair(input, "https://gemini.google.com" + final.appPath), final);
  assert.equal(app.selectCanonicalProgramPair(input, "https://gemini.google.com/app"), proxy);
  assert.equal(app.selectCanonicalProgramPair([], "https://gemini.google.com/app"), null);
});

test("program handle evidence proxy candidates include contract-only and metadata-only forms", () => {
  assert.equal(app.hasCanvasProxyProgramCandidate([], { transportKind: "canvas_program_ws_candidate" }), true);
  assert.equal(app.hasCanvasProxyProgramCandidate([null, { sourceSurface: "canvas_proxy_client" }]), true);
  assert.equal(app.hasCanvasProxyProgramCandidate([], {}), false);
});

test("program handle evidence share and conversation derivation preserve trailing-slash fallback", () => {
  assert.equal(app.deriveShareId("https://gemini.google.com/share/ABCDEFGH?x=1"), "ABCDEFGH");
  assert.equal(app.deriveShareId("/share/short"), null);
  const ids = Object.freeze(["c_deadbeef", "c_cafebabe"]);
  assert.equal(app.deriveConversationIdForAppPath("/app/deadbeef", ids), "c_deadbeef");
  assert.equal(app.deriveConversationIdForAppPath("/app/deadbeef/", ids), "c_cafebabe");
  assert.equal(app.deriveConversationIdForAppPath("/app/ deadbeef ", ids), "c_deadbeef");
  assert.equal(app.deriveConversationIdForAppPath(null, []), null);
});
