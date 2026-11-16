// Opt-in native Chromium test. Only a new profile and loopback fixture are used.
import test from "node:test";
import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdtemp, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const executablePath = process.env.GATEWAY_SUNO_CDP_TEST_BROWSER;
const evidenceRoot = process.env.GATEWAY_SUNO_CDP_TEST_ROOT;
test("Suno actual CDP success/error release lease without closing borrowed browser/page", {
  skip: !executablePath || !evidenceRoot, timeout: 45000,
}, async () => {
  let generates = 0;
  let releases = 0;
  let endpoint;
  const server = createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/v1/browser/sessions/acquire") {
      res.end(JSON.stringify({ data: { session: { session_id: "synthetic", attach: { endpoint, page_url: `${origin}/create` } } } }));
    } else if (req.url === "/v1/browser/sessions/synthetic/release") {
      releases++; res.end("{}");
    } else if (req.url === "/api/generate/v2-web/") {
      generates++;
      res.end(JSON.stringify({ clips: [{ id: "synthetic", status: "complete", audio_url: `${origin}/not-a-real-audio` }] }));
    } else {
      res.setHeader("content-type", "text/html");
      res.end(`<title>Create</title><textarea maxlength="3000"></textarea><button aria-label="Create song" onclick="fetch('/api/generate/v2-web/', {method:'POST',body:JSON.stringify({mv:'page-default',generation_type:'TEXT',prompt:document.querySelector('textarea').value,make_instrumental:false,metadata:{create_mode:'simple'}})})">Create song</button>`);
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  let owner;
  try {
    const profile = await mkdtemp(join(evidenceRoot, "chromium-profile-"));
    owner = await chromium.launchPersistentContext(profile, {
      executablePath, headless: true, args: ["--remote-debugging-port=0", "--no-first-run"],
    });
    const port = (await readFile(join(profile, "DevToolsActivePort"), "utf8")).split(/\r?\n/)[0];
    endpoint = `http://127.0.0.1:${port}`;
    const page = owner.pages()[0];
    await page.goto(`${origin}/create`);
    const run = async () => {
      const worker = fileURLToPath(new URL("../suno-browser-worker.mjs", import.meta.url));
      const child = spawn(process.execPath, [worker], {
        stdio: ["pipe", "pipe", "pipe"],
        signal: AbortSignal.timeout(15000), killSignal: "SIGKILL",
        env: { ...process.env, SUNO_BROWSER_CDP_URL: "", SUNO_EASYBROWSER_BASE_URL: "" },
      });
      let stdout = "", stderr = "";
      child.stdout.on("data", (value) => { stdout += value; });
      child.stderr.on("data", (value) => { stderr += value; });
      child.stdin.end(JSON.stringify({
        baseUrl: origin, origin, referer: `${origin}/create`,
        easyBrowserBaseUrl: origin, easyBrowserBearerToken: "synthetic-fixture",
        requestBody: { model: "chirp-auk-turbo", prompt: "synthetic local fixture" },
        waitCompletion: false,
      }));
      const code = await new Promise((resolve, reject) => {
        child.once("error", reject); child.once("close", resolve);
      });
      assert.equal(code, 0, stderr);
      return JSON.parse(stdout);
    };
    assert.equal((await run()).ok, true);
    assert.equal(generates, 1);
    assert.equal(releases, 1);
    assert.equal(owner.pages().length, 1);
    assert.equal(page.url(), `${origin}/create`);
    assert.equal(await page.locator("textarea").inputValue(), "synthetic local fixture");
    await page.evaluate(() => { document.title = "Just a moment"; });
    assert.equal((await run()).error.code, "suno_browser_challenge_required");
    assert.equal(releases, 2);
    assert.equal(generates, 1);
    assert.equal(await page.title(), "Just a moment");
    assert.equal(page.isClosed(), false);
  } finally {
    await owner?.close(); // This test owns this isolated browser, never a user profile.
    await new Promise((resolve) => server.close(resolve));
  }
});
