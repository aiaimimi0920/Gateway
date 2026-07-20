/**
 * Formal probe for the first true-Canvas hurdle:
 *
 * Can we establish and observe a valid Gemini Canvas program context from a
 * real browser profile and a share-page seed?
 *
 * This script is intentionally read-heavy and evidence-oriented. It does not
 * attempt to prove the full quota lane yet; it freezes the browser/runtime
 * state we need before the later program-owned request-path work begins.
 */

import { chromium } from "playwright-core";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { resolveGeminiCanvasManualLiveVendorProfileDir } from "./gemini-canvas-runtime-paths.mjs";

const WINDOWS_BROWSER_PATHS = [
  process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH,
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
].filter(Boolean);

const executablePath = WINDOWS_BROWSER_PATHS.find(candidate => existsSync(candidate));
if (!executablePath) {
  console.error(
    JSON.stringify({ ok: false, message: "No Chromium-compatible browser found." }, null, 2),
  );
  process.exit(1);
}

const storageRoot = path.resolve(
  process.cwd(),
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects",
);

function resolveProfileDir() {
  const envDir = process.env.GEMINI_CANVAS_PROFILE_DIR;
  if (envDir && existsSync(envDir)) {
    return envDir;
  }

  const manualVendor = resolveGeminiCanvasManualLiveVendorProfileDir(storageRoot);
  if (existsSync(manualVendor)) {
    return manualVendor;
  }

  const profileRoot = path.join(storageRoot, "credential-runtime", "gemini-canvas-profile");
  if (!existsSync(profileRoot)) {
    return null;
  }
  const latest = readdirSync(profileRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => path.join(profileRoot, entry.name, "user-data"))
    .filter(candidate => existsSync(candidate))
    .sort((a, b) => b.localeCompare(a))[0];
  return latest ?? null;
}

const profileDir = resolveProfileDir();
if (!profileDir) {
  console.error(
    JSON.stringify(
      { ok: false, message: "No Gemini Canvas browser profile directory could be resolved." },
      null,
      2,
    ),
  );
  process.exit(1);
}

const shareUrl =
  process.env.GEMINI_CANVAS_SHARE_URL ??
  `https://gemini.google.com/share/${process.env.GEMINI_CANVAS_SHARE_ID ?? "fe24c455a570"}`;
const appUrl = process.env.GEMINI_CANVAS_APP_URL ?? "https://gemini.google.com/app";
const timeoutMs = Math.max(Number(process.env.GEMINI_CANVAS_PROBE_TIMEOUT_MS ?? "60000"), 15000);

const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
const outDir = path.join(
  process.cwd(),
  ".runtime",
  "gemini-canvas-program-probe",
  timestamp,
);
mkdirSync(outDir, { recursive: true });

function textPreview(value, max = 4000) {
  return String(value ?? "").slice(0, max);
}

async function collectSnapshot(page, label) {
  const snapshot = await page.evaluate(currentLabel => {
    const bodyText = document.body?.innerText ?? "";
    const buttons = Array.from(document.querySelectorAll("button,[role=\"button\"],a[role=\"button\"]"))
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter(entry => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 200);
    const textboxes = Array.from(
      document.querySelectorAll('[role="textbox"], textarea, [contenteditable="true"], input'),
    )
      .map((node, index) => ({
        index,
        tag: node.tagName,
        role: node.getAttribute("role"),
        placeholder: node.getAttribute("placeholder"),
        ariaLabel: node.getAttribute("aria-label"),
      }))
      .slice(0, 80);
    return {
      label: currentLabel,
      url: location.href,
      title: document.title,
      bodyText,
      buttons,
      textboxes,
    };
  }, label);
  writeFileSync(
    path.join(outDir, `${label}.json`),
    `${JSON.stringify(
      {
        ...snapshot,
        bodyText: textPreview(snapshot.bodyText, 12000),
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
  await page.screenshot({
    path: path.join(outDir, `${label}.png`),
    fullPage: true,
  });
  return snapshot;
}

const context = await chromium.launchPersistentContext(profileDir, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

try {
  const page = context.pages()[0] ?? (await context.newPage());

  await page.goto(shareUrl, {
    waitUntil: "domcontentloaded",
    timeout: timeoutMs,
  });
  await page.waitForTimeout(5000);
  const shareSnapshot = await collectSnapshot(page, "share");

  await page.goto(appUrl, {
    waitUntil: "domcontentloaded",
    timeout: timeoutMs,
  });
  await page.waitForTimeout(5000);
  const appSnapshot = await collectSnapshot(page, "app");

  const bodyJoined = `${shareSnapshot.bodyText}\n${appSnapshot.bodyText}`;
  const findings = {
    ok: true,
    profileDir,
    executablePath,
    shareUrl,
    appUrl,
    outDir,
    shareFinalUrl: shareSnapshot.url,
    appFinalUrl: appSnapshot.url,
    canvasKeywordsSeen: /(Canvas|画布|启用 Canvas|Create image|创作音乐|创作视频|制作图片)/i.test(
      bodyJoined,
    ),
    textboxesSeen: shareSnapshot.textboxes.length + appSnapshot.textboxes.length,
    buttonCount: shareSnapshot.buttons.length + appSnapshot.buttons.length,
    note:
      "This probe only proves whether a share/app-driven Canvas context is present. It does not yet prove the separate program quota lane.",
  };

  writeFileSync(path.join(outDir, "summary.json"), `${JSON.stringify(findings, null, 2)}\n`, "utf8");
  console.log(JSON.stringify(findings, null, 2));
} finally {
  await context.close().catch(() => undefined);
}
