// New profile + loopback only: exercise the actual worker, native request and borrowed-page cleanup.
import test from "node:test";
import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdtemp, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
const executablePath = process.env.GATEWAY_SUNO_CDP_TEST_BROWSER;
const root = process.env.GATEWAY_SUNO_CDP_TEST_ROOT;
test("Suno worker binds the routed native model before the actual POST", {
  skip: !executablePath || !root, timeout: 90000,
}, async (t) => {
  const calls = [];
  let mode = "description", endpoint, releases = 0;
  const server = createServer(async (req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/v1/browser/sessions/acquire") {
      res.end(JSON.stringify({ data: { session: { session_id: "synthetic", attach: { endpoint, page_url: `${origin}/create` } } } }));
    } else if (req.url === "/v1/browser/sessions/synthetic/release") {
      releases++; res.end("{}");
    } else if (req.url === "/api/generate/v2-web/") {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      calls.push({ body: JSON.parse(Buffer.concat(chunks).toString()), authorization: req.headers.authorization });
      res.end(JSON.stringify({ clips: [{ id: "synthetic", status: "complete", audio_url: "https://asset.invalid/not-audio" }] }));
    } else if (req.url === "/sw.js") {
      res.setHeader("content-type", "text/javascript");
      res.end("self.addEventListener('install',()=>self.skipWaiting());self.addEventListener('activate',event=>event.waitUntil(self.clients.claim()));");
    } else {
      res.setHeader("content-type", "text/html");
      res.end(`<title>Create</title><textarea maxlength="3000"></textarea>
        <button aria-label="Create song" onclick="submitSong()">Create song</button><script>
        async function submitSong() {
          const mode = ${JSON.stringify(mode)}, prompt = document.querySelector('textarea').value;
          const body = { mv:'page-default-model', token:'synthetic-challenge', generation_type:'TEXT',
            make_instrumental:false, metadata:{create_mode:'simple',is_max_mode:false},
            prompt:mode === 'prompt' ? prompt : '', gpt_description_prompt:mode === 'prompt' ? '' : prompt };
          if(mode === 'wrong-prompt') body.gpt_description_prompt = 'other task';
          if(mode === 'custom') body.metadata.create_mode = 'custom';
          if(mode === 'instrumental') body.make_instrumental = true;
          if(mode === 'continue') body.continue_clip_id = 'other-song';
          if(mode === 'unknown') body.unknown_operation = 'other task';
          const send = () => fetch('/api/generate/v2-web/', {method:'POST',headers:{'content-type':'application/json',authorization:'Bearer synthetic-session'},body:JSON.stringify(body)}).catch(()=>{});
          await (mode === 'duplicate' ? Promise.all([send(),send()]) : send());
        }</script>`);
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  let owner;
  try {
    const profile = await mkdtemp(join(root, "suno-binding-profile-"));
    owner = await chromium.launchPersistentContext(profile, { executablePath, headless: true,
      args: ["--remote-debugging-port=0", "--no-first-run"] });
    const port = (await readFile(join(profile, "DevToolsActivePort"), "utf8")).split(/\r?\n/)[0];
    endpoint = `http://127.0.0.1:${port}`;
    const page = owner.pages()[0];
    for (const current of ["description", "prompt", "wrong-prompt", "custom", "instrumental", "continue", "unknown", "duplicate", "service-worker"]) {
      await t.test(current, async () => {
        mode = current;
        await page.goto(`${origin}/create`);
        if (current === "service-worker") {
          await page.evaluate(async () => { await navigator.serviceWorker.register('/sw.js'); await navigator.serviceWorker.ready; });
          await page.waitForFunction(() => Boolean(navigator.serviceWorker.controller));
        }
        const before = calls.length, beforeReleases = releases;
        const worker = fileURLToPath(new URL("../suno-browser-worker.mjs", import.meta.url));
        const child = spawn(process.execPath, [worker], { stdio: ["pipe", "pipe", "pipe"],
          signal: AbortSignal.timeout(15000), killSignal: "SIGKILL",
          env: { ...process.env, SUNO_BROWSER_CDP_URL: "", SUNO_EASYBROWSER_BASE_URL: "" } });
        let stdout = "", stderr = "";
        child.stdout.on("data", v => { stdout += v; }); child.stderr.on("data", v => { stderr += v; });
        const closed = new Promise((resolve, reject) => { child.once("error", reject); child.once("close", resolve); });
        child.stdin.end(JSON.stringify({ baseUrl: origin, origin, referer: `${origin}/create`,
          easyBrowserBaseUrl: origin, easyBrowserBearerToken: "synthetic-fixture",
          requestBody: { model: "chirp-auk", prompt: "synthetic local fixture" }, waitCompletion: false }));
        assert.equal(await closed, 0, stderr);
        const result = JSON.parse(stdout);
        if (["description", "prompt"].includes(current)) {
          assert.equal(calls.length, before + 1);
          assert.equal(calls.at(-1).body.mv, "chirp-auk", "outbound model must not be the page's previous selection");
          assert.equal(calls.at(-1).body.token, "synthetic-challenge");
          assert.equal(calls.at(-1).authorization, "Bearer synthetic-session");
          assert.equal(result.ok, true, stdout); assert.equal(result.result.wireModel, "chirp-auk");
        } else {
          assert.equal(result.ok, false, stdout);
          assert.equal(result.error.code, current === "duplicate" ? "suno_browser_ambiguous_generation" : "suno_browser_request_mismatch");
          assert.equal(calls.length, before + (current === "duplicate" ? 1 : 0), "unsupported tasks and second POSTs are not sent");
        }
        assert.ok(!stdout.includes("synthetic-challenge") && !stdout.includes("synthetic-session"));
        assert.equal(releases, beforeReleases + 1);
        assert.equal(page.isClosed(), false); assert.equal(owner.pages().length, 1);
      });
    }
  } finally {
    await owner?.close(); server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
});
