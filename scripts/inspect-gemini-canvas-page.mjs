/**
 * Focused repo-side helper.
 *
 * Opens the latest persistent Gemini profile, snapshots the current app page,
 * and writes DOM/accessibility artifacts so we can automate image generation.
 */

import { chromium } from "playwright-core";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const WINDOWS_BROWSER_PATHS = [
  process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH,
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
].filter(Boolean);

const executablePath = WINDOWS_BROWSER_PATHS.find((candidate) => existsSync(candidate));
if (!executablePath) {
  console.error(JSON.stringify({ ok: false, message: "No Chromium-compatible browser found." }));
  process.exit(1);
}

const storageRoot = path.resolve(
  process.cwd(),
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects",
);
const profileRoot = path.join(storageRoot, "credential-runtime", "gemini-canvas-profile");
const latestProfileDir = readdirSync(profileRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => ({
    name: entry.name,
    fullPath: path.join(profileRoot, entry.name, "user-data"),
    sortKey: entry.name,
  }))
  .filter((entry) => existsSync(entry.fullPath))
  .sort((a, b) => b.sortKey.localeCompare(a.sortKey))[0];

if (!latestProfileDir) {
  console.error(JSON.stringify({ ok: false, message: `No profile found under ${profileRoot}` }));
  process.exit(1);
}

const outDir = path.join(storageRoot, "debug", "gemini-canvas", "dom-inspect");
mkdirSync(outDir, { recursive: true });

const context = await chromium.launchPersistentContext(latestProfileDir.fullPath, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

const page = context.pages()[0] ?? (await context.newPage());
await page.goto("https://gemini.google.com/app", {
  waitUntil: "domcontentloaded",
  timeout: 60_000,
});
await page.waitForTimeout(8_000);

const domInfo = await page.evaluate(() => ({
  url: location.href,
  title: document.title,
  textareas: Array.from(document.querySelectorAll("textarea")).map((el, i) => ({
    i,
    placeholder: el.getAttribute("placeholder"),
    aria: el.getAttribute("aria-label"),
    disabled: el.matches(":disabled"),
    value: el.value,
  })),
  inputs: Array.from(document.querySelectorAll("input"))
    .slice(0, 40)
    .map((el, i) => ({
      i,
      type: el.type,
      placeholder: el.getAttribute("placeholder"),
      aria: el.getAttribute("aria-label"),
      disabled: el.matches(":disabled"),
      value: el.value,
    })),
  editable: Array.from(
    document.querySelectorAll('[contenteditable="true"], [role="textbox"], textarea, input'),
  )
    .slice(0, 40)
    .map((el, i) => ({
      i,
      tag: el.tagName,
      role: el.getAttribute("role"),
      aria: el.getAttribute("aria-label"),
      placeholder: el.getAttribute("placeholder"),
      disabled: el.matches(":disabled"),
      text: el.textContent,
      className: String(el.className || "").slice(0, 240),
      outerHtml: el.outerHTML.slice(0, 600),
    })),
  buttons: Array.from(document.querySelectorAll("button"))
    .slice(0, 160)
    .map((el, i) => ({
      i,
      text: (el.innerText || "").trim(),
      aria: el.getAttribute("aria-label"),
      title: el.getAttribute("title"),
      dataTestId: el.getAttribute("data-test-id"),
      disabled: el.matches(":disabled"),
    })),
  bodyText: document.body.innerText.slice(0, 5000),
}));

await page.screenshot({
  path: path.join(outDir, "page.png"),
  fullPage: true,
});
writeFileSync(path.join(outDir, "page-info.json"), `${JSON.stringify(domInfo, null, 2)}\n`, "utf8");

console.log(
  JSON.stringify(
    {
      ok: true,
      outDir,
      url: domInfo.url,
      title: domInfo.title,
      textareaCount: domInfo.textareas.length,
      inputCount: domInfo.inputs.length,
      buttonCount: domInfo.buttons.length,
      profileDir: latestProfileDir.fullPath,
    },
    null,
    2,
  ),
);

await context.close().catch(() => undefined);
