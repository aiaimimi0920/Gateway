// Real CDP admission and worker entry; no real platform, user profile or credentials.
import test from "node:test";
import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdtemp, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
const executablePath = process.env.GATEWAY_SUNO_CDP_TEST_BROWSER;
const evidenceRoot = process.env.GATEWAY_SUNO_CDP_TEST_ROOT;
const clip = (id, status = "complete") => ({ id, status, audio_url: "https://asset.invalid/not-audio" });
test("Suno native worker metadata integration", {
  skip: !executablePath || !evidenceRoot, timeout: 90000,
}, async (t) => {
  let mode, endpoint, started, feeds = 0, generations = 0, releases = 0;
  const server = createServer((req, res) => {
    req.resume();
    res.setHeader("content-type", "application/json");
    if (req.url === "/v1/browser/sessions/acquire") {
      res.end(JSON.stringify({ data: { session: { session_id: "synthetic", attach: { endpoint, page_url: `${origin}/create` } } } }));
    } else if (req.url === "/v1/browser/sessions/synthetic/release") {
      releases++; res.end("{}");
    } else if (req.url.startsWith("/api/generate/v2-web/")) {
      if (req.url.includes("other=1")) { res.end(JSON.stringify({ clips: [clip("FOREIGN")] })); return; }
      generations++; started?.();
      if (mode === "gzip") {
        res.setHeader("content-encoding", "gzip");
        res.end(gzipSync(JSON.stringify({ clips: [clip("A")], padding: "x".repeat(256 * 1024) })));
      } else if (mode === "html") {
        res.setHeader("content-type", "text/html"); res.end("private html document");
      } else if (mode === "http-error") {
        res.statusCode = 503; res.end("private upstream document");
      } else if (mode === "bad-json") res.end("private malformed {");
      else if (mode === "duplicate") res.end(JSON.stringify({ clips: [clip("A"), clip("A")] }));
      else if (mode === "chunked") {
        const body = JSON.stringify({ clips: [clip("A")] });
        res.write(body.slice(0, 10)); setTimeout(() => res.end(body.slice(10)), 10);
      } else res.end(JSON.stringify({ clips: [clip("A", "submitted"), clip("B", "submitted")] }));
    } else if (req.url === "/api/feed/v3") {
      feeds++;
      res.end(JSON.stringify({ clips: feeds === 1 ? [clip("A")] : [clip("B"), clip("foreign", "failed"), clip("A")] }));
    } else {
      res.setHeader("content-type", "text/html");
      res.end(`<title>Create</title><textarea maxlength="3000"></textarea>
        <button aria-label="Create song" onclick="fetch('/api/generate/v2-web/',{method:'POST',body:JSON.stringify({mv:'page-default',generation_type:'TEXT',prompt:document.querySelector('textarea').value,make_instrumental:false,metadata:{create_mode:'simple'}})}).then(()=>{
          setTimeout(()=>fetch('/api/feed/v3',{method:'POST'}),50);
          setTimeout(()=>fetch('/api/feed/v3',{method:'POST'}),400);
        })">Create song</button>`);
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  let owner;
  try {
    const profile = await mkdtemp(join(evidenceRoot, "suno-admission-profile-"));
    owner = await chromium.launchPersistentContext(profile, {
      executablePath, headless: true, args: ["--remote-debugging-port=0", "--no-first-run"],
    });
    const port = (await readFile(join(profile, "DevToolsActivePort"), "utf8")).split(/\r?\n/)[0];
    endpoint = `http://127.0.0.1:${port}`;
    const page = owner.pages()[0];
    const other = await owner.newPage();
    await other.goto(`${origin}/other`);
    await other.evaluate(() => { window.fixtureTicks = 0; setInterval(() => window.fixtureTicks++, 10); });
    for (const [current, code] of [["partial", null], ["chunked", null], ["gzip", "suno_browser_body_limit"],
      ["html", "suno_browser_content_type"], ["http-error", "suno_browser_generation_failed"],
      ["bad-json", "suno_browser_invalid_generation_json"], ["duplicate", "suno_browser_clip_identity"]]) {
      await t.test(current, async () => {
        mode = current; feeds = 0;
        const beforeGenerations = generations, beforeReleases = releases;
        const tick = await other.evaluate(() => window.fixtureTicks);
        await page.goto(`${origin}/create`);
        const begin = new Promise((resolve) => { started = resolve; });
        const worker = fileURLToPath(new URL("../suno-browser-worker.mjs", import.meta.url));
        const child = spawn(process.execPath, [worker], {
          stdio: ["pipe", "pipe", "pipe"], signal: AbortSignal.timeout(15000), killSignal: "SIGKILL",
          env: { ...process.env, SUNO_BROWSER_CDP_URL: "", SUNO_EASYBROWSER_BASE_URL: "" },
        });
        let stdout = "", stderr = "";
        child.stdout.on("data", (value) => { stdout += value; });
        child.stderr.on("data", (value) => { stderr += value; });
        const closed = new Promise((resolve, reject) => { child.once("error", reject); child.once("close", resolve); });
        child.stdin.end(JSON.stringify({ baseUrl: origin, origin, referer: `${origin}/create`,
          easyBrowserBaseUrl: origin, easyBrowserBearerToken: "synthetic-fixture", requestBody: { model: "chirp-auk-turbo", prompt: "local fixture" },
          waitCompletion: current === "partial", waitTimeoutMs: 10000 }));
        if (current === "partial") {
          await Promise.race([begin, closed.then(() => { throw new Error("worker exited before generation"); })]);
          await other.evaluate(() => fetch('/api/generate/v2-web/?other=1', { method: 'POST' }));
        }
        assert.equal(await closed, 0, stderr);
        const value = JSON.parse(stdout);
        if (code) { assert.equal(value.ok, false); assert.equal(value.error.code, code, stdout); }
        else {
          assert.equal(value.ok, true, stdout); assert.equal(value.result.completed, true);
          assert.deepEqual(value.result.clips.map((c) => c.id), current === "partial" ? ["A", "B"] : ["A"]);
        }
        assert.ok(!stdout.includes("private"));
        assert.equal(generations, beforeGenerations + 1);
        assert.equal(releases, beforeReleases + 1);
        assert.equal(owner.pages().length, 2);
        assert.equal(page.isClosed(), false); assert.equal(other.url(), `${origin}/other`);
        assert.ok(await other.evaluate(() => window.fixtureTicks) > tick);
      });
    }
  } finally {
    await owner?.close();
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
});
