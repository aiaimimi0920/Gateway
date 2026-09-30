/**
 * Focused repo-side helper.
 *
 * Executes one real Gemini Canvas image edit using the latest persistent
 * browser profile, uploads a real local reference image, and
 * broadly records the matching HTTP requests/responses and WebSocket frames
 * so we can reverse the true edit transport contract.
 */

import { chromium } from "playwright-core";
import { Buffer } from "node:buffer";
import { copyFileSync, existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { resolveGeminiCanvasManualLiveVendorProfileDir } from "./gemini-canvas-runtime-paths.mjs";
import { createImageEditBroadCapture } from "./gemini-canvas-image-edit-broad-capture.mjs";
import { collectImageEditBroadPageState, exportImageEditBroadPageBlobs } from "./gemini-canvas-image-edit-broad-page.mjs";

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
const manualOverrideProfile = process.env.GEMINI_CANVAS_PROFILE_OVERRIDE;
const currentVendorProfile = resolveGeminiCanvasManualLiveVendorProfileDir(storageRoot);
const profileRoot = path.join(storageRoot, "credential-runtime", "gemini-canvas-profile");

function resolveProfileDir() {
  if (manualOverrideProfile && existsSync(manualOverrideProfile)) {
    return {
      name: path.basename(manualOverrideProfile),
      fullPath: manualOverrideProfile,
      sortKey: "override",
      source: "env-override",
    };
  }

  if (existsSync(currentVendorProfile)) {
    return {
      name: path.basename(currentVendorProfile),
      fullPath: currentVendorProfile,
      sortKey: "manual-live-vendor-profile",
      source: "manual-live-vendor",
    };
  }

  const latestProfileDir = readdirSync(profileRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => ({
      name: entry.name,
      fullPath: path.join(profileRoot, entry.name, "user-data"),
      sortKey: entry.name,
      source: "latest-gemini-canvas-profile",
    }))
    .filter((entry) => existsSync(entry.fullPath))
    .sort((a, b) => b.sortKey.localeCompare(a.sortKey))[0];

  return latestProfileDir ?? null;
}

const latestProfileDir = resolveProfileDir();

if (!latestProfileDir) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: `No Gemini Canvas browser profile found under ${profileRoot}`,
      },
      null,
      2,
    ),
  );
  process.exit(1);
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
const outDir = path.join(storageRoot, "debug", "gemini-canvas", `image-edit-broad-${stamp}`);
mkdirSync(outDir, { recursive: true });

const marker = process.env.GEMINI_CANVAS_IMAGE_EDIT_MARKER ?? `CANVAS_API_PROBE_IMAGE_EDIT_${stamp}`;
const prompt =
  process.env.GEMINI_CANVAS_IMAGE_EDIT_PROMPT ??
  `${marker} 把上传的样例图编辑成一个霓虹徽章，保留主体，增强边缘发光和科技感。`;
const sourceImageOverride = process.env.GEMINI_CANVAS_IMAGE_EDIT_SOURCE_PATH;
const samplePngBase64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z6f0AAAAASUVORK5CYII=";
const sourceImageExtension = sourceImageOverride
  ? path.extname(sourceImageOverride).toLowerCase() || ".png"
  : ".png";
const sampleImagePath = path.join(outDir, `edit-source${sourceImageExtension}`);
if (sourceImageOverride) {
  if (!existsSync(sourceImageOverride)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          message: `Override image not found: ${sourceImageOverride}`,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }
  copyFileSync(sourceImageOverride, sampleImagePath);
} else {
  writeFileSync(sampleImagePath, Buffer.from(samplePngBase64, "base64"));
}

const capture = createImageEditBroadCapture({ marker, outDir });
let uploadTrigger = null;

async function uploadReferenceImage(page, imagePath) {
  const triggerFileChooser = async (locator, label, { force = false } = {}) => {
    const chooserPromise = page.waitForEvent("filechooser", { timeout: 15_000 });
    await locator.click({ timeout: 15_000, force });
    const chooser = await chooserPromise;
    await chooser.setFiles(imagePath);
    uploadTrigger = label;
    return true;
  };

  const uploadMenuButton = page.getByRole("button", { name: /打开文件上传菜单|upload/i }).first();
  try {
    await uploadMenuButton.click({ timeout: 8_000, force: true });
    await page.waitForTimeout(400);
  } catch {
    // keep trying other selectors
  }

  const followupTriggers = [
    [
      page.locator('[data-test-id="local-images-files-uploader-button"]').first(),
      "upload-menu-local-images-files-uploader-button",
      { force: false },
    ],
    [
      page.locator(".hidden-local-file-image-selector-button").first(),
      "upload-menu-hidden-local-file-image-selector-button",
      { force: false },
    ],
    [
      page.locator('[data-test-id="local-images-files-uploader-button"]').first(),
      "upload-menu-local-images-files-uploader-button-force",
      { force: true },
    ],
    [
      page.locator('[data-test-id="hidden-local-image-upload-button"]').first(),
      "upload-menu-hidden-local-image-upload-button-force",
      { force: true },
    ],
    [
      page.getByRole("button", { name: /图片|image/i }).first(),
      "upload-menu-image-button-force",
      { force: true },
    ],
    [
      page.getByRole("menuitem", { name: /图片|image/i }).first(),
      "upload-menu-image-menuitem-force",
      { force: true },
    ],
  ];

  for (const [locator, label, options] of followupTriggers) {
    try {
      return await triggerFileChooser(locator, label, options);
    } catch {
      // try next
    }
  }

  throw new Error("Failed to trigger Gemini Canvas local image upload chooser.");
}

const context = await chromium.launchPersistentContext(latestProfileDir.fullPath, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

try {
  const page = context.pages()[0] ?? (await context.newPage());
  const cdp = await context.newCDPSession(page);
  await cdp.send("Network.enable", {
    maxTotalBufferSize: 128 * 1024 * 1024,
    maxResourceBufferSize: 32 * 1024 * 1024,
  });

  await capture.attach(cdp, page);

  await page.goto("https://gemini.google.com/app", {
    waitUntil: "domcontentloaded",
    timeout: 60_000,
  });
  await page.waitForTimeout(6_000);

  await page.getByRole("button", { name: /制作图片|Create image|Create images|Make image/i }).first().click({
    timeout: 30_000,
  });
  await page.waitForTimeout(1_000);

  await uploadReferenceImage(page, sampleImagePath);
  await page.waitForTimeout(3_000);

  const textbox = page
    .locator(
      '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
    )
    .first();
  await textbox.click({ timeout: 30_000 });
  await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
  await page.keyboard.type(prompt, { delay: 16 });
  await page.waitForTimeout(400);
  const sendButton = page.getByRole("button", { name: /发送|Send|提交/i }).first();
  await sendButton.click({ timeout: 30_000, force: true }).catch(() => undefined);
  await page.waitForTimeout(800);
  if (!capture.markerTransport) {
    await page.keyboard.press("Enter").catch(() => undefined);
  }

  const waitUntil = Date.now() + 5 * 60 * 1000;
  while (Date.now() < waitUntil && !capture.markerTransport) {
    await page.waitForTimeout(1_000);
  }

  const imageWaitUntil = Date.now() + 12 * 60 * 1000;
  let imageNodes = [];
  let anchorNodes = [];
  let cssBgNodes = [];
  let canvasNodes = [];
  let buttonNodes = [];
  let pageState = null;
  let exportedPageBlobs = [];
  let hasGeneratedImage = false;
  let hasAiGeneratedImage = false;
  let hasUploadedPreviewImage = false;
  let hasInterestingAnchor = false;
  let hasInterestingCssBackground = false;
  let hasLargeCanvas = false;
  let hasDownloadUi = false;
  let strongSuccessSignal = false;
  let styleSelectionHandled = false;
  while (Date.now() < imageWaitUntil) {
    if (!styleSelectionHandled) {
      const styleSelectionVisible = await page
        .getByText(/为图片选择风格|选择风格|Choose a style/i)
        .first()
        .isVisible({ timeout: 1_000 })
        .catch(() => false);
      if (styleSelectionVisible) {
        const styleLocators = [
          page.locator('img[alt*="珐琅胸针"]').first(),
          page.getByText(/珐琅胸针|Enamel pin/i).first(),
          page.locator('img[alt*="单色"]').first(),
          page.getByText(/单色|Monochrome/i).first(),
        ];
        for (const locator of styleLocators) {
          try {
            await locator.click({ timeout: 5_000, force: true });
            await page.waitForTimeout(800);
            await sendButton.click({ timeout: 10_000, force: true }).catch(() => undefined);
            await page.keyboard.press("Enter").catch(() => undefined);
            styleSelectionHandled = true;
            break;
          } catch {
            // try next style selector
          }
        }
      }
    }

    ({ imageNodes, anchorNodes, cssBgNodes, canvasNodes, buttonNodes, pageState } = await collectImageEditBroadPageState(page));

    hasGeneratedImage = imageNodes.some(
      (node) =>
        /googleusercontent|gstatic|blob:|data:image\/|gg-dl|rd-gg-dl/i.test(node.src) &&
        ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256),
    );
    hasAiGeneratedImage = imageNodes.some(
      (node) =>
        /AI\s*生成/i.test(node.alt ?? "") &&
        /googleusercontent|gstatic|blob:|data:image\/|gg-dl|rd-gg-dl/i.test(node.src) &&
        ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256),
    );
    hasUploadedPreviewImage = imageNodes.some(
      (node) =>
        /上传图片的预览图/i.test(node.alt ?? "") &&
        /googleusercontent|gstatic|blob:|data:image\/|gg-dl|rd-gg-dl/i.test(node.src) &&
        ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256),
    );
    hasInterestingAnchor = anchorNodes.some((node) =>
      /googleusercontent|gg-dl|rd-gg-dl|\.png(\?|$)|\.jpg(\?|$)|\.jpeg(\?|$)|\.webp(\?|$)/i.test(
        node.href ?? "",
      ),
    );
    hasInterestingCssBackground = cssBgNodes.some(
      (node) =>
        /googleusercontent|gg-dl|rd-gg-dl|data:image|blob:/i.test(node.backgroundImage ?? "") &&
        (((node.width ?? 0) >= 256) || ((node.height ?? 0) >= 256)),
    );
    hasLargeCanvas = canvasNodes.length > 0;
    const hasMarkerResponse = Boolean(
      capture.markerTransport &&
        capture.events.some((event) => event.kind === "response" && event.url === capture.markerTransport.url),
    );
    hasDownloadUi = buttonNodes.some((node) =>
      /(download|下载|save|保存|open|打开)/i.test(`${node.text ?? ""} ${node.aria ?? ""}`),
    );
    strongSuccessSignal =
      hasAiGeneratedImage ||
      hasInterestingAnchor ||
      hasInterestingCssBackground ||
      hasDownloadUi;

    if (capture.markerResponseSeenAt && strongSuccessSignal) {
      if (
        capture.secondarySignalerPollSeenAt &&
        !capture.secondarySignalerPollCompletedAt &&
        Date.now() - capture.secondarySignalerPollSeenAt < 270_000
      ) {
        await page.waitForTimeout(2_000);
        continue;
      }
      break;
    }
    if (
      hasMarkerResponse &&
      capture.markerResponseSeenAt &&
      Date.now() - capture.markerResponseSeenAt >= 180_000 &&
      strongSuccessSignal
    ) {
      if (
        capture.secondarySignalerPollSeenAt &&
        !capture.secondarySignalerPollCompletedAt &&
        Date.now() - capture.secondarySignalerPollSeenAt < 270_000
      ) {
        await page.waitForTimeout(2_000);
        continue;
      }
      break;
    }
    await page.waitForTimeout(2_000);
  }

  await page.screenshot({
    path: path.join(outDir, "page.png"),
    fullPage: true,
  });
  const finalHtml = await page.content();
  writeFileSync(path.join(outDir, "page.html"), finalHtml, "utf8");

  exportedPageBlobs = await exportImageEditBroadPageBlobs(page, outDir);

  const storageStatePath = path.join(outDir, "storage-state.json");
  await context.storageState({ path: storageStatePath });

  const artifact = {
    ok: strongSuccessSignal,
    capturedAt: new Date().toISOString(),
    runtimeStateObjectKey: path.relative(storageRoot, latestProfileDir.fullPath).replaceAll(path.sep, "/"),
    profileDir: latestProfileDir.fullPath,
    profileSource: latestProfileDir.source,
    sampleImagePath,
    prompt,
    uploadTrigger,
    markerTransport: capture.markerTransport,
    markerRequestSeenAt: capture.markerRequestSeenAt,
    markerResponseSeenAt: capture.markerResponseSeenAt,
    secondarySignalerPollUrl: capture.secondarySignalerPollUrl,
    secondarySignalerPollSeenAt: capture.secondarySignalerPollSeenAt,
    secondarySignalerPollCompletedAt: capture.secondarySignalerPollCompletedAt,
    finalizeUploadCapture: capture.finalizeUploadCapture,
    imageNodes,
    anchorNodes,
    cssBgNodes,
    canvasNodes,
    buttonNodes,
    pageState,
    exportedPageBlobs,
    pageUrl: page.url(),
    strongSuccessSignal,
    hasGeneratedImage,
    hasAiGeneratedImage,
    hasUploadedPreviewImage,
    hasInterestingAnchor,
    hasInterestingCssBackground,
    hasLargeCanvas,
    hasDownloadUi,
    events: capture.events,
    note: "Focused broad Gemini Canvas image edit probe only.",
  };

  writeFileSync(path.join(outDir, "image-edit-broad.json"), `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

  console.log(
    JSON.stringify(
      {
        ok: artifact.ok,
        outDir,
        runtimeStateObjectKey: artifact.runtimeStateObjectKey,
        uploadTrigger,
        markerTransportKind: capture.markerTransport?.kind ?? null,
        markerTransportUrl: capture.markerTransport?.url ?? null,
        exportedPageBlobCount: exportedPageBlobs.length,
        imageNodeCount: imageNodes.length,
        anchorNodeCount: anchorNodes.length,
        pageUrl: artifact.pageUrl,
        strongSuccessSignal: artifact.strongSuccessSignal,
        hasAiGeneratedImage: artifact.hasAiGeneratedImage,
        hasUploadedPreviewImage: artifact.hasUploadedPreviewImage,
      },
      null,
      2,
    ),
  );
} finally {
  try {
    await capture.stop();
  } catch {
    // Cleanup must not replace the probe outcome.
  }
  await context.close().catch(() => undefined);
}
