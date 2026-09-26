import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

for (const frontFails of [false, true]) {
  test(`media lease retains a real attached page when bring-to-front failure is ${frontFails}`, browserTest, async (t) => {
    const { entry, page, context } = await appFixture(t);
    entry.attachedCdp = true;
    let frontCalls = 0;
    t.mock.method(page, "bringToFront", async () => { frontCalls += 1; if (frontFails) throw new Error("front unavailable"); });
    t.mock.method(context, "newPage", () => assert.fail("attached page must be reused"));
    const lease = await app.acquireMediaOperationPage(entry, baseUrl);
    assert.equal(lease.page, page);
    assert.equal(lease.closeWhenDone, false);
    assert.equal(frontCalls, 1);
    assert.equal(page.isClosed(), false);
    assert.deepEqual(context.pages(), [page]);
  });

  test(`media lease owns a new real page when bring-to-front failure is ${frontFails}`, browserTest, async (t) => {
    const { entry, page, context } = await appFixture(t);
    const newPage = context.newPage.bind(context);
    let created, frontCalls = 0, createCalls = 0;
    t.mock.method(context, "newPage", async () => {
      createCalls += 1;
      created = await newPage();
      t.mock.method(created, "bringToFront", async () => { frontCalls += 1; if (frontFails) throw new Error("front unavailable"); });
      return created;
    });
    const lease = await app.acquireMediaOperationPage(entry, baseUrl);
    assert.equal(lease.page, created);
    assert.notEqual(lease.page, page);
    assert.equal(lease.closeWhenDone, true);
    assert.equal(createCalls, 1);
    assert.equal(frontCalls, 1);
    assert.equal(entry.page, page);
    assert.deepEqual(context.pages(), [page, created]);
  });
}

test("media lease preserves a page creation failure", async () => {
  const failure = new Error("synthetic new-page failure");
  const entry = { context: { newPage: async () => { throw failure; } } };
  await assert.rejects(app.acquireMediaOperationPage(entry, baseUrl), (error) => error === failure);
});

function closeFixture(t) {
  const timers = [], logs = [], active = new Set();
  t.mock.method(globalThis, "setTimeout", (callback, ms) => {
    timers.push({ callback, ms }); active.add(timers.length); return timers.length;
  });
  t.mock.method(globalThis, "clearTimeout", (id) => active.delete(id));
  t.mock.method(console, "log", (...args) => logs.push(args));
  return { timers, logs, active };
}

test("media page close ignores an absent page without allocating a deadline", async (t) => {
  const f = closeFixture(t);
  await app.closePageSafely(null, "absent");
  assert.deepEqual(f.timers, []);
  assert.deepEqual(f.logs, []);
});

for (const rejects of [false, true]) {
  test(`media page close clears its deadline when close rejection is ${rejects}`, async (t) => {
    const f = closeFixture(t);
    let closes = 0;
    await app.closePageSafely({ close: async () => { closes += 1; if (rejects) throw new Error("close rejected"); } }, "settled");
    assert.equal(closes, 1);
    assert.deepEqual(f.logs, []);
    assert.equal(f.active.size, 0);
    assert.equal(f.timers.length, 1);
    assert.equal(f.timers[0].ms, 3000);
    f.timers[0].callback();
    await Promise.resolve();
    assert.deepEqual(f.logs, []);
  });
}

test("media page close returns at its deadline and allows the close to finish later", async (t) => {
  const f = closeFixture(t);
  let finishClose, closed = false;
  const lateClose = new Promise((resolve) => { finishClose = resolve; }).then(() => { closed = true; });
  const pending = app.closePageSafely({ close: () => lateClose }, "owned-page", 42);
  assert.equal(f.timers[0].ms, 42);
  f.timers[0].callback();
  await pending;
  assert.equal(f.active.size, 0);
  assert.equal(closed, false);
  assert.equal(f.logs.length, 1);
  assert.equal(f.logs[0][1], "page close timed out");
  assert.deepEqual(JSON.parse(f.logs[0][2]), { contextLabel: "owned-page", timeoutMs: 42 });
  finishClose();
  await lateClose;
  assert.equal(closed, true);
  assert.equal(f.logs.length, 1);
});

test("media page close preserves a synchronous close failure before allocating a timer", async (t) => {
  const f = closeFixture(t), failure = new Error("sync close failure");
  await assert.rejects(app.closePageSafely({ close: () => { throw failure; } }, "sync"), (error) => error === failure);
  assert.deepEqual(f.timers, []);
  assert.deepEqual(f.logs, []);
});
