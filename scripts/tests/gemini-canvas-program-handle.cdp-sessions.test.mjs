import assert from "node:assert/strict";
import test from "node:test";
import { cdpFixture, until } from "./gemini-canvas-program-handle.cdp-fixtures.mjs";
import { deferred, turn } from "./gemini-canvas-program-handle.response-fixtures.mjs";

test("new pages stay paused until capture configuration and readiness complete", async (t) => {
  const pending = deferred();
  const f = await cdpFixture(t, { configure: (session) => session.sessionId === "direct-popup" ? pending.promise : undefined });
  f.root.attach([], "auto-popup", "page", "popup");
  await until(() => f.configured.length === 2);
  let ready = false;
  const waiting = f.owner.waitForTargets().then(() => { ready = true; });
  await turn();
  assert.equal(ready, false);
  assert.ok(!f.root.commands.some((c) => c.method === "Runtime.runIfWaitingForDebugger" && c.route[0] === "direct-popup"));
  pending.resolve();
  await waiting;
  assert.ok(f.root.commands.some((c) => c.method === "Target.detachFromTarget" && c.params.sessionId === "auto-popup"));
  assert.equal(f.identity.detached, 1);
  assert.deepEqual(f.failures, []);
});

test("foreign contexts detach their automatic session without reading or configuring them", async (t) => {
  const f = await cdpFixture(t);
  f.root.attach([], "foreign", "page", "foreign", "other-context");
  await f.owner.waitForTargets();
  assert.equal(f.configured.length, 1);
  assert.ok(!f.root.commands.some((c) => c.method === "Target.attachToTarget" && c.params.targetId === "foreign"));
  assert.ok(f.root.commands.some((c) => c.method === "Target.detachFromTarget" && c.params.sessionId === "foreign"));
  assert.deepEqual(f.failures, []);
});

test("nested worker routing retains ancestry and removes only owned listeners", async (t) => {
  const f = await cdpFixture(t), unrelated = () => undefined;
  f.root.on("Target.attachedToTarget", unrelated);
  f.root.attach(["direct-initial"], "worker");
  await f.owner.waitForTargets();
  f.root.attach(["direct-initial", "worker"], "nested");
  await f.owner.waitForTargets();
  const child = f.configured[2];
  assert.equal(child.parent, f.configured[1]);
  assert.equal(child.nestingDepth, 2);
  assert.deepEqual(await child.send("Fixture.echo"), { route: ["direct-initial", "worker", "nested"] });
  const stopped = f.owner.stop();
  assert.equal(stopped, f.owner.stop());
  await stopped;
  assert.equal(f.root.detached, 1);
  assert.equal(f.root.listenerCount("Target.receivedMessageFromTarget"), 0);
  assert.deepEqual(f.root.listeners("Target.attachedToTarget"), [unrelated]);
  assert.deepEqual(new Set(f.disposed), new Set(f.configured));
  assert.deepEqual(f.failures, []);
});

test("stop aborts configuration immediately and retains its late disposer", async (t) => {
  const pending = deferred();
  const f = await cdpFixture(t, { deferReady: true, configure: () => pending.promise });
  await until(() => f.configured.length === 1);
  await f.owner.stop();
  assert.equal(f.configured[0].listenerCount("Fixture.network"), 0);
  pending.resolve();
  await turn();
  assert.equal(f.disposed.length, 1);
  assert.deepEqual(f.failures, []);
});

test("startup timeout still detaches a browser session that arrives after stop", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const pending = deferred();
  const f = await cdpFixture(t, { deferReady: true, timeoutMs: 100, newBrowserCDPSession: () => pending.promise });
  await until(f.browserRequested);
  t.mock.timers.tick(100);
  await assert.rejects(f.owner.ready, { code: "gateway_program_handle_capture_failed" });
  await f.owner.stop();
  pending.resolve(f.root);
  await turn();
  assert.equal(f.root.detached, 1);
  assert.equal(f.identity.detached, 1);
  assert.equal(f.failures.length, 1);
});

test("page bursts cannot exceed the context session budget or leave paused targets owned", async (t) => {
  const pending = deferred();
  const f = await cdpFixture(t, { configure: (session) => session.sessionId === "direct-initial" ? undefined : pending.promise });
  for (let index = 0; index < 40; index++) f.root.attach([], `auto-${index}`, "page", `page-${index}`);
  await f.owner.stop();
  assert.equal(f.failures.length, 1);
  assert.ok(f.configured.length <= 15);
  assert.equal(f.root.detached, 1);
  assert.equal(f.root.listenerCount("Target.receivedMessageFromTarget"), 0);
  pending.resolve();
});

test("child command saturation cancels pending commands and fails the context once", async (t) => {
  const pending = deferred();
  const f = await cdpFixture(t, { respond: ({ method }) => method === "Fixture.stall" ? pending.promise : {} });
  const session = f.configured[0];
  const commands = Array.from({ length: 17 }, () => assert.rejects(session.send("Fixture.stall")));
  await Promise.all(commands);
  await f.owner.stop();
  assert.equal(session.pending.size, 0);
  assert.equal(f.failures.length, 1);
  assert.equal(f.root.detached, 1);
  pending.resolve({});
});
