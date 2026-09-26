import assert from "node:assert/strict";
import test from "node:test";
import { chromium } from "playwright-core";
import { browserSession, contextArgs, contextFixture, deferred, pool } from "./gemini-canvas-browser-pool.context-fixtures.mjs";

test("capacity evicts the least recently used idle entry while retaining a busy entry", async (t) => {
  contextFixture(t);
  process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS = "2";
  const sessions = [];
  t.mock.method(chromium, "connectOverCDP", async () => {
    const session = browserSession(); sessions.push(session); return session.browser;
  });
  const old = await pool.ensureContext(contextArgs("old"));
  const busy = await pool.ensureContext(contextArgs("busy"));
  old.lastUsedAt = 1;
  busy.lastUsedAt = 0;
  busy.busy = true;
  await pool.ensureContext(contextArgs("new"));
  assert.deepEqual([...pool.contexts.keys()], ["busy", "new"]);
  assert.deepEqual(sessions[0].closes, ["browser"]);
  assert.deepEqual(sessions[1].closes, []);
});

test("all-busy capacity rejects without launching or retaining a pending registry entry", async (t) => {
  contextFixture(t);
  process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS = "1";
  const session = browserSession();
  const connect = t.mock.method(chromium, "connectOverCDP", async () => session.browser);
  const busy = await pool.ensureContext(contextArgs("busy"));
  busy.busy = true;
  await assert.rejects(pool.ensureContext(contextArgs("blocked")), (error) => error.code === "gemini_canvas_context_busy" && error.status === 429);
  assert.equal(connect.mock.callCount(), 1);
  assert.equal(pool.contexts.get("busy"), busy);
  assert.equal(pool.initializingContexts.size, 0);
  assert.deepEqual(session.closes, []);
});

test("in-flight creation consumes capacity before a browser enters the registry", async (t) => {
  contextFixture(t);
  process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS = "1";
  const session = browserSession(), ready = deferred(), release = deferred();
  const connect = t.mock.method(chromium, "connectOverCDP", async () => {
    ready.resolve(); await release.promise; return session.browser;
  });
  const first = pool.ensureContext(contextArgs("first"));
  try {
    await ready.promise;
    await assert.rejects(pool.ensureContext(contextArgs("second")), (error) => error.status === 429);
    assert.equal(connect.mock.callCount(), 1);
    assert.equal(pool.contexts.size, 0);
    assert.equal(pool.initializingContexts.size, 1);
  } finally {
    release.resolve();
    await first;
  }
  assert.equal(pool.initializingContexts.size, 0);
  assert.equal(pool.contexts.size, 1);
});

test("idle eviction closes only expired non-busy entries", async (t) => {
  contextFixture(t);
  t.mock.method(chromium, "connectOverCDP", async () => browserSession().browser);
  const old = await pool.ensureContext(contextArgs("old"));
  const busy = await pool.ensureContext(contextArgs("busy"));
  const fresh = await pool.ensureContext(contextArgs("fresh"));
  old.lastUsedAt = 1;
  busy.lastUsedAt = 1;
  busy.busy = true;
  fresh.lastUsedAt = Date.now() + 60_000;
  await pool.evictIdleContexts();
  assert.equal(old.contextClosed, true);
  assert.equal(busy.contextClosed, false);
  assert.equal(fresh.contextClosed, false);
  assert.deepEqual([...pool.contexts.keys()], ["busy", "fresh"]);
});
