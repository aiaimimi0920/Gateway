// Real worker + native CDP + browser stream reader; loopback fixture, not Udio playback/download.
import test from "node:test";
import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdtemp, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
const executablePath = process.env.GATEWAY_UDIO_CDP_TEST_BROWSER;
const evidenceRoot = process.env.GATEWAY_UDIO_CDP_TEST_ROOT;
const song = (id, finished = true) => ({ id, finished, ...(finished && { song_path: "https://asset.invalid/not-audio" }) });
test("Udio isolated native worker metadata integration", { skip: !executablePath || !evidenceRoot, timeout: 120000 }, async t => {
  let mode, started, submissions = 0, polls = 0, queried = [], cleanupCalls = 0, priorResponse, priorSeen;
  const server = createServer((req, res) => {
    req.resume(); res.setHeader("content-type", "application/json");
    if (req.url.startsWith("/api/generate-proxy")) {
      if (req.url.includes("prior=1")) { priorResponse = res; priorSeen(); return; }
      if (req.url.includes("after=1")) { cleanupCalls++; res.end("{}"); return; }
      if (req.url.includes("other=1")) { res.end(JSON.stringify({ track_ids: ["FOREIGN"], songs: [song("FOREIGN")] })); return; }
      submissions++; started?.();
      if (priorResponse) { priorResponse.end(JSON.stringify({ track_ids: ["FOREIGN"], songs: [song("FOREIGN")] })); priorResponse = null; }
      if (mode === "gzip") {
        res.setHeader("content-encoding", "gzip");
        res.end(gzipSync(JSON.stringify({ track_ids: ["A"], songs: [song("A")], padding: "x".repeat(256 * 1024) })));
      } else if (mode === "html" || mode === "challenge") {
        res.setHeader("content-type", "text/html");
        if (mode === "challenge") res.statusCode = 403;
        res.end(mode === "challenge" ? "Vercel Security Checkpoint" : "private html document");
      } else if (mode === "http-error") { res.statusCode = 503; res.end("private upstream document"); }
      else if (mode === "bad-json") res.end("private malformed {");
      else if (mode === "duplicate") res.end(JSON.stringify({ track_ids: ["A", "A"], songs: [] }));
      else if (mode === "chunked") {
        const text = JSON.stringify({ track_ids: ["A"], songs: [song("A")] });
        res.write(text.slice(0, 5)); setTimeout(() => res.end(text.slice(5)), 10);
      } else setTimeout(() => res.end(JSON.stringify({ track_ids: ["A", "B"], songs: [] })), 150);
    } else if (req.url.startsWith("/api/songs?")) {
      queried.push(new URL(req.url, "http://fixture.invalid").searchParams.get("songIds")); polls++;
      const songs = mode === "failed" ? [song("A"), { ...song("B"), status: "failed" }]
        : polls === 1 ? [song("A")] : [song("B"), song("A")];
      res.end(JSON.stringify({ songs }));
    } else {
      res.setHeader("content-type", "text/html");
      const params = { model_type: mode === "wrong-model" ? "foreign-private-model" : "udio-fixture-native" };
      if (mode === "missing-model") delete params.model_type;
      if (mode === "conflicting-model") params.modelType = "foreign-private-model";
      const script = `fetch('/api/generate-proxy',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({gen_params:${JSON.stringify(params)}})}).catch(()=>{})`;
      res.end(`<title>Create</title><textarea></textarea><button id="create">Create</button><script>document.querySelector('#create').onclick=()=>{${script}};</script>`);
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  let owner;
  try {
    const profile = await mkdtemp(join(evidenceRoot, "udio-native-profile-"));
    owner = await chromium.launchPersistentContext(profile, { executablePath, headless: true,
      args: ["--remote-debugging-port=0", "--no-first-run"] });
    const port = (await readFile(join(profile, "DevToolsActivePort"), "utf8")).split(/\r?\n/)[0];
    const page = owner.pages()[0], other = await owner.newPage();
    await other.goto(`${origin}/other`);
    await other.evaluate(() => { window.fixtureTicks = 0; setInterval(() => window.fixtureTicks++, 10); });
    for (const [current, code] of [["partial", null], ["chunked", null], ["gzip", "udio_browser_metadata_limit"],
      ["html", "udio_browser_content_type"], ["http-error", "udio_browser_request_failed"],
      ["bad-json", "udio_invalid_generation_json"], ["duplicate", "udio_browser_song_identity"],
      ["challenge", "udio_browser_challenge_required"], ["failed", "udio_browser_generation_failed"],
      ["wrong-model", "udio_browser_model_mismatch"], ["missing-model", "udio_browser_model_mismatch"],
      ["conflicting-model", "udio_browser_model_mismatch"], ["prior-inflight", "udio_browser_capture_failed"]]) {
      await t.test(current, async () => {
        mode = current; polls = 0; queried = [];
        const before = submissions, tick = await other.evaluate(() => window.fixtureTicks);
        await page.goto(`${origin}/create`);
        if (current === "prior-inflight") {
          const prior = new Promise(resolve => { priorSeen = resolve; });
          await page.evaluate(() => { void fetch('/api/generate-proxy?prior=1', { method: 'POST', body: '{}' }).catch(() => {}); });
          await prior;
        }
        const began = new Promise(resolve => { started = resolve; });
        const worker = fileURLToPath(new URL("../udio-browser-worker.mjs", import.meta.url));
        const child = spawn(process.execPath, [worker], { stdio: ["pipe", "pipe", "pipe"],
          signal: AbortSignal.timeout(20000), killSignal: "SIGKILL",
          env: { ...process.env, UDIO_BROWSER_CDP_URL: "", UDIO_BROWSER_DEBUG_LOG_PATH: "" } });
        let stdout = "", stderr = "";
        child.stdout.on("data", b => { stdout += b; }); child.stderr.on("data", b => { stderr += b; });
        const closed = new Promise((resolve, reject) => { child.once("error", reject); child.once("close", resolve); });
        child.stdin.end(JSON.stringify({ baseUrl: origin, origin, referer: `${origin}/create`,
          browserCdpUrl: `http://127.0.0.1:${port}`, browserCdpTargetUrl: `${origin}/create`,
          requestBody: { gen_params: { prompt: "loopback fixture", model_type: "udio-fixture-native" } },
          waitAudio: ["partial", "failed"].includes(current), waitTimeoutMs: 15000, pollIntervalMs: 1000 }));
        if (current === "partial") {
          await Promise.race([began, closed.then(() => { throw new Error("worker exited before submission"); })]);
          await other.evaluate(() => fetch('/api/generate-proxy?other=1', { method: 'POST' }));
        }
        assert.equal(await closed, 0, stderr);
        const value = JSON.parse(stdout);
        if (code) { assert.equal(value.ok, false, stdout); assert.equal(value.error.code, code, stdout); }
        else {
          assert.equal(value.ok, true, stdout); assert.equal(value.result.completed, true);
          assert.deepEqual(value.result.songs.map(s => s.id), current === "partial" ? ["A", "B"] : ["A"]);
        }
        assert.ok(!stdout.includes("private") && !stdout.includes("Vercel Security Checkpoint"));
        assert.equal(submissions, before + (code === "udio_browser_model_mismatch" ? 0 : 1)); assert.ok(queried.every(ids => ids === "A,B"));
        if (current === "partial") assert.equal(polls, 2);
        assert.equal(owner.pages().length, 2); assert.equal(page.isClosed(), false);
        assert.equal(other.url(), `${origin}/other`); assert.ok(await other.evaluate(() => window.fixtureTicks) > tick);
        const previousCleanup = cleanupCalls;
        await page.evaluate(() => fetch('/api/generate-proxy?after=1', { method: 'POST', body: '{}' }));
        assert.equal(cleanupCalls, previousCleanup + 1, "borrowed page has no leftover worker route");
      });
    }
  } finally {
    await owner?.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
  }
});
