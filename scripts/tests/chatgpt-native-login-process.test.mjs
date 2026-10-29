import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { existsSync, writeFileSync } from "node:fs";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { createNativeBrowserSession } from "../chatgpt-web-session/browser-login-process.mjs";

async function fixture({ endpoint = "54321\n/devtools/browser/owned-id\n", metadata,
  spawnError = false, closeBrowser } = {}) {
  const children = [];
  const launches = [];
  const terminated = [];
  let profile;
  const session = await createNativeBrowserSession({ executablePath: "fixture-browser",
    processExists: () => true,
    spawnProcess: (executable, args, options) => {
      profile = args.find((arg) => arg.startsWith("--user-data-dir=")).slice("--user-data-dir=".length);
      const child = Object.assign(new EventEmitter(), { pid: spawnError ? undefined : 1000 + children.length,
        exitCode: null, signalCode: null, quit(code = 0) { this.exitCode = code; this.emit("exit", code); } });
      children.push(child); launches.push({ executable, args, options });
      if (children.length % 2 === 0) writeFileSync(path.join(profile, "DevToolsActivePort"), endpoint, "utf8");
      if (spawnError) queueMicrotask(() => child.emit("error", new Error("not-found")));
      return child;
    },
    readMetadata: async (port) => metadata || { webSocketDebuggerUrl: `ws://127.0.0.1:${port}/devtools/browser/owned-id` },
    terminateProcess: async (child) => { terminated.push(child.pid); child.quit(); },
    closeBrowser: closeBrowser || (async () => { throw new Error("fixture-no-debugger-close"); }),
  });
  return { session, children, launches, terminated, get profile() { return profile; } };
}

test("human login has no debugger or automation flags and no daily profile; capture follows window close", async (t) => {
  const f = await fixture(); t.after(() => f.session.close());
  assert.equal(f.launches[0].executable, "fixture-browser");
  assert.equal(f.launches[0].options.detached, false);
  assert.deepEqual(f.launches[0].args, [`--user-data-dir=${f.profile}`, "--no-first-run",
    "--no-default-browser-check", "--new-window", "https://chatgpt.com/"]);
  assert.match(path.basename(f.profile), /^gateway-chatgpt-login-/);
  assert.equal(f.launches[0].args.some((arg) => /automation|debugging|disable-security|no-sandbox/i.test(arg)), false);
  await assert.rejects(f.session.startCapture(1000), /invalid_input/);
  assert.equal(f.launches.length, 1);
  f.children[0].quit();
  await f.session.waitForWindowClosed(1000);
  assert.equal(await f.session.startCapture(1000), 54321);
  assert.ok(f.launches[1].args.includes("--remote-debugging-address=127.0.0.1"));
  assert.ok(f.launches[1].args.includes("--remote-debugging-port=0"));
  assert.ok(f.launches[1].args.includes(`--user-data-dir=${f.profile}`));
  assert.equal(f.session.isAlive(), true);
  await f.session.close();
  assert.deepEqual(f.terminated, [1001]);
  assert.equal(existsSync(f.profile), false);
});

test("one explicit verification reuses the owned profile without debugging and only then recaptures", async () => {
  const f = await fixture();
  try {
    f.children[0].quit(); await f.session.waitForWindowClosed(1000);
    await f.session.startCapture(1000);
    await f.session.prepareVerification();
    assert.equal(f.session.isAlive(), false);
    assert.equal(existsSync(f.profile), true);
    assert.equal(f.launches.length, 2, "suspension never opens another browser implicitly");
    const verification = f.session.openVerification(1000);
    assert.ok(f.launches[2].args.includes("https://chatgpt.com/api/auth/session"));
    assert.equal(f.launches[2].args.some((arg) => /debugging|automation/i.test(arg)), false);
    f.children[2].quit(); await verification;
    await f.session.startCapture(1000);
    assert.equal(f.launches.length, 4);
    assert.ok(f.launches.every((launch) => launch.args.includes(`--user-data-dir=${f.profile}`)));
    await assert.rejects(f.session.prepareVerification(), /invalid_input/);
  } finally { await f.session.close(); }
  assert.equal(existsSync(f.profile), false);
  assert.deepEqual(f.terminated, [1001, 1003]);
});

test("final cleanup waits for an in-progress retained-profile shutdown and prevents reopening", async () => {
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  const f = await fixture({ closeBrowser: async () => { await gate; f.children[1].quit(); } });
  f.children[0].quit(); await f.session.waitForWindowClosed(1000);
  await f.session.startCapture(1000);
  const transition = f.session.prepareVerification();
  const cleanup = f.session.close();
  release(); await transition; await cleanup;
  assert.equal(existsSync(f.profile), false);
  await assert.rejects(f.session.openVerification(1000), /invalid_input/);
  assert.equal(f.launches.length, 2);
});

test("human login is bounded and cleanup only terminates the owned child", async () => {
  const f = await fixture();
  try {
    await assert.rejects(f.session.waitForWindowClosed(1), /browser_timeout/);
    assert.equal(f.launches.length, 1);
  } finally { await f.session.close(); }
  assert.deepEqual(f.terminated, [1000]);
  assert.equal(existsSync(f.profile), false);
});

test("spawn failure never issues a termination command with an absent PID", async () => {
  const f = await fixture({ spawnError: true });
  try { await assert.rejects(f.session.waitForWindowClosed(1000), /browser_closed/); }
  finally { await f.session.close(); }
  assert.deepEqual(f.terminated, []);
  assert.equal(existsSync(f.profile), false);
});

test("Windows canonical temp paths never become Chromium verbatim profile arguments", {
  skip: process.platform !== "win32",
}, async (t) => {
  const parent = await mkdtemp(path.join(tmpdir(), "gateway-login-path-测试 "));
  const previous = ["TEMP", "TMP", "TMPDIR"].map((key) => [key, process.env[key]]);
  let f;
  try {
    for (const [key] of previous) process.env[key] = path.toNamespacedPath(parent);
    f = await fixture();
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
    t.after(async () => { await f?.session.close(); await rm(parent, { recursive: true, force: true }); });
  }
  assert.equal(f.profile.startsWith("\\\\?\\"), false);
  assert.equal(await realpath(path.dirname(f.profile)), await realpath(parent));
  f.children[0].quit(); await f.session.waitForWindowClosed(1000);
  assert.equal(await f.session.startCapture(1000), 54321);
  assert.ok(f.launches[1].args.includes(`--user-data-dir=${f.profile}`));
  await f.session.close();
  assert.equal(existsSync(f.profile), false);
});

for (const [name, endpoint, metadata] of [
  ["reserved port", "4300\n/devtools/browser/owned-id\n"],
  ["oversized endpoint", "9".repeat(256)],
  ["non-browser endpoint", "54321\n/devtools/page/owned-id\n"],
  ["foreign browser", undefined, { webSocketDebuggerUrl: "ws://127.0.0.1:54321/devtools/browser/other" }],
  ["external host", undefined, { webSocketDebuggerUrl: "ws://example.test:54321/devtools/browser/owned-id" }],
]) {
  test(`capture rejects ${name} without attaching to it`, async () => {
    const f = await fixture({ endpoint, metadata });
    try {
      f.children[0].quit(); await f.session.waitForWindowClosed(1000);
      await assert.rejects(f.session.startCapture(1000), /invalid_debugger/);
    } finally { await f.session.close(); }
    assert.deepEqual(f.terminated, [1001]);
  });
}
