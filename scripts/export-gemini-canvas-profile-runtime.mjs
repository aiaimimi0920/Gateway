import { chromium } from "playwright-core";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

function extractSignalerAccountIdFromPageBlob(blob) {
  if (typeof blob !== "string" || !blob) {
    return null;
  }
  const match = blob.match(/"S06Grb":"([^"]+)"/);
  const candidate = match?.[1]?.trim();
  return candidate ? candidate : null;
}

async function harvestSignalerAccountIdFromContext(context) {
  const page = await context.newPage();
  let finalUrl = null;
  let pageContent = "";
  let pageReportedSignalerAccountId = null;
  let pageContainsLoginCta = false;

  try {
    await page.goto("https://gemini.google.com/app", {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });
    await page.waitForTimeout(2_000);
    await page
      .waitForFunction(() => Boolean(globalThis.WIZ_global_data), {
        timeout: 10_000,
      })
      .catch(() => null);
    finalUrl = page.url();
    pageReportedSignalerAccountId =
      (await page
        .evaluate(() => {
          const raw = globalThis.WIZ_global_data?.S06Grb;
          return typeof raw === "string" && raw.trim() ? raw.trim() : null;
        })
        .catch(() => null)) ?? null;
    pageContent = await page.content();
    pageContainsLoginCta =
      /ServiceLogin|登录|signin/i.test(pageContent);
  } finally {
    await page.close().catch(() => {});
  }

  return {
    finalUrl,
    pageReportedSignalerAccountId,
    pageBlobSignalerAccountId: extractSignalerAccountIdFromPageBlob(pageContent),
    pageContainsLoginCta,
  };
}

function parseArgs(argv) {
  const result = new Map();
  for (let index = 2; index < argv.length; index += 1) {
    const key = argv[index];
    if (!key.startsWith("--")) {
      continue;
    }
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      result.set(key, "true");
      continue;
    }
    result.set(key, next);
    index += 1;
  }
  return result;
}

function requiredArg(args, key) {
  const value = args.get(key);
  if (!value) {
    throw new Error(`Missing required argument: ${key}`);
  }
  return value;
}

function detectBrowserExecutable() {
  const candidates = [
    process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH,
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  ].filter(Boolean);
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function backupIfChanged(filePath, nextText) {
  if (!existsSync(filePath)) {
    return null;
  }
  const currentText = readFileSync(filePath, "utf8");
  if (currentText === nextText) {
    return null;
  }
  const now = new Date();
  const stamp = [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, "0"),
    String(now.getDate()).padStart(2, "0"),
    "-",
    String(now.getHours()).padStart(2, "0"),
    String(now.getMinutes()).padStart(2, "0"),
    String(now.getSeconds()).padStart(2, "0"),
  ].join("");
  const backupPath = `${filePath}.bak-${stamp}`;
  copyFileSync(filePath, backupPath);
  return backupPath;
}

const args = parseArgs(process.argv);
const profileDir = path.resolve(requiredArg(args, "--profile-dir"));
const captureJsonPath = path.resolve(requiredArg(args, "--capture-json"));
const outputDir = path.resolve(requiredArg(args, "--output-dir"));
const existingStorageStatePath = path.resolve(
  args.get("--existing-storage-state") ?? path.join(outputDir, "storage-state.json"),
);
const browserExecutable = detectBrowserExecutable();

if (!browserExecutable) {
  throw new Error("No Chromium-compatible browser executable found.");
}
if (!existsSync(profileDir)) {
  throw new Error(`Profile directory not found: ${profileDir}`);
}
if (!existsSync(captureJsonPath)) {
  throw new Error(`Capture JSON not found: ${captureJsonPath}`);
}

mkdirSync(outputDir, { recursive: true });

const capture = JSON.parse(readFileSync(captureJsonPath, "utf8"));
if (
  !capture?.markerTransport?.url ||
  !capture?.markerTransport?.headers ||
  !capture?.markerTransport?.postData
) {
  throw new Error(
    `Capture JSON does not expose markerTransport url/headers/postData: ${captureJsonPath}`,
  );
}

const context = await chromium.launchPersistentContext(profileDir, {
  executablePath: browserExecutable,
  headless: true,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

let exportedState;
let signalerProbe;
try {
  signalerProbe = await harvestSignalerAccountIdFromContext(context);
  exportedState = await context.storageState();
} finally {
  await context.close();
}

let preservedSignalerAccountId = null;
if (existsSync(existingStorageStatePath)) {
  try {
    const existingState = JSON.parse(readFileSync(existingStorageStatePath, "utf8"));
    preservedSignalerAccountId = existingState?.signalerAccountId ?? null;
  } catch {
    preservedSignalerAccountId = null;
  }
}

const mergedState = {
  ...exportedState,
  ...((signalerProbe?.pageReportedSignalerAccountId ||
    signalerProbe?.pageBlobSignalerAccountId ||
    exportedState.signalerAccountId ||
    preservedSignalerAccountId)
    ? {
        signalerAccountId:
          signalerProbe?.pageReportedSignalerAccountId ??
          signalerProbe?.pageBlobSignalerAccountId ??
          exportedState.signalerAccountId ??
          preservedSignalerAccountId,
      }
    : {}),
};
const storageStatePath = path.join(outputDir, "storage-state.json");
const sidecarPath = path.join(outputDir, "image-edit-stream-generate-template.json");

const sidecar = {
  imageEditStreamGenerateTemplate: {
    url: capture.markerTransport.url,
    headers: capture.markerTransport.headers,
    postData: capture.markerTransport.postData,
  },
  sourceCapture: captureJsonPath,
  capturedAt: capture.capturedAt ?? new Date().toISOString(),
  strongSuccessSignal: Boolean(capture.strongSuccessSignal),
  hasGeneratedImage: Boolean(capture.hasGeneratedImage ?? capture.hasAiGeneratedImage),
  hasDownloadUi: Boolean(capture.hasDownloadUi),
};

const storageStateText = `${JSON.stringify(mergedState, null, 2)}\n`;
const sidecarText = `${JSON.stringify(sidecar, null, 2)}\n`;
const storageStateBackup = backupIfChanged(storageStatePath, storageStateText);
const sidecarBackup = backupIfChanged(sidecarPath, sidecarText);
writeFileSync(storageStatePath, storageStateText, "utf8");
writeFileSync(sidecarPath, sidecarText, "utf8");

console.log(
  JSON.stringify(
    {
      ok: true,
      browserExecutable,
      profileDir,
      captureJsonPath,
      outputDir,
      storageStatePath,
      storageStateBackup,
      storageStateCookieCount: mergedState.cookies?.length ?? null,
      storageStateOriginCount: mergedState.origins?.length ?? null,
      signalerAccountId: mergedState.signalerAccountId ?? null,
      signalerProbe: signalerProbe ?? null,
      sidecarPath,
      sidecarBackup,
      sidecarPromptPreview:
        typeof capture.prompt === "string" ? capture.prompt.slice(0, 160) : null,
    },
    null,
    2,
  ),
);
