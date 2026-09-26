import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { bootstrapHarness, bootstrapSnapshot, programUrl } from "./gemini-canvas-browser-pool.bootstrap-fixtures.mjs";

const app = await importTestableScript();
const stages = (h, name) => h.calls.filter(([stage]) => stage === name);

function assertCleanup(h) {
  for (const capture of h.captures) assert.equal(capture.stops, 1);
  for (const page of h.pages) {
    for (const event of ["request", "response", "websocket"]) assert.deepEqual(page.listeners(event), [page.inherited]);
  }
}

function assertSnapshotMerges(h, expected) {
  const sequences = [];
  let aggregateHints;
  for (let i = 0; i < h.calls.length; i += 1) {
    if (h.calls[i][0] !== "hints" || h.calls[i + 1]?.[0] !== "extract") continue;
    const slice = h.calls.slice(i, i + 5);
    assert.deepEqual(slice.map(([stage]) => stage), ["hints", "extract", "action", "build", "invoke"]);
    const build = slice[3][1], snapshot = build.snapshot;
    aggregateHints ??= slice[0][1];
    assert.equal(slice[0][1], aggregateHints);
    assert.equal(slice[0][2], snapshot.handleHints);
    assert.equal(slice[1][1], snapshot.bodyText);
    assert.equal(slice[2][1], build.state.actionContract);
    assert.equal(build.action, build.state.actionContract);
    assert.equal(build.transport, build.state.transportHints);
    assert.equal(build.state, h.captures.at(-1).state);
    assert.equal(build.prompt, "fixture prompt");
    assert.equal(slice[4][2].actionName, build.actionAtBuild.canvasProgramAction);
    sequences.push(snapshot);
  }
  assert.deepEqual(sequences.map((snapshot) => snapshot.id), expected.map((snapshot) => snapshot.id));
  sequences.forEach((snapshot, index) => assert.equal(snapshot, expected[index]));
}

test("Bootstrap merges direct-page snapshots in order before building invoke contracts", async (t) => {
  const h = bootstrapHarness(t, app), result = await h.run();
  assertSnapshotMerges(h, h.snapshots);
  assert.equal(stages(h, "build")[0][1].actionAtBuild.canvasProgramAction, "before");
  assert.equal(result.canvasProgramAction, "before");
  assert.equal(result.canvasProgramActionInput, "before-input");
  assert.deepEqual(result.aggregateHints.responseIds, ["before", "new-chat", "poll"]);
  assert.equal(result.shareFollowKind, "direct_program_page");
  assert.equal(result.bootstrapPrompt, "fixture prompt");
  assert.equal(result.pageUrl, programUrl);
  assert.equal(result.capturedAt, "fixture-captured");
  assert.equal(stages(h, "submit").length, 0);
  assert.equal(stages(h, "mode").length, 0);
  assertCleanup(h);
});

test("Bootstrap share popup transfers one accumulated capture state before closing the old page", async (t) => {
  const snapshots = ["share-before", "share-after", "before", "new-chat", "poll"].map((id) => bootstrapSnapshot(id));
  const h = bootstrapHarness(t, app, { snapshots, followPopup: true });
  const result = await h.run({ programPageUrl: null, shareId: "fixture" });
  assertSnapshotMerges(h, snapshots);
  assert.equal(h.entry.page, h.popup);
  assert.equal(h.captures[0].state, h.captures[1].state);
  assert.equal(result.shareFollowKind, "popup");
  assert.deepEqual(h.calls.filter(([stage]) => ["capture", "stop", "close"].includes(stage)).map(([stage, page]) => [stage, page.name]), [
    ["capture", "initial"], ["stop", "initial"], ["capture", "popup"], ["close", "initial"], ["stop", "popup"],
  ]);
  assertCleanup(h);
});

test("Bootstrap fills action fields from separate later snapshots before invoke construction", async (t) => {
  const snapshots = [
    bootstrapSnapshot("before", { bodyText: "plain" }),
    bootstrapSnapshot("action", { bodyText: '"action": "later"' }),
    bootstrapSnapshot("input", { bodyText: '"action": "ignored"\n"action_input": "later-input"' }),
  ];
  const h = bootstrapHarness(t, app, { snapshots }), result = await h.run();
  assertSnapshotMerges(h, snapshots);
  const actions = stages(h, "build").slice(0, 3).map(([, build]) => build.actionAtBuild);
  assert.deepEqual(actions, [
    { canvasProgramAction: null, canvasProgramActionInput: null },
    { canvasProgramAction: "later", canvasProgramActionInput: null },
    { canvasProgramAction: "later", canvasProgramActionInput: "later-input" },
  ]);
  assert.equal(result.canvasProgramAction, "later");
  assert.equal(result.canvasProgramActionInput, "later-input");
  assertCleanup(h);
});

test("Bootstrap proxy retry preserves before-snapshot alias and skips prompt and mode changes", async (t) => {
  const snapshots = ["before", "retry", "poll"].map((id) => bootstrapSnapshot(id));
  const h = bootstrapHarness(t, app, { snapshots, stay: true, followPopup: true });
  const result = await h.run({ shareId: "fixture", preferExistingProgramPage: true });
  assertSnapshotMerges(h, [snapshots[0], snapshots[1], snapshots[0], snapshots[2]]);
  assert.equal(result.shareMaterializationRetry.attempted, true);
  assert.equal(result.shareMaterializationRetry.shareFollowKind, "popup");
  assert.equal(result.newChatClicked, false);
  assert.equal(result.modeSelected, false);
  for (const name of ["new-chat", "mode", "submit"]) assert.equal(stages(h, name).length, 0);
  assertCleanup(h);
});

for (const bridgeEvents of [1, 2]) {
  test(`Bootstrap preview direct launch preserves early result and cleanup with ${bridgeEvents} bridge events`, async (t) => {
    const before = bootstrapSnapshot("before", { bodyText: "Browser API Proxy Client" });
    const snapshots = [before, bootstrapSnapshot("preview"), ...(bridgeEvents === 1 ? [bootstrapSnapshot("stamp")] : []), bootstrapSnapshot("adopted")];
    const h = bootstrapHarness(t, app, { snapshots, stay: true, bridgeEvents, directLaunch: true });
    const result = await h.run({ launchCanvasProxyPreview: true });
    assertSnapshotMerges(h, snapshots.slice(0, -1));
    assert.equal(h.entry.page, h.preview);
    assert.equal(result.canvasProxyPreview.directLaunch.launched, true);
    assert.equal("page" in result.canvasProxyPreview.directLaunch, false);
    assert.equal(result.bodyText, snapshots.at(-1).bodyText);
    assert.equal(result.canvasProgramAction, "preview");
    assert.equal("rpcCaptures" in result, false);
    assert.equal("capturedAt" in result, false);
    assert.equal(stages(h, "stamp").length, bridgeEvents === 1 ? 1 : 0);
    for (const name of ["new-chat", "mode", "submit", "progress"]) assert.equal(stages(h, name).length, 0);
    assertCleanup(h);
  });
}

test("Bootstrap music style play and download snapshots refresh the returned invoke contract", async (t) => {
  const snapshots = [bootstrapSnapshot("before"), bootstrapSnapshot("new-chat", { bodyText: "选择要混合制作的曲目" }), bootstrapSnapshot("style"), bootstrapSnapshot("poll"), bootstrapSnapshot("play"), bootstrapSnapshot("download", { target: "https://fixture.invalid/audio.wav" })];
  const h = bootstrapHarness(t, app, { snapshots });
  const result = await h.run({ discoveryOnly: false, bootstrapOperation: "music" });
  assertSnapshotMerges(h, snapshots);
  assert.deepEqual(h.calls.filter(([stage]) => ["new-chat", "mode", "style", "submit", "play", "download"].includes(stage)).map(([stage]) => stage), ["new-chat", "mode", "style", "submit", "play", "download"]);
  assert.equal(result.canvasProgramInvokeContract.target, "https://fixture.invalid/audio.wav");
  assert.equal(result.bodyText, snapshots.at(-1).bodyText);
  assert.equal(result.canvasProgramAction, "before");
  assertCleanup(h);
});

test("Bootstrap skips download once the play snapshot supplies a concrete media target", async (t) => {
  const snapshots = ["before", "new-chat", "poll", "play"].map((id) => bootstrapSnapshot(id, id === "play" ? { target: "https://fixture.invalid/audio.wav" } : {}));
  const h = bootstrapHarness(t, app, { snapshots });
  const result = await h.run({ discoveryOnly: false, bootstrapOperation: "music" });
  assertSnapshotMerges(h, snapshots);
  assert.equal(stages(h, "download").length, 0);
  assert.equal(result.canvasProgramInvokeContract.target, "https://fixture.invalid/audio.wav");
  assertCleanup(h);
});

for (const rejectProbe of [false, true]) {
  test(`Bootstrap retains adopted capture until the final media probe ${rejectProbe ? "rejects" : "resolves"}`, async (t) => {
    const entered = Promise.withResolvers(), gate = Promise.withResolvers();
    const lateFailure = new Error("fixture final media probe failure");
    const target = "https://fixture.invalid/audio.wav";
    const snapshots = ["share-before", "share-after", "before", "new-chat", "poll", "play"]
      .map((id) => bootstrapSnapshot(id, id === "play" ? { target } : {}));
    const h = bootstrapHarness(t, app, {
      snapshots, followPopup: true,
      async onMediaAction(action) {
        if (action !== "play") return;
        entered.resolve();
        await gate.promise;
      },
    });
    t.after(() => gate.resolve());
    const pending = h.run({ programPageUrl: null, shareId: "fixture", discoveryOnly: false, bootstrapOperation: "music" });
    const outcome = rejectProbe ? assert.rejects(pending, (error) => error === lateFailure) : pending;
    await entered.promise;
    assert.equal(h.captures.length, 2);
    assert.equal(h.captures[0].stops, 1);
    assert.equal(h.captures[1].stops, 0);
    assert.equal(h.entry.page, h.popup);
    const lateEvent = { kind: "fixture_final_media_response" };
    h.captures[1].state.events.push(lateEvent);
    if (rejectProbe) gate.reject(lateFailure);
    else gate.resolve();
    const result = await outcome;
    if (!rejectProbe) {
      assert.equal(result.canvasProgramInvokeContract.target, target);
      assert.deepEqual(result.networkEvents.at(-1), lateEvent);
      assert.equal(stages(h, "download").length, 0);
    }
    assertCleanup(h);
  });
}

test("Bootstrap rejects absent page and share configuration before acquiring capture", async (t) => {
  const h = bootstrapHarness(t, app);
  await assert.rejects(h.run({ programPageUrl: null }), { status: 400, code: "gemini_canvas_missing_share_id" });
  assert.equal(h.captures.length, 0);
  assertCleanup(h);
});

test("Bootstrap unmaterialized share fails with its original code and stops capture", async (t) => {
  const h = bootstrapHarness(t, app, { unmaterialized: true, hasTextbox: false });
  await assert.rejects(h.run({ programPageUrl: null, shareId: "fixture" }), { status: 502, code: "gemini_canvas_program_bootstrap_share_follow_failed" });
  assert.equal(stages(h, "follow").length, 2);
  assertCleanup(h);
});

test("Bootstrap missing concrete handle waits only until deadline and preserves last body diagnostics", async (t) => {
  const snapshot = bootstrapSnapshot("missing", { handleHints: { appPaths: [], conversationIds: [], responseIds: [], sharePaths: [] } });
  const h = bootstrapHarness(t, app, { missingHandle: true, snapshots: [snapshot] });
  await assert.rejects(h.run(), { status: 504, code: "gemini_canvas_program_bootstrap_handle_missing", bodyText: snapshot.bodyText });
  assert.equal(stages(h, "snapshot").length, 5);
  assert.equal(h.now, 8400);
  assertCleanup(h);
});

test("Bootstrap direct preview missing handle preserves diagnostics and releases both captures", async (t) => {
  const snapshot = bootstrapSnapshot("missing", {
    bodyText: "Browser API Proxy Client",
    handleHints: { appPaths: [], conversationIds: [], responseIds: [], sharePaths: [] },
  });
  const h = bootstrapHarness(t, app, { missingHandle: true, snapshots: [snapshot], stay: true, directLaunch: true });
  await assert.rejects(h.run({ launchCanvasProxyPreview: true }), {
    status: 504, code: "gemini_canvas_program_bootstrap_handle_missing", bodyText: snapshot.bodyText,
  });
  assert.equal(h.captures.length, 2);
  assert.equal(h.entry.page, h.preview);
  assertCleanup(h);
});

test("Bootstrap music accepted progress retains its twelve-second stabilization window", async (t) => {
  const h = bootstrapHarness(t, app, { uiState: "music_generating" });
  await h.run({ discoveryOnly: false, bootstrapOperation: "music", timeoutMs: 30000 });
  assert.equal(stages(h, "progress").length, 8);
  assert.equal(h.now, 17400);
  assert.equal(stages(h, "play").length, 0);
  assertCleanup(h);
});

for (const failureAt of ["program", "snapshot", "hints", "extract", "action", "build", "invoke", "new-chat", "submit"]) {
  test(`Bootstrap releases owned capture when ${failureAt} throws`, async (t) => {
    const h = bootstrapHarness(t, app, { failureAt });
    await assert.rejects(h.run({ discoveryOnly: false }), (error) => error === h.failure);
    assertCleanup(h);
  });
}

test("Bootstrap merge failure after popup adoption stops both acquired captures exactly once", async (t) => {
  const h = bootstrapHarness(t, app, { followPopup: true, failureAt: "build", failureCall: 2 });
  await assert.rejects(h.run({ programPageUrl: null, shareId: "fixture" }), (error) => error === h.failure);
  assert.equal(h.entry.page, h.popup);
  assert.equal(h.captures.length, 2);
  assertCleanup(h);
});

for (const [failureAt, failureCall] of [["snapshot", 3], ["build", 3], ["progress", 1]]) {
  test(`Bootstrap outer cleanup covers polling ${failureAt} rejection`, async (t) => {
    const h = bootstrapHarness(t, app, { failureAt, failureCall });
    await assert.rejects(h.run(), (error) => error === h.failure);
    assert.equal(stages(h, failureAt).length, failureCall);
    assertCleanup(h);
  });
}
