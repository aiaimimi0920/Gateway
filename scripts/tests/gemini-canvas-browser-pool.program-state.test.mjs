import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const base = "https://gemini.google.com";
const shareUrl = `${base}/batchexecute?source-path=%2Fshare%2Fabcdefgh`;
const pair = (id, extra = {}) => Object.freeze({ appPath: `/app/${id.repeat(8)}`, programUrl: `${base}/app/${id.repeat(8)}`, conversationId: `c_${id.repeat(8)}`, responseId: `r_${id.repeat(8)}`, ...extra });
const ordinary = pair("a"), share = pair("b", { sourceUrl: shareUrl }), requested = pair("c", { sourceUrl: shareUrl });
const proxy = pair("d", { sourceSurface: "canvas_proxy_client" }), shareProxy = pair("e", { sourceSurface: "canvas_proxy_client", sourceUrl: shareUrl });

for (const [label, pairs, expected] of [
  ["share proxy", [shareProxy, proxy, requested, share, ordinary], shareProxy],
  ["proxy", [proxy, requested, share, ordinary], proxy],
  ["requested share", [requested, share, ordinary], requested],
  ["share", [share, ordinary], share],
  ["latest complete", [pair("f"), ordinary, pair("1", { responseId: null })], ordinary],
]) {
  test(`program stable selection preserves ${label} precedence without changing input order`, () => {
    const input = Object.freeze(pairs), before = [...pairs];
    assert.equal(app.selectStableProgramPair(input, requested.appPath), expected);
    assert.deepEqual(input, before);
  });
}

test("program stable selection handles empty and incomplete ordinary candidates", () => {
  assert.equal(app.selectStableProgramPair(null), null);
  assert.equal(app.selectStableProgramPair([pair("a", { appPath: null }), pair("b", { responseId: null })]), null);
});

test("program concrete path selection rejects short and missing identifiers", () => {
  assert.equal(app.concreteAppPathFromUrl(`${base}/app/ABCDEF12`), "/app/ABCDEF12");
  assert.equal(app.concreteAppPathFromUrl("/app/1234567890123"), "/app/1234567890123");
  for (const value of [null, "", `${base}/app/new`, "/app/abc1234", "/share/abcdefgh"]) assert.equal(app.concreteAppPathFromUrl(value), null);
});

test("program canonical selection prefers the latest response matching the active page", () => {
  assert.equal(app.selectCanonicalProgramPair([shareProxy, ordinary], ordinary.programUrl), ordinary);
});

test("program canonical selection retains proxy precedence when only an older response matches", () => {
  const latest = pair("f");
  assert.equal(app.selectCanonicalProgramPair([shareProxy, ordinary, latest], ordinary.programUrl), shareProxy);
});

test("program canonical selection retains the latest response fallback without an app path", () => {
  const latest = pair("a", { appPath: null });
  assert.equal(app.selectCanonicalProgramPair([latest], `${base}/share/abcdefgh`), latest);
  assert.equal(app.selectCanonicalProgramPair([], null), null);
});

test("program state assembly returns a complete empty contract and ISO timestamps", () => {
  const result = app.buildProgramHandleState(base, {}, null, null);
  for (const key of ["canvasProgramUrl", "pageUrl", "appPath", "conversationId", "responseId", "lastSeenConversationId", "lastSeenResponseId", "invokeBaseUrl", "musicWsUrl", "videoInvokePath", "stableProgramPair", "latestResponsePair"]) assert.equal(result[key], null, key);
  assert.deepEqual(result.candidatePairs, []);
  assert.equal(new Date(result.capturedAt).toISOString(), result.capturedAt);
  assert.equal(new Date(result.lastValidatedAt).toISOString(), result.lastValidatedAt);
});

test("program state assembly preserves navigation-derived requested URL and conversation input", () => {
  const result = app.buildProgramHandleState(`${base}/`, { appPath: " /app/cccccccc ", conversationId: " c_cccccccc " }, `${base}/share/abcdefgh`, null);
  assert.equal(result.canvasProgramUrl, `${base}/app/cccccccc`);
  assert.equal(result.appPath, "/app/cccccccc");
  assert.equal(result.conversationId, "c_cccccccc");
  assert.equal(result.pageUrl, `${base}/share/abcdefgh`);
  assert.equal(result.responseId, null);
});

test("program state assembly uses latest valid hints and transport candidates without mutation", () => {
  const handleHints = Object.freeze({ appPaths: Object.freeze(["/app/aaaaaaaa", "/app/bbbbbbbb", "/app/new"]), conversationIds: Object.freeze(["c_aaaaaaaa", "c_bbbbbbbb"]), responseIds: Object.freeze(["r_aaaaaaaa", "r_bbbbbbbb"]), sharePaths: Object.freeze([]) });
  const transportHints = Object.freeze({ invokeBaseUrls: Object.freeze(["https://fixture.invalid/old", "https://fixture.invalid/current"]), musicWsUrls: Object.freeze(["wss://fixture.invalid/old", "wss://fixture.invalid/current"]), videoInvokePaths: Object.freeze(["/old", "/current"]) });
  const capture = Object.freeze({ handleHints, transportHints }), before = structuredClone(capture);
  const result = app.buildProgramHandleState(base, {}, null, capture);
  assert.equal(result.appPath, "/app/bbbbbbbb");
  assert.equal(result.canvasProgramUrl, `${base}/app/bbbbbbbb`);
  assert.equal(result.conversationId, "c_bbbbbbbb");
  assert.equal(result.responseId, "r_bbbbbbbb");
  assert.equal(result.lastSeenConversationId, "c_bbbbbbbb");
  assert.equal(result.lastSeenResponseId, "r_bbbbbbbb");
  assert.equal(result.invokeBaseUrl, "https://fixture.invalid/current");
  assert.equal(result.musicWsUrl, "wss://fixture.invalid/current");
  assert.equal(result.videoInvokePath, "/current");
  assert.deepEqual(capture, before);
});

test("program state assembly keeps stable and latest identities distinct after dedupe", () => {
  const replacement = Object.freeze({ ...shareProxy, sourceKind: "response" });
  const input = Object.freeze([shareProxy, ordinary, replacement, pair("f", { responseId: null })]);
  const result = app.buildProgramHandleState(base, {}, `${base}/share/abcdefgh`, { handlePairs: input });
  assert.deepEqual(result.candidatePairs, [replacement, ordinary]);
  assert.notEqual(result.candidatePairs, input);
  assert.equal(result.stableProgramPair, replacement);
  assert.equal(result.latestResponsePair, ordinary);
  assert.equal(result.canvasProgramUrl, replacement.programUrl);
  assert.equal(result.appPath, replacement.appPath);
  assert.equal(result.conversationId, replacement.conversationId);
  assert.equal(result.responseId, replacement.responseId);
  assert.equal(result.lastSeenConversationId, ordinary.conversationId);
  assert.equal(result.lastSeenResponseId, ordinary.responseId);
});
