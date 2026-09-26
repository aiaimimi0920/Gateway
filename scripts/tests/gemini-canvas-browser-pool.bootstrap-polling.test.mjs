import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { createBootstrapPollingOwner } from "../gemini-canvas-browser-pool-bootstrap-polling.mjs";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const sample = (id, extra = {}) => ({ id, url: "https://fixture.invalid/" + id, handleReady: true, invokeReady: false, contract: {}, transportReady: false, ...extra });

function pollingHarness({ snapshots = [sample("result")], failureAt = null, failureCall = 1 } = {}) {
  const calls = [], counts = new Map(), failure = new Error("fixture polling dependency failure");
  let now = 1000, index = 0, current = null;
  const record = (stage, ...args) => {
    calls.push([stage, ...args]);
    counts.set(stage, (counts.get(stage) ?? 0) + 1);
    if (stage === failureAt && counts.get(stage) === failureCall) throw failure;
  };
  const page = {
    async waitForTimeout(ms) {
      record("wait", ms);
      now += ms;
      if (now > 120000) throw new Error("Fixture virtual-clock budget exceeded");
    },
  };
  const input = {
    activePage: page, captureState: { invokeContract: null, transportHints: { ready: false } },
    baseUrl: "https://fixture.invalid", args: { fixture: true },
    bootstrapOperation: "text", discoveryOnly: false, timeoutMs: 5000,
    initialSnapshot: sample("initial", { handleReady: false }),
    mergeSnapshot(snapshot) {
      record("merge", snapshot);
      input.captureState.invokeContract = snapshot.contract;
      input.captureState.transportHints.ready = snapshot.transportReady;
    },
  };
  const context = {
    Date: { now: () => now },
    async collectProgramHandleSnapshot(nextPage) {
      record("snapshot", nextPage);
      current = snapshots[Math.min(index++, snapshots.length - 1)];
      return current;
    },
    buildProgramHandleState(...args) {
      record("handle", ...args);
      return current.handleReady ? { canvasProgramUrl: current.url, appPath: "/app/fixture", conversationId: "c_fixture" } : {};
    },
    hasConcreteProgramHandleState: app.hasConcreteProgramHandleState,
    invokeContractIndicatesConcreteProgress(...args) { record("progress", ...args); return current.invokeReady; },
    hasTransportHints(transport) { record("transport", transport); return transport.ready; },
  };
  const { pollBootstrapProgram } = createBootstrapPollingOwner(context);
  const run = vm.runInNewContext(`(${pollBootstrapProgram.toString()})`, context, { timeout: 1000 });
  return { input, calls, failure, snapshots, get now() { return now; }, run: (extra = {}) => run({ ...input, ...extra }) };
}

const stages = (h, name) => h.calls.filter(([stage]) => stage === name);

test("Bootstrap polling merges a snapshot before inspecting its handle and invoke state", async () => {
  const h = pollingHarness(), result = await h.run();
  assert.equal(result, h.snapshots[0]);
  assert.deepEqual(h.calls.map(([stage]) => stage), ["wait", "snapshot", "merge", "handle", "progress"]);
  assert.deepEqual(stages(h, "wait"), [["wait", 1800]]);
  assert.equal(stages(h, "snapshot")[0][1], h.input.activePage);
  assert.equal(stages(h, "merge")[0][1], result);
  const handle = stages(h, "handle")[0];
  assert.equal(handle[1], h.input.baseUrl);
  assert.equal(handle[2], h.input.args);
  assert.equal(handle[3], result.url);
  assert.equal(handle[4], h.input.captureState);
  const progress = stages(h, "progress")[0];
  assert.equal(progress[1], "text");
  assert.equal(progress[2], result.contract);
  assert.equal(progress[3], result);
});

for (const timeoutMs of [0, -1, Number.NaN]) {
  test(`Bootstrap polling preserves the initial snapshot without work for timeout ${timeoutMs}`, async () => {
    const h = pollingHarness(), result = await h.run({ timeoutMs });
    assert.equal(result, h.input.initialSnapshot);
    assert.deepEqual(h.calls, []);
  });
}

for (const field of ["transportKind", "actionName", "uiState"]) {
  test(`Bootstrap discovery stops on concrete handle plus ${field}`, async () => {
    const h = pollingHarness({ snapshots: [sample("ready", { contract: { [field]: "fixture" } })] });
    const result = await h.run({ discoveryOnly: true });
    assert.equal(result.id, "ready");
    assert.equal(stages(h, "wait").length, 1);
  });
}

test("Bootstrap discovery accepts transport hints but never breaks without a concrete handle", async () => {
  const h = pollingHarness({ snapshots: [sample("pending", { handleReady: false, transportReady: true }), sample("ready", { transportReady: true })] });
  const result = await h.run({ discoveryOnly: true });
  assert.equal(result.id, "ready");
  assert.equal(stages(h, "wait").length, 2);
  assert.equal(stages(h, "progress").length, 1);
  assert.equal(stages(h, "transport").length, 1);
});

test("Bootstrap discovery with no invocation evidence keeps the last polled snapshot at deadline", async () => {
  const h = pollingHarness({ snapshots: [sample("first"), sample("second"), sample("last")] });
  const result = await h.run({ discoveryOnly: true });
  assert.equal(result, h.snapshots[2]);
  assert.equal(h.now, 6400);
  assert.equal(stages(h, "wait").length, 3);
});

for (const bootstrapOperation of ["text", "image"]) {
  test(`Bootstrap ${bootstrapOperation} polling accepts a concrete handle without media progress`, async () => {
    const h = pollingHarness();
    await h.run({ bootstrapOperation });
    assert.equal(stages(h, "wait").length, 1);
    assert.equal(stages(h, "transport").length, 0);
  });
}

test("Bootstrap media polling accepts transport hints before the stabilization window", async () => {
  const h = pollingHarness({ snapshots: [sample("transport", { transportReady: true })] });
  await h.run({ bootstrapOperation: "music", timeoutMs: 30000 });
  assert.equal(stages(h, "wait").length, 1);
});

test("Bootstrap video polling accepts concrete invoke progress immediately", async () => {
  const h = pollingHarness({ snapshots: [sample("progress", { invokeReady: true })] });
  await h.run({ bootstrapOperation: "video", timeoutMs: 30000 });
  assert.equal(stages(h, "wait").length, 1);
});

test("Bootstrap music polling skips stabilization when the player is ready", async () => {
  const h = pollingHarness({ snapshots: [sample("player", { invokeReady: true, contract: { uiState: "music_player_ready" } })] });
  await h.run({ bootstrapOperation: "music", timeoutMs: 30000 });
  assert.equal(stages(h, "wait").length, 1);
});

test("Bootstrap music polling waits twelve seconds from first accepted progress", async () => {
  const h = pollingHarness({ snapshots: [sample("generating", { invokeReady: true })] });
  await h.run({ bootstrapOperation: "music", timeoutMs: 30000 });
  assert.equal(stages(h, "wait").length, 8);
  assert.equal(h.now, 15400);
});

test("Bootstrap music polling preserves its first progress timestamp across a temporary gap", async () => {
  const h = pollingHarness({ snapshots: [sample("accepted", { invokeReady: true }), sample("gap"), sample("resumed", { invokeReady: true })] });
  const result = await h.run({ bootstrapOperation: "music", timeoutMs: 30000 });
  assert.equal(result.id, "resumed");
  assert.equal(stages(h, "wait").length, 8);
});

test("Bootstrap concrete-handle fallback keeps its first timestamp across a missing-handle sample", async () => {
  const h = pollingHarness({ snapshots: [sample("concrete"), sample("missing", { handleReady: false }), sample("restored")] });
  const result = await h.run({ bootstrapOperation: "music", timeoutMs: 30000 });
  assert.equal(result.id, "restored");
  assert.equal(stages(h, "wait").length, 10);
  assert.equal(h.now, 19000);
});

test("Bootstrap polling honors a shorter deadline while music stabilization is pending", async () => {
  const h = pollingHarness({ snapshots: [sample("generating", { invokeReady: true })] });
  await h.run({ bootstrapOperation: "music", timeoutMs: 5000 });
  assert.equal(stages(h, "wait").length, 3);
  assert.equal(h.now, 6400);
});

for (const failureAt of ["wait", "snapshot", "merge", "handle", "progress", "transport"]) {
  test(`Bootstrap polling propagates ${failureAt} errors without further polling`, async () => {
    const h = pollingHarness({ failureAt });
    await assert.rejects(h.run({ discoveryOnly: true }), (error) => error === h.failure);
    assert.equal(h.calls.at(-1)[0], failureAt);
    assert.equal(stages(h, "wait").length, 1);
  });
}

test("Bootstrap polling actual factory returns the initial snapshot for an elapsed deadline", async () => {
  const { pollBootstrapProgram } = createBootstrapPollingOwner(app);
  const initialSnapshot = sample("initial");
  assert.equal(await pollBootstrapProgram({ timeoutMs: -1, initialSnapshot }), initialSnapshot);
});
