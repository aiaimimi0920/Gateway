// Exercise the real Node service + worker subprocess; only Playwright is an explicit synthetic seam.
import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import net from "node:net";
import { fileURLToPath } from "node:url";
test("Suno versioned executor authenticates, isolates the provider and preserves its binding receipt", { timeout: 15000 }, async () => {
  const reservation = net.createServer(); reservation.listen(0, "127.0.0.1"); await once(reservation, "listening");
  const port = reservation.address().port; await new Promise(resolve => reservation.close(resolve));
  const preload = new URL("./fixtures/suno-worker-lifecycle-preload.mjs", import.meta.url).href;
  const script = fileURLToPath(new URL("../local-browser-executor-service.mjs", import.meta.url));
  const child = spawn(process.execPath, [script], { cwd: fileURLToPath(new URL("../", import.meta.url)),
    env: { ...process.env, LOCAL_BROWSER_EXECUTOR_PORT: String(port),
      GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN: "synthetic-executor-token", NODE_OPTIONS: `--import ${preload}`,
      SUNO_LIFECYCLE_FIXTURE: "success", SUNO_BROWSER_CDP_URL: "", SUNO_EASYBROWSER_BASE_URL: "" },
    stdio: ["ignore", "pipe", "pipe"] });
  const exited = once(child, "exit"); let stdout = "", stderr = "";
  child.stdout.on("data", value => { stdout += value; assert.ok(stdout.length < 65536); });
  child.stderr.on("data", value => { stderr += value; assert.ok(stderr.length < 65536); });
  try {
    const deadline = Date.now() + 5000;
    while (!stdout.includes('"mode":"local_node_browser_executor_service"')) {
      assert.equal(child.exitCode, null, stderr); assert.ok(Date.now() < deadline, stderr);
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    const url = `http://127.0.0.1:${port}/v1/internal/browser-executor/suno-bound-v1/execute`;
    const send = (body, token = "synthetic-executor-token") => fetch(url, { method: "POST", signal: AbortSignal.timeout(5000),
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" }, body: JSON.stringify(body) });
    const input = { baseUrl: "https://suno.fixture.invalid", referer: "https://suno.fixture.invalid/create",
      requestBody: { model: "chirp-auk-turbo", prompt: "synthetic fixture" }, waitCompletion: false,
      browserExecutablePath: process.execPath };
    assert.equal((await send({ provider: "suno", input }, "wrong")).status, 401);
    const foreign = await send({ provider: "udio", input });
    assert.equal(foreign.status, 400); assert.equal((await foreign.json()).error.code, "suno_browser_binding_provider");
    const response = await send({ provider: "suno", providerAccountId: "exact", endpointKind: "music_generations", input });
    const result = await response.json(); assert.equal(response.status, 200);
    assert.equal(result.ok, true, JSON.stringify(result)); assert.equal(result.result.wireModel, "chirp-auk-turbo");
    assert.equal(result.result.completed, true);
    const old = await send({ provider: "suno", input: { ...input, requestBody: { prompt: "synthetic fixture" } } });
    const oldResult = await old.json(); assert.equal(oldResult.ok, false);
    assert.equal(oldResult.error.code, "suno_browser_request_mismatch");
    const advanced = await send({ provider: "suno", input: { ...input,
      requestBody: { ...input.requestBody, continue_clip_id: "other-song" } } });
    assert.equal((await advanced.json()).error.code, "suno_browser_request_mismatch");
    assert.ok(!JSON.stringify([result, oldResult]).includes("synthetic-executor-token"));
  } finally { child.kill("SIGTERM"); await exited; }
});
