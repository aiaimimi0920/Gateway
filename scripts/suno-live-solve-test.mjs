// Self-serve Suno live generation test.
// Usage: node scripts/suno-live-solve-test.mjs
// It will: ensure a prompt, open the Cloudflare Turnstile modal by clicking Create,
// then WAIT up to 5 minutes for YOU to click the "请验证您是真人" checkbox in the
// Chrome window. On solve, Suno auto-submits the generate request; if it doesn't,
// the script clicks Create once more. Finally it prints the generate result.
import { chromium } from "playwright-core";

const PORT = process.env.SUNO_CDP_PORT || "9226";
const log = (m) => console.log(`[${new Date().toISOString().slice(11, 19)}] ${m}`);

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`, { timeout: 60_000 });
const context = browser.contexts()[0];
const page = context.pages().find((p) => p.url().startsWith("https://suno.com"));
if (!page) throw new Error("no suno.com page found on CDP port " + PORT);

let done = false;
page.on("response", async (res) => {
  if (/\/api\/generate\/v2-web\/?$/.test(res.url())) {
    let body = "";
    try { body = await res.text(); } catch {}
    let reqInfo = "?";
    try {
      const b = JSON.parse(res.request().postData() || "{}");
      reqInfo = `token=${b.token === null ? "null" : "len" + String(b.token).length} provider=${JSON.stringify(b.token_provider)}`;
    } catch {}
    log(`GENERATE ${res.status()} | req ${reqInfo} | resp ${body.slice(0, 220)}`);
    if (res.status() === 200) { done = true; log("SUCCESS — generation accepted."); }
  }
});

const modalOpen = () =>
  page.evaluate(() =>
    Array.from(document.querySelectorAll("div")).some(
      (d) => typeof d.className === "string" && d.className.includes("z-[1000]") && d.className.includes("inset-0"),
    ),
  ).catch(() => false);

const readToken = async () => {
  let max = 0;
  for (const f of page.frames()) {
    try {
      const v = await f.evaluate(() => {
        const el = document.querySelector('[name="cf-turnstile-response"]');
        return el ? (el.value || "").length : 0;
      });
      if (v > max) max = v;
    } catch {}
  }
  return max;
};

const clickCreate = async () => {
  let btn = page.locator('button:visible[aria-label="创作歌曲"], button:visible[aria-label="Create song"]').first();
  if (!(await btn.count())) btn = page.locator('button:visible:has-text("创作"), button:visible:has-text("Create")').last();
  await btn.click({ timeout: 8000 }).catch((e) => log("create click blocked: " + String(e).split("\n")[0]));
};

const promptBox = page.locator('textarea:visible[maxlength="3000"]').first();
await promptBox.waitFor({ state: "visible", timeout: 10_000 });
if (!(await promptBox.inputValue())) await promptBox.fill("a short gentle acoustic song about morning coffee");

if (!(await modalOpen())) {
  log("clicking Create to open the verification modal…");
  await clickCreate();
  await page.waitForTimeout(3000);
}
log('>>> Please click the "请验证您是真人" checkbox in the Chrome window now. Waiting up to 5 min…');

const deadline = Date.now() + 300_000;
let solvedAnnounced = false;
let reclickTried = false;
while (Date.now() < deadline && !done) {
  const tok = await readToken();
  if (tok > 20 && !solvedAnnounced) {
    solvedAnnounced = true;
    log(`token solved (len=${tok}); waiting for auto-submit…`);
    setTimeout(async () => {
      if (!done && !reclickTried) { reclickTried = true; log("no auto-submit; clicking Create again with token…"); await clickCreate(); }
    }, 8000);
  }
  await page.waitForTimeout(1500);
}
if (!done) log("no successful generation within the window.");
await browser.close();
