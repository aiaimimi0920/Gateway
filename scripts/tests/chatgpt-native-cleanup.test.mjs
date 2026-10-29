import assert from "node:assert/strict";
import test from "node:test";
import { closeNativeSession } from "../chatgpt-web-session/browser-login-cleanup.mjs";

function fixture() {
  let finish;
  const child = { pid: 1000, exitCode: null, signalCode: null };
  const done = new Promise((resolve) => { finish = resolve; });
  const quit = () => { child.exitCode = 0; finish({ code: 0 }); };
  const removed = [];
  return { child, quit, removed, options: { children: [{ child, done }], profile: "owned-fixture",
    browserEndpoint: "ws://127.0.0.1:54321/devtools/browser/owned", processExists: () => true,
    removeProfile: async (...args) => { removed.push(args); } } };
}

test("verified capture debugger closes gracefully without taskkill", async () => {
  const f = fixture();
  await closeNativeSession({ ...f.options, closeBrowser: async (endpoint) => {
    assert.equal(endpoint, f.options.browserEndpoint); f.quit();
  }, terminateProcess: async () => assert.fail("graceful close must not force kill") });
  assert.equal(f.removed.length, 1);
});

test("OS-confirmed exit is not held hostage by a delayed Node exit callback", async () => {
  const f = fixture();
  let running = true;
  try {
    await closeNativeSession({ ...f.options, browserEndpoint: undefined,
      processExists: () => running,
      terminateProcess: async () => { running = false; },
    });
    assert.equal(f.child.exitCode, null, "the delayed callback has not arrived");
    assert.equal(f.removed.length, 1, "the OS confirms the owned root no longer exists");
  } finally { f.quit(); }
});

test("a failed termination helper cannot veto OS-confirmed browser exit", async () => {
  const f = fixture();
  let running = true;
  try {
    await closeNativeSession({ ...f.options, browserEndpoint: undefined,
      processExists: () => running,
      terminateProcess: async () => { running = false; throw new Error("helper_callback_timeout"); },
    });
    assert.equal(f.child.exitCode, null, "browser callback is still delayed");
    assert.equal(f.removed.length, 1, "the helper result does not override OS absence");
  } finally { f.quit(); }
});

test("an unknown OS probe result cannot authorize live-profile removal", async () => {
  const f = fixture();
  try {
    await assert.rejects(closeNativeSession({ ...f.options, browserEndpoint: undefined,
      processExists: () => { throw new Error("private OS probe failure"); }, terminateProcess: async () => {},
    }), /browser_cleanup_timeout/);
    assert.equal(f.removed.length, 0);
  } finally { f.quit(); }
});

test("failed graceful close falls back to only the owned process", async () => {
  const f = fixture();
  const killed = [];
  await closeNativeSession({ ...f.options, closeBrowser: async () => { throw new Error("fixture"); },
    terminateProcess: async (child) => { killed.push(child.pid); f.quit(); } });
  assert.deepEqual(killed, [1000]);
  assert.equal(f.removed.length, 1);
});

test("a failed process termination never removes a still-live profile", async () => {
  const f = fixture();
  await assert.rejects(closeNativeSession({ ...f.options, browserEndpoint: undefined,
    terminateProcess: async () => { throw new Error("private helper failure"); } }),
  (error) => error.message === "browser_cleanup_timeout");
  assert.equal(f.removed.length, 0);
  f.quit();
});

test("profile removal failure is fixed metadata, never the OS error or profile path", async () => {
  const f = fixture(); f.quit();
  await assert.rejects(closeNativeSession({ ...f.options,
    removeProfile: async () => { throw new Error("private filesystem detail"); } }),
  (error) => error.message === "browser_cleanup_failed");
});
