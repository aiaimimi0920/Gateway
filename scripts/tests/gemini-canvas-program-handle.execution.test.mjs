import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import { executionFixture } from "./gemini-canvas-program-handle.execution-fixtures.mjs";

const count = (f, name) => f.calls.filter(([kind]) => kind === name).length;

test("standalone execution persists five artifacts before stdout and closes its context", async (t) => {
  const f = await executionFixture(t);
  assert.equal(await f.run(), undefined);
  assert.deepEqual(f.writes, ["network-requests.json", "network-responses.json", "program-rpc-captures.json", "summary.json", "program-handle.json"]);
  const summary = f.read("summary.json"), handle = f.read("program-handle.json");
  assert.deepEqual(Object.keys(summary).sort(), [
    "ok", "profileDir", "executablePath", "outDir", "shareUrl", "appUrl", "operation", "prompt", "discoveryOnly",
    "shareFollowKind", "newChatClicked", "modeSelected", "beforeUrl", "finalUrl", "appPath", "strongestHandle",
    "aggregateHints", "transportHints", "candidatePairs", "stableProgramPair", "latestResponsePair",
    "canvasProgramAction", "canvasProgramActionInput", "canvasProgramInvokeContract", "rpcCaptures", "networkSummary", "note",
  ].sort());
  assert.equal(summary.ok, true);
  assert.equal(summary.appPath, "/app/abcdef123456");
  assert.equal(handle.runtimeStateObjectKey, "fixture/profile");
  assert.equal(handle.programUrl, "https://gemini.google.com/app/abcdef123456");
  assert.equal(handle.runtimeProfileDir, "fixture-profile");
  assert.equal(handle.cookieHeader, null);
  assert.deepEqual(summary.networkSummary, { requestCount: 0, responseCount: 0, rpcCaptureCount: 0 });
  assert.deepEqual(f.read("network-requests.json"), []);
  assert.deepEqual(f.printed, [JSON.stringify(summary, null, 2)]);
  assert.deepEqual(f.calls.slice(-3).map(([name]) => name), ["stdout", "capture stop", "context close"]);
  assert.equal(count(f, "submit"), 0);
  assert.equal(count(f, "context close"), 1);
  assert.deepEqual(structuredClone(f.calls[0]), ["launch", "fixture-profile", {
    executablePath: "fixture-browser", headless: false, locale: "zh-CN",
    args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
  }]);
});

test("standalone execution creates a page only for an empty context", async (t) => {
  const f = await executionFixture(t, { emptyContext: true });
  await f.run();
  assert.equal(count(f, "new page"), 1);
  assert.equal(count(f, "capture"), 1);
});

test("standalone execution popup adoption retains its context capture until final cleanup", async (t) => {
  const f = await executionFixture(t, { follow: ["popup"] });
  await f.run();
  assert.equal(f.captures.length, 1);
  for (const event of ["request", "response", "websocket"]) {
    assert.equal(f.initial.listenerCount(event), 0);
    assert.equal(f.popup.listenerCount(event), 0);
  }
  assert.equal(f.initial.closed, true);
  assert.equal(count(f, "capture stop"), 1);
  assert.equal(f.read("summary.json").shareFollowKind, "popup");
  assert.deepEqual(f.calls.filter(([kind]) => ["capture stop", "capture", "capture adopt", "page close"].includes(kind)).map(([kind, page]) => [kind, page]), [
    ["capture", "initial"], ["capture adopt", "popup"], ["page close", "initial"], ["capture stop", "initial"],
  ]);
});

test("standalone execution proxy discovery reuses the before snapshot and exits on transport", async (t) => {
  const f = await executionFixture(t, {
    seedState(state) { state.handlePairs.push({ sourceSurface: "canvas_proxy_client" }); state.transportHints.musicWsUrls.push("wss://fixture.invalid/ws"); },
  });
  await f.run();
  assert.equal(count(f, "new chat"), 0);
  assert.equal(count(f, "mode"), 0);
  assert.equal(count(f, "snapshot after-new-chat"), 0);
  assert.equal(count(f, "snapshot after"), 1);
  assert.equal(f.read("summary.json").newChatClicked, false);
});

test("standalone execution generation falls back to app only when hints are absent", async (t) => {
  const f = await executionFixture(t, { discoveryOnly: false, snapshot: () => ({ url: "https://gemini.google.com/app", bodyText: "" }) });
  await f.run();
  assert.deepEqual(f.calls.filter(([kind]) => kind === "goto").map(([, , url]) => url), [f.runtime.shareUrl, f.runtime.appUrl]);
  assert.equal(count(f, "submit"), 1);
  assert.equal(f.read("summary.json").appPath, null);
  assert.equal(f.read("summary.json").ok, true);
});

test("standalone execution music style snapshot precedes prompt submission", async (t) => {
  const f = await executionFixture(t, { operation: "music", discoveryOnly: false, snapshot: (label) => ({ bodyText: label === "after-new-chat" ? "选择要混合制作的曲目" : "" }) });
  await f.run();
  assert.equal(count(f, "style"), 1);
  assert.ok(f.calls.findIndex(([kind]) => kind === "snapshot after-style-selection") < f.calls.findIndex(([kind]) => kind === "submit"));
});

for (const operation of ["music", "video"]) {
  test("standalone execution " + operation + " player probes play then download without a target", async (t) => {
    const f = await executionFixture(t, { operation, discoveryOnly: false, invoke: () => ({ uiState: operation + "_player_ready" }) });
    await f.run();
    assert.deepEqual(f.calls.filter(([kind]) => ["play", "download"].includes(kind)).map(([kind]) => kind), ["play", "download"]);
    assert.equal(count(f, "snapshot after-play"), 1);
    assert.equal(count(f, "snapshot after-download"), 1);
    assert.equal(count(f, "snapshot after"), 1);
  });
}

test("standalone execution refreshed play target suppresses download", async (t) => {
  const f = await executionFixture(t, {
    operation: "video", discoveryOnly: false,
    invoke: (_operation, _action, _transport, snapshot) => ({ uiState: "video_player_ready", ...(snapshot.label === "after-play" ? { target: { url: "https://fixture.invalid/video.mp4" } } : {}) }),
  });
  await f.run();
  assert.equal(count(f, "play"), 1);
  assert.equal(count(f, "download"), 0);
  assert.equal(f.read("summary.json").canvasProgramInvokeContract.target.url, "https://fixture.invalid/video.mp4");
});

test("standalone execution timeout retains the last polled snapshot without another capture", async (t) => {
  const f = await executionFixture(t, { timeoutMs: 3600, snapshot: (_label, index) => ({ bodyText: "snapshot " + index }) });
  await f.run();
  assert.equal(count(f, "snapshot after"), 2);
  assert.equal(f.snapshots.at(-1).label, "after");
  assert.equal(f.read("summary.json").finalUrl, f.snapshots.at(-1).url);
});

for (const failAt of ["pages", "capture", "capture ready", "goto", "snapshot share-before", "follow", "snapshot before", "new chat", "mode", "snapshot after", "invoke", "object key", "stdout"]) {
  test("standalone execution closes context and preserves rejection at " + failAt, async (t) => {
    const f = await executionFixture(t, { failAt });
    await assert.rejects(f.run(), (error) => error === f.failure);
    assert.equal(count(f, "context close"), 1);
  });
}

test("standalone execution launch rejection owns no context or output", async (t) => {
  const f = await executionFixture(t, { failAt: "launch" });
  await assert.rejects(f.run(), (error) => error === f.failure);
  assert.equal(count(f, "context close"), 0);
  assert.deepEqual(fs.readdirSync(f.outDir), []);
});

test("standalone execution summary write rejection retains preceding capture artifacts", async (t) => {
  const f = await executionFixture(t, { failAt: "write summary.json" });
  await assert.rejects(f.run(), (error) => error === f.failure);
  assert.deepEqual(f.writes, ["network-requests.json", "network-responses.json", "program-rpc-captures.json"]);
  assert.deepEqual(f.printed, []);
  assert.equal(count(f, "context close"), 1);
});

test("standalone execution suppresses context close rejection after successful output", async (t) => {
  const f = await executionFixture(t, { failAt: "context close" });
  await f.run();
  assert.equal(count(f, "context close"), 1);
  assert.equal(f.printed.length, 1);
});
