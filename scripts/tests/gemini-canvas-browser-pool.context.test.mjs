import assert from "node:assert/strict";
import test from "node:test";
import { chromium } from "playwright-core";
import { browserSession, contextArgs, contextFixture, deferred, pool } from "./gemini-canvas-browser-pool.context-fixtures.mjs";

test("concurrent requests share one creation and attached disposal only closes the browser connection", async (t) => {
  contextFixture(t);
  const session = browserSession(), ready = deferred(), release = deferred();
  const connect = t.mock.method(chromium, "connectOverCDP", async () => {
    ready.resolve();
    await release.promise;
    return session.browser;
  });
  const first = pool.ensureContext(contextArgs("shared"));
  const second = pool.ensureContext(contextArgs("shared"));
  try {
    await ready.promise;
    assert.equal(connect.mock.callCount(), 1);
    assert.equal(pool.initializingContexts.size, 1);
    assert.equal(pool.contexts.size, 0);
  } finally {
    release.resolve();
  }
  const [left, right] = await Promise.all([first, second]);
  assert.equal(left, right);
  assert.equal(pool.contexts.get("shared"), left);
  assert.equal(pool.initializingContexts.size, 0);
  await pool.closeContext("shared");
  await pool.closeContext("shared");
  assert.deepEqual(session.closes, ["browser"]);
  assert.equal(left.contextClosed, true);
});

test("cached context adopts a surviving page and refreshes last use without another connection", async (t) => {
  contextFixture(t);
  const session = browserSession();
  const connect = t.mock.method(chromium, "connectOverCDP", async () => session.browser);
  const entry = await pool.ensureContext(contextArgs("adopt"));
  const replacement = { isClosed: () => false };
  session.page.closed = true;
  session.pages.push(replacement);
  entry.lastUsedAt = 1;
  assert.equal(await pool.ensureContext(contextArgs("adopt")), entry);
  assert.equal(entry.page, replacement);
  assert.ok(entry.lastUsedAt > 1);
  assert.equal(connect.mock.callCount(), 1);
});

test("stale disconnected context is disposed before its replacement is created", async (t) => {
  contextFixture(t);
  const first = browserSession(), second = browserSession();
  let connections = 0;
  t.mock.method(chromium, "connectOverCDP", async () => {
    if (connections++ === 0) return first.browser;
    assert.deepEqual(first.closes, ["browser"]);
    assert.equal(pool.contexts.has("stale"), false);
    return second.browser;
  });
  const original = await pool.ensureContext(contextArgs("stale"));
  first.browser.connected = false;
  const replacement = await pool.ensureContext(contextArgs("stale"));
  assert.notEqual(replacement, original);
  assert.equal(original.contextClosed, true);
  assert.equal(replacement.browser, second.browser);
  assert.equal(connections, 2);
});

test("late close events from an older entry cannot remove the replacement registry entry", async (t) => {
  contextFixture(t);
  const first = browserSession(), second = browserSession(), sessions = [first, second];
  t.mock.method(chromium, "connectOverCDP", async () => sessions.shift().browser);
  const original = await pool.createContextEntry(contextArgs("replace"));
  const replacement = await pool.createContextEntry(contextArgs("replace"));
  await first.context.close();
  await first.browser.close();
  assert.equal(original.contextClosed, true);
  assert.equal(pool.contexts.get("replace"), replacement);
  second.context.emit("close");
  assert.equal(replacement.contextClosed, true);
  assert.equal(pool.contexts.has("replace"), false);
  await second.browser.close();
});

test("owned disposal removes registry ownership before awaiting failing context and browser closes", async (t) => {
  contextFixture(t);
  const session = browserSession(), entry = { ...session, attachedCdp: false, contextClosed: false };
  pool.contexts.set("owned", entry);
  const calls = [];
  t.mock.method(session.context, "close", async () => {
    assert.equal(pool.contexts.has("owned"), false);
    assert.equal(entry.contextClosed, true);
    calls.push("context");
    throw new Error("fixture context cleanup error");
  });
  t.mock.method(session.browser, "close", async () => { calls.push("browser"); throw new Error("fixture browser cleanup error"); });
  await pool.closeContext("owned");
  await pool.closeContext("owned");
  assert.deepEqual(calls, ["context", "browser"]);
});
