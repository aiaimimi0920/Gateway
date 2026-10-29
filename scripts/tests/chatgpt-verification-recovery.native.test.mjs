import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { createNativeBrowserSession } from "../chatgpt-web-session/browser-login-process.mjs";
import { createLoginInteraction } from "../chatgpt-web-session/browser-login-interaction.mjs";
import { loginBrowserSession } from "../chatgpt-web-session/browser-login.mjs";
import { serveChatgptFixture } from "./fixtures/chatgpt-session-cdp.mjs";

const rootExited = (child) => {
  try { process.kill(child.pid, 0); return false; }
  catch (error) { return error.code === "ESRCH"; }
};

// Explicit opt-in; synthetic cookies/responses, no real ChatGPT requests or human login.
test("native challenge recovery waits for consent, reuses one profile, and reclaims every owned window", {
  skip: process.platform !== "win32" || process.env.GATEWAY_TEST_NATIVE_CHROME !== "1", timeout: 230000,
}, async (t) => {
  const started = Date.now();
  const record = (phase) => t.diagnostic(`${phase}: ${Date.now() - started} ms`);
  const executablePath = process.env.CHATGPT_WEB_BROWSER_EXECUTABLE_PATH;
  assert.ok(executablePath && existsSync(executablePath));
  const root = await mkdtemp(path.join(tmpdir(), "gateway-verification-测试 "));
  const launches = [];
  const children = [];
  const fixtures = [];
  const seen = [];
  let session;
  const server = createServer((request, response) => {
    seen.push(request.url);
    response.setHeader("Content-Type", "text/html; charset=utf-8");
    if (request.url === "/seed") response.setHeader("Set-Cookie", "fixture=synthetic; Path=/; HttpOnly");
    response.end("<!doctype html><html><body>synthetic verification fixture</body></html>");
  });
  t.after(async () => {
    try { await session?.close(); }
    finally {
      for (const fixture of fixtures) fixture.dispose();
      server.closeAllConnections(); await new Promise((resolve) => server.close(resolve));
      if (children.every(rootExited)) await rm(root, { recursive: true, force: true });
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const previous = ["TEMP", "TMP", "TMPDIR"].map((key) => [key, process.env[key]]);
  try {
    for (const [key] of previous) process.env[key] = root;
    session = await createNativeBrowserSession({ executablePath, spawnProcess: (exe, args, options) => {
      const phase = ["seed", "capture1", "verify", "capture2"][launches.length];
      assert.ok(phase, "at most one explicit verification round");
      record(`launch_${phase}`);
      launches.push(args);
      const rewritten = args.map((arg) => arg.startsWith("https://chatgpt.com/") ? `${origin}/${phase}` : arg);
      const child = spawn(exe, [...rewritten, "--headless=new", "--disable-background-networking",
        ...(["seed", "verify"].includes(phase) ? ["--dump-dom"] : [])], { ...options, windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
      let stderr = "";
      child.stderr.on("data", (chunk) => { stderr = (stderr + chunk.toString()).slice(-4096); });
      child.once("exit", (code, signal) => {
        record(`${phase}_exit_code_${code}_signal_${signal}`);
        if (code !== 0) t.diagnostic(`${phase} synthetic browser stderr: ${stderr}`);
      });
      children.push(child); return child;
    } });
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
  }
  const channel = await createLoginInteraction(root);
  let ready;
  const verificationReady = new Promise((resolve) => { ready = resolve; });
  const interaction = { ...channel, setPhase: async (phase) => {
    await channel.setPhase(phase);
    record(phase);
    if (phase === "verification_required") ready();
  } };
  const parkFixture = async () => {
    record("fixture_park_start");
    try { await fixtures.at(-1)?.park(); }
    catch (error) { record(`fixture_park_error_${error.message}`); throw error; }
    record("fixture_park_done");
  };
  const pending = loginBrowserSession({ createSession: async () => ({ ...session,
    prepareVerification: async () => { await parkFixture(); await session.prepareVerification(); },
    close: async () => { try { await parkFixture(); } finally {
      record("session_close_start"); await session.close(); record("session_close_done");
    } },
    startCapture: async (timeout) => {
    const port = await session.startCapture(timeout);
    record(`capture_endpoint_${fixtures.length + 1}`);
    // Attach only to the owned loopback page before any first-party navigation.
    const deadline = Date.now() + 5000;
    const expected = fixtures.length === 0 ? "/capture1" : "/capture2";
    while (!seen.includes(expected) && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 25));
    fixtures.push(await serveChatgptFixture(port, origin, { challenge: fixtures.length === 0, delayFirst: false }));
    record(`capture_fixture_${fixtures.length}`);
    return port;
  } }), interaction, pollMs: 25 });
  // Observe the state without consuming the login result, bounded even if setup fails.
  let readinessTimer;
  try {
    await Promise.race([pending.then(() => assert.fail("read must await explicit consent")), verificationReady,
      new Promise((_, reject) => { readinessTimer = setTimeout(() => reject(new Error("fixture_verification_not_ready")), 120000); }),
    ]);
  } finally { clearTimeout(readinessTimer); }
  assert.equal(launches.length, 2);
  const profile = launches[0].find((arg) => arg.startsWith("--user-data-dir=")).slice(16);
  assert.equal(existsSync(profile), true);
  assert.equal(session.isAlive(), false, "capture is closed while awaiting user consent");
  assert.equal(fixtures[0].stats.sessionRequests, 1);
  assert.equal(fixtures[0].stats.catalogRequests, 0);
  await writeFile(path.join(root, "login-verify"), "", { flag: "wx" });
  const result = await pending;
  record("completed");
  assert.equal(result.accountName, "synthetic@example.test");
  assert.equal(result.cleanupPending, undefined);
  assert.equal(launches.length, 4);
  assert.ok(launches.every((args) => args.includes(`--user-data-dir=${profile}`)));
  assert.equal(launches[2].some((arg) => /debugging|automation/i.test(arg)), false);
  assert.equal(seen.includes("/verify"), true);
  assert.equal(existsSync(profile), false);
  assert.ok(children.every(rootExited), "OS confirms every owned browser root has exited");
});
