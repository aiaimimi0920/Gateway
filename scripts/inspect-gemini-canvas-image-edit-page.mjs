/**
 * Focused repo-side helper.
 *
 * Opens the latest persistent Gemini profile, enters Gemini Canvas image mode,
 * opens the upload menu, and dumps the relevant DOM so we can identify the
 * real local-image upload trigger for image-edit capture.
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

const outDir = path.join(storageRoot, "debug", "gemini-canvas", "image-edit-dom-inspect");
mkdirSync(outDir, { recursive: true });

function takeSnapshot(label) {
  return page.evaluate((phase) => {
    const normalizeText = (value) => String(value || "").trim();
    const takeOuterHtml = (node) => normalizeText(node.outerHTML).slice(0, 800);
    return {
      phase,
      url: location.href,
      title: document.title,
      bodyText: (document.body?.innerText ?? "").slice(0, 4000),
      inputs: Array.from(document.querySelectorAll("input"))
        .slice(0, 80)
        .map((node, i) => ({
          i,
          type: node.getAttribute("type"),
          name: node.getAttribute("name"),
          accept: node.getAttribute("accept"),
          dataTestId: node.getAttribute("data-test-id"),
          aria: node.getAttribute("aria-label"),
          hidden: node.hidden,
          disabled: node.disabled,
          className: String(node.className || "").slice(0, 240),
          outerHtml: takeOuterHtml(node),
        })),
      buttons: Array.from(document.querySelectorAll("button"))
        .slice(0, 120)
        .map((node, i) => ({
          i,
          text: normalizeText(node.innerText),
          aria: node.getAttribute("aria-label"),
          title: node.getAttribute("title"),
          dataTestId: node.getAttribute("data-test-id"),
          disabled: node.disabled,
          hidden: node.hidden,
          className: String(node.className || "").slice(0, 240),
          outerHtml: takeOuterHtml(node),
        })),
      menuItems: Array.from(document.querySelectorAll('[role="menuitem"], [role="option"], [role="listitem"]'))
        .slice(0, 80)
        .map((node, i) => ({
          i,
          role: node.getAttribute("role"),
          text: normalizeText(node.innerText),
          aria: node.getAttribute("aria-label"),
          dataTestId: node.getAttribute("data-test-id"),
          className: String(node.className || "").slice(0, 240),
          outerHtml: takeOuterHtml(node),
        })),
      dataTestNodes: Array.from(document.querySelectorAll("[data-test-id]"))
        .slice(0, 120)
        .map((node, i) => ({
          i,
          tag: node.tagName,
          dataTestId: node.getAttribute("data-test-id"),
          role: node.getAttribute("role"),
          aria: node.getAttribute("aria-label"),
          text: normalizeText(node.innerText),
          hidden: Boolean(node.hidden),
          className: String(node.className || "").slice(0, 240),
          outerHtml: takeOuterHtml(node),
        })),
    };
  }, label);
}

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
await page.waitForTimeout(6_000);

await page.getByRole("button", { name: /制作图片|Create image|Create images|Make image/i }).first().click({
  timeout: 30_000,
});
await page.waitForTimeout(1200);
const afterImageMode = await takeSnapshot("after-image-mode");
await page.screenshot({ path: path.join(outDir, "after-image-mode.png"), fullPage: true });

let uploadMenuError = null;
try {
  await page.getByRole("button", { name: /打开文件上传菜单|upload/i }).first().click({
    timeout: 10_000,
    force: true,
  });
  await page.waitForTimeout(1000);
} catch (error) {
  uploadMenuError = error instanceof Error ? error.message : String(error);
}

const afterUploadMenu = await takeSnapshot("after-upload-menu");
await page.screenshot({ path: path.join(outDir, "after-upload-menu.png"), fullPage: true });

const artifact = {
  ok: true,
  outDir,
  profileDir: latestProfileDir.fullPath,
  uploadMenuError,
  afterImageMode,
  afterUploadMenu,
};

writeFileSync(path.join(outDir, "page-info.json"), `${JSON.stringify(artifact, null, 2)}\n`, "utf8");
console.log(JSON.stringify({ ok: true, outDir, uploadMenuError }, null, 2));

await context.close().catch(() => undefined);
