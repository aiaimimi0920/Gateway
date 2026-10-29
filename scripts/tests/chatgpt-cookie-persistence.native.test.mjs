import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { createNativeBrowserSession } from "../chatgpt-web-session/browser-login-process.mjs";
import { terminateOwned } from "../chatgpt-web-session/browser-login-cleanup.mjs";
import { connectPage } from "../chatgpt-web-session/browser-import-cdp.mjs";
import { importBrowserSession } from "../chatgpt-web-session/browser-import.mjs";
import { loginBrowserSession } from "../chatgpt-web-session/browser-login.mjs";
import { serveChatgptFixture } from "./fixtures/chatgpt-session-cdp.mjs";

const rootExited = (child) => {
  try { process.kill(child.pid, 0); return false; }
  catch (error) { return error.code === "ESRCH"; }
};

// Explicit opt-in: real Chromium, loopback navigation and synthetic HttpOnly cookies only.
test("native Chromium persists cookies, retries full session capture and closes gracefully", {
  skip: process.platform !== "win32" || process.env.GATEWAY_TEST_NATIVE_CHROME !== "1",
  timeout: 230000,
}, async (t) => {
  const executablePath = process.env.CHATGPT_WEB_BROWSER_EXECUTABLE_PATH;
  assert.ok(executablePath && existsSync(executablePath), "explicit browser executable required");
  const parent = await mkdtemp(path.join(tmpdir(), "gateway-cookie-测试 "));
  const observed = [];
  const launches = [];
  const children = [];
  let forceKills = 0;
  let session;
  let fixture;
  const server = createServer((request, response) => {
    observed.push({ phase: request.url, cookie: request.headers.cookie === "gateway_fixture=synthetic" });
    response.setHeader("Content-Type", "text/html; charset=utf-8");
    if (request.url === "/seed") {
      response.setHeader("Set-Cookie", "gateway_fixture=synthetic; Path=/; Max-Age=3600; HttpOnly; SameSite=Lax");
    }
    response.end("<!doctype html><html><body>synthetic fixture</body></html>");
  });
  t.after(async () => {
    try { await session?.close(); }
    finally {
      fixture?.dispose();
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
      if (children.every(rootExited)) await rm(parent, { recursive: true, force: true });
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const previous = ["TEMP", "TMP", "TMPDIR"].map((key) => [key, process.env[key]]);
  try {
    // Reproduce the parent's Windows canonicalize spelling, not just an ordinary temp path.
    for (const [key] of previous) process.env[key] = path.toNamespacedPath(parent);
    session = await createNativeBrowserSession({ executablePath,
      terminateProcess: async (child) => { forceKills++; await terminateOwned(child); },
      spawnProcess: (executable, args, options) => {
      const phase = launches.length === 0 ? "seed" : "check";
      launches.push(args);
      const fixtureArgs = args.map((arg) => arg === "https://chatgpt.com/" ? `${origin}/${phase}` : arg);
      // Headless dump-dom exits normally after /seed; /check keeps the real CDP capture alive.
      const child = spawn(executable, [...fixtureArgs, "--headless=new", "--disable-background-networking",
        ...(phase === "seed" ? ["--dump-dom"] : [])], { ...options, windowsHide: true });
      children.push(child); return child;
    } });
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
  }
  await session.waitForWindowClosed(180000);
  const profile = launches[0].find((arg) => arg.startsWith("--user-data-dir=")).slice(16);
  assert.equal(profile.startsWith("\\\\?\\"), false);
  assert.equal(launches[0].some((arg) => arg.startsWith("--remote-debugging")), false);
  assert.ok(existsSync(path.join(profile, "Default", "Network", "Cookies")), "cookie database persisted on clean exit");
  assert.ok(observed.some((item) => item.phase === "/seed" && !item.cookie));
  const port = await session.startCapture(10000);
  assert.ok(Number.isSafeInteger(port));
  const deadline = Date.now() + 5000;
  while (!observed.some((item) => item.phase === "/check") && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  assert.ok(observed.some((item) => item.phase === "/check" && item.cookie), "reopened capture receives the login cookie");
  assert.ok(launches[1].includes(`--user-data-dir=${profile}`));
  fixture = await serveChatgptFixture(port, origin);
  let retries = 0;
  const result = await loginBrowserSession({ createSession: async () => ({
    ...session, waitForWindowClosed: async () => {}, startCapture: async () => port,
    close: async () => { try { await fixture.park(); }
      catch (error) { t.diagnostic(`synthetic fixture park failed: ${error.message}`); throw error; }
      finally { await session.close(); } },
  }), captureMs: 5000, pollMs: 50, readSession: async (input) => {
    try { return await importBrowserSession(input, { connect: (url) => connectPage(url, { commandTimeoutMs: 150 }) }); }
    catch (error) { if (error.message === "session_read_timeout") retries++; throw error; }
  } });
  assert.equal(result.material.authToken, fixture.token);
  assert.equal(result.material.deviceId, "synthetic-device");
  assert.deepEqual(result.material.modelSlugs, ["synthetic-model"]);
  assert.ok(result.material.cookieHeader.includes("synthetic-session=fixture"));
  assert.equal(result.accountName, "synthetic@example.test");
  assert.ok(retries >= 1, "real CDP timeout occurred and was retried");
  assert.ok(fixture.stats.sessionRequests >= 2);
  assert.equal(fixture.stats.catalogRequests, 1);
  assert.equal(result.cleanupPending, undefined, "real cleanup completed");
  assert.equal(forceKills, 0, "real Browser.close completed without force-kill fallback");
  assert.equal(session.isAlive(), false);
  assert.ok(children.every(rootExited), "OS confirms every owned browser root has exited");
  assert.equal(existsSync(profile), false, "owned browser/profile reclaimed after capture");
});
