/**
 * Probe a concrete Gemini Canvas program handle by:
 *
 * 1. opening a logged-in Gemini browser profile
 * 2. starting from a concrete share seed when available
 * 3. attempting to follow the share page CTA into an editable Canvas/app context
 * 4. optionally selecting a target mode for contract discovery
 * 5. extracting reusable handle evidence from:
 *    - page URL
 *    - history.state
 *    - anchor hrefs
 *    - StreamGenerate / batchexecute response bodies
 *
 * The goal is not yet to prove the separate Canvas quota lane. The immediate
 * goal is to turn "generic /app is reachable" into a stronger, reusable handle
 * candidate such as `/app/<id>`, `c_<id>`, or a share CTA that deterministically
 * materializes one. By default this probe is discovery-only: it must not rely
 * on page-side media generation as the final invoke path.
 */

import {
  uniqueStrings,
  extractAppPathsFromText,
  extractConversationIdsFromText,
  extractResponseIdsFromText,
  extractSharePathsFromText,
  extractHandleHintsFromText,
  sanitizeTransportHintUrl,
  deriveInvokeBaseUrlFromRequestUrl,
  deriveVideoInvokePathFromRequestUrl,
  deriveMusicWsUrlFromRequestUrl,
  extractTransportHintsFromUrl,
  mergeTransportHints,
  buildHandlePair,
  normalizeHandleId,
  extractHandlePairsFromText,
  mergeHints,
  dedupeHandlePairs,
  mergeHandlePairs,
  extractAppPath,
  selectProgramConversationPair,
  selectLatestResponsePair,
  hasCanvasProxyProgramCandidate,
  concreteAppPathFromUrl,
  selectCanonicalProgramPair,
  deriveShareId,
  deriveConversationIdForAppPath,
} from "./gemini-canvas-program-handle-evidence.mjs";
import { createProgramHandleMediaOwner } from "./gemini-canvas-program-handle-media.mjs";
import { createInvokeContractOwner } from "./gemini-canvas-browser-pool-invoke-merge.mjs";
import { mergeActionContract, extractCanvasProgramActionContractFromText, extractQuotedScalar, extractNumberScalar, extractDurationSecondsFromBodyText } from "./gemini-canvas-browser-pool-action.mjs";
import { createProgramRpcOwner } from "./gemini-canvas-browser-pool-rpc-candidates.mjs";
import { invokeContractIndicatesConcreteProgress } from "./gemini-canvas-browser-pool-media-policy.mjs";
import { createProxyDiscoveryOwner } from "./gemini-canvas-browser-pool-proxy-discovery.mjs";
import { createInvokeAssemblyOwner } from "./gemini-canvas-browser-pool-invoke.mjs";
import {
  waitForShareSurface,
  hasPromptTextbox,
  clickFirstVisible,
  tryFollowShareEntryPoint,
  clickNewChat,
  clickOperationMode,
  clickMediaActionButton,
  trySelectMusicStyleCard,
  submitPrompt,
} from "./gemini-canvas-program-handle-interaction.mjs";
import { createProgramHandleSnapshotOwner } from "./gemini-canvas-program-handle-snapshot.mjs";
import { createProgramCaptureOwner } from "./gemini-canvas-browser-pool-capture-metadata.mjs";
import { executeProgramHandleProbe } from "./gemini-canvas-program-handle-execution.mjs";
import { createProgramNetworkCaptureOwner } from "./gemini-canvas-program-handle-network-capture.mjs";
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

const executablePath = WINDOWS_BROWSER_PATHS.find((candidate) => existsSync(candidate));
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

function toObjectKeyFromLocalPath(localPath) {
  const resolved = path.resolve(localPath);
  const relative = path.relative(storageRoot, resolved);
  if (!relative || relative.startsWith("..")) {
    const marker = `${path.sep}credential-runtime${path.sep}`;
    const markerIndex = resolved.toLowerCase().indexOf(marker.toLowerCase());
    if (markerIndex >= 0) {
      return resolved
        .slice(markerIndex + 1)
        .split(path.sep)
        .join("/");
    }
    return null;
  }
  return relative.split(path.sep).join("/");
}

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
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(profileRoot, entry.name, "user-data"))
    .filter((candidate) => existsSync(candidate))
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
const operation = (process.env.GEMINI_CANVAS_PROGRAM_HANDLE_OPERATION ?? "image").trim().toLowerCase();
function defaultProbePrompt(currentOperation) {
  const marker = Date.now();
  switch (currentOperation) {
    case "text":
      return `CANVAS_PROGRAM_BOOTSTRAP_TEXT_${marker} Reply with exactly: ok`;
    case "music":
      return `CANVAS_PROGRAM_BOOTSTRAP_MUSIC_${marker} A short electronic cue with a clear pulse.`;
    case "video":
      return `CANVAS_PROGRAM_BOOTSTRAP_VIDEO_${marker} A 3 second clip of a glowing cube rotating on a clean background.`;
    default:
      return `CANVAS_PROGRAM_BOOTSTRAP_IMAGE_${marker} A neon badge that says CANVAS PROGRAM, high detail, Requested aspect ratio: 1:1.`;
  }
}
const prompt =
  process.env.GEMINI_CANVAS_PROGRAM_HANDLE_PROMPT ?? defaultProbePrompt(operation);
const discoveryOnly = parseBooleanString(
  process.env.GEMINI_CANVAS_PROGRAM_HANDLE_DISCOVERY_ONLY,
  true,
);
const timeoutMs = Math.max(Number(process.env.GEMINI_CANVAS_PROBE_TIMEOUT_MS ?? "120000"), 30000);

const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
const outDir = path.join(
  process.cwd(),
  ".runtime",
  "gemini-canvas-program-handle-probe",
  timestamp,
);
mkdirSync(outDir, { recursive: true });

const INTERESTING_PROGRAM_RPC_IDS = new Set([
  "ujx1Bf",
  "hNvQHb",
  "kwDCne",
  "MUAZcd",
  "qpEbW",
  "aPya6c",
  "MaZiqc",
  "ESY5D",
  "XhaU0b",
  "k81mDb",
]);

function textPreview(value, max = 4000) {
  return String(value ?? "").slice(0, max);
}

function parseBooleanString(rawValue, fallback) {
  const normalized = String(rawValue ?? "").trim().toLowerCase();
  if (!normalized) {
    return fallback;
  }
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }
  return fallback;
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function readRpcIdFromUrl(url) {
  try {
    return new URL(String(url ?? "")).searchParams.get("rpcids");
  } catch {
    return null;
  }
}

function readSourcePathFromUrl(url) {
  try {
    return new URL(String(url ?? "")).searchParams.get("source-path");
  } catch {
    return null;
  }
}

function classifyProgramRpcCapture(url, method) {
  const rpcId = readRpcIdFromUrl(url);
  if (rpcId && INTERESTING_PROGRAM_RPC_IDS.has(rpcId)) {
    return {
      rpcId,
      label: rpcId,
      sourcePath: readSourcePathFromUrl(url),
      method: String(method || "GET").toUpperCase(),
    };
  }
  if (/\/StreamGenerate/i.test(String(url || ""))) {
    return {
      rpcId: null,
      label: "StreamGenerate",
      sourcePath: readSourcePathFromUrl(url),
      method: String(method || "GET").toUpperCase(),
    };
  }
  return null;
}

const { collectSnapshot } = createProgramHandleSnapshotOwner({ outDir, extractHandleHintsFromText, textPreview });

function strongestHandle(hints) {
  const reversedAppPaths = [...(hints.appPaths || [])].reverse();
  const reversedConversationIds = [...(hints.conversationIds || [])].reverse();
  const reversedSharePaths = [...(hints.sharePaths || [])].reverse();
  return (
    reversedAppPaths.find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ??
    reversedConversationIds[0] ??
    reversedSharePaths[0] ??
    null
  );
}

function hasTransportHints(transportHints) {
  return Boolean(
    (transportHints?.invokeBaseUrls || []).length ||
      (transportHints?.musicWsUrls || []).length ||
      (transportHints?.videoInvokePaths || []).length,
  );
}

const {
  inferMimeTypeFromUrl,
  isBlobLikeUrl,
  isAudioLikeMimeType,
  isAudioLikeUrl,
  isVideoLikeUrl,
  pushUniqueMediaUrl,
  scorePlayerReadyTargetCandidate,
  summarizePlayerReadyTargetCandidate,
  pushPlayerReadyTargetCandidate,
  collectRpcBodyDownloadCandidates,
  collectPlayerReadyTargetCandidates,
} = createProgramHandleMediaOwner({ normalizeString });

const { mergeInvokeContract } = createInvokeContractOwner({ scorePlayerReadyTargetCandidate });

function inferInvokeUiState(operation, snapshot) {
  const bodyText = normalizeString(snapshot?.bodyText) ?? "";
  const controls = [
    ...(snapshot?.buttons || []).flatMap((entry) => [entry.text, entry.ariaLabel, entry.title]),
  ]
    .filter(Boolean)
    .map((value) => String(value).trim());
  if (operation === "music") {
    if (/Generating your music/i.test(bodyText)) {
      return "music_generating";
    }
    if (
      controls.some((value) => value.includes("下载音乐作品")) &&
      /0:\d{2}\s*\/\s*0:\d{2}/.test(bodyText)
    ) {
      return "music_player_ready";
    }
  }
  if (operation === "video") {
    if (/Generating your video/i.test(bodyText)) {
      return "video_generating";
    }
    if (controls.some((value) => value.includes("播放视频") || value.includes("下载视频"))) {
      return "video_player_ready";
    }
  }
  if (controls.some((value) => value.includes("不使用应用，再试一次"))) {
    return "retry_without_app_visible";
  }
  return null;
}

const { deriveProgramRpcInvokeCandidate, extractModelHintFromRpcText } = createProgramRpcOwner({ extractAppPath });

const {
  collectCanvasProxyContractTexts, deriveCanvasProxyWsInvokeCandidate,
  extractCanvasProxyWsUrlFromText, extractCanvasProxyTargetDomainFromText,
} = createProxyDiscoveryOwner({ extractAppPath, extractModelHintFromRpcText });

const {
  trimProgramRpcCaptureText, pushProgramRpcCapture, extractRequestCookieHeader,
  captureCookieHeaderFromContext, classifyHandlePairSurface,
} = createProgramCaptureOwner({ extractCanvasProxyWsUrlFromText, extractCanvasProxyTargetDomainFromText });

const { startNetworkCapture } = createProgramNetworkCaptureOwner({
  operation,
  extractHandleHintsFromText,
  classifyHandlePairSurface,
  extractHandlePairsFromText,
  readRpcIdFromUrl,
  extractTransportHintsFromUrl,
  extractCanvasProgramActionContractFromText,
  mergeHints,
  mergeHandlePairs,
  mergeTransportHints,
  mergeActionContract,
  classifyProgramRpcCapture,
  pushProgramRpcCapture,
  trimProgramRpcCaptureText,
  extractRequestCookieHeader,
  captureCookieHeaderFromContext,
  textPreview,
  isAudioLikeMimeType,
  isAudioLikeUrl,
  isVideoLikeUrl,
  pushUniqueMediaUrl,
});

const { buildCanvasProgramInvokeContract } = createInvokeAssemblyOwner({
  collectPlayerReadyTargetCandidates, deriveCanvasProxyWsInvokeCandidate,
  deriveProgramRpcInvokeCandidate, inferInvokeUiState,
});

await executeProgramHandleProbe({
  chromium, profileDir, executablePath, shareUrl, appUrl, operation, prompt,
  discoveryOnly, timeoutMs, outDir, collectSnapshot, startNetworkCapture,
  strongestHandle, hasTransportHints, toObjectKeyFromLocalPath, normalizeString,
  mergeInvokeContract, buildCanvasProgramInvokeContract,
});
