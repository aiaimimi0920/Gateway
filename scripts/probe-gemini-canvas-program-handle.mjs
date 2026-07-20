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

const APP_PATH_REGEX = /(?:https:\/\/gemini\.google\.com)?(\/app\/(?:[0-9a-f]{8,}|\d{13,}))/gi;
const CONVERSATION_ID_REGEX = /\bc_([0-9a-f]{8,})\b/gi;
const RESPONSE_ID_REGEX = /\br_([0-9a-f]{8,})\b/gi;
const SHARE_PATH_REGEX = /(\/share\/[0-9a-z]{8,})/gi;
const CONVERSATION_RESPONSE_PAIR_REGEX =
  /(?:\\?")?(c_[0-9a-f]{8,})(?:\\?")?\s*,\s*(?:\\?")?(r_[0-9a-f]{8,})(?:\\?")?/gi;
const PROGRAM_RPC_CAPTURE_LIMIT = 80;
const PROGRAM_RPC_TEXT_LIMIT = 40000;
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

function uniqueStrings(values) {
  return [...new Set(values.filter(Boolean).map((value) => String(value).trim()).filter(Boolean))];
}

function extractAppPathsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = APP_PATH_REGEX.exec(source)) !== null) {
    matches.push(match[1]);
  }
  return uniqueStrings(matches);
}

function extractConversationIdsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = CONVERSATION_ID_REGEX.exec(source)) !== null) {
    matches.push(`c_${match[1]}`);
  }
  return uniqueStrings(matches);
}

function extractResponseIdsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = RESPONSE_ID_REGEX.exec(source)) !== null) {
    matches.push(`r_${match[1]}`);
  }
  return uniqueStrings(matches);
}

function extractSharePathsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = SHARE_PATH_REGEX.exec(source)) !== null) {
    matches.push(match[1]);
  }
  return uniqueStrings(matches);
}

function extractHandleHintsFromText(text) {
  const appPaths = extractAppPathsFromText(text);
  const conversationIds = extractConversationIdsFromText(text);
  const responseIds = extractResponseIdsFromText(text);
  const sharePaths = extractSharePathsFromText(text);
  const derivedAppPaths = conversationIds.map((id) => `/app/${id.replace(/^c_/, "")}`);
  return {
    appPaths: uniqueStrings([...appPaths, ...derivedAppPaths]),
    conversationIds,
    responseIds,
    sharePaths,
  };
}

function sanitizeTransportHintUrl(rawUrl) {
  const normalized = String(rawUrl ?? "").trim();
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    parsed.search = "";
    parsed.hash = "";
    return parsed.toString();
  } catch {
    return normalized;
  }
}

function deriveInvokeBaseUrlFromRequestUrl(rawUrl) {
  const normalized = sanitizeTransportHintUrl(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    const modelsIndex = parsed.pathname.indexOf("/models/");
    if (modelsIndex <= 0) {
      return null;
    }
    return `${parsed.origin}${parsed.pathname.slice(0, modelsIndex)}`;
  } catch {
    return null;
  }
}

function deriveVideoInvokePathFromRequestUrl(rawUrl) {
  const normalized = sanitizeTransportHintUrl(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    return /:predictLongRunning$/i.test(parsed.pathname) ? parsed.pathname : null;
  } catch {
    return null;
  }
}

function deriveMusicWsUrlFromRequestUrl(rawUrl) {
  const normalized = sanitizeTransportHintUrl(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    return /BidiGenerateMusic/i.test(parsed.pathname) ? parsed.toString() : null;
  } catch {
    return null;
  }
}

function extractTransportHintsFromUrl(rawUrl) {
  return {
    invokeBaseUrl: deriveInvokeBaseUrlFromRequestUrl(rawUrl),
    musicWsUrl: deriveMusicWsUrlFromRequestUrl(rawUrl),
    videoInvokePath: deriveVideoInvokePathFromRequestUrl(rawUrl),
  };
}

function mergeTransportHints(target, incoming) {
  target.invokeBaseUrls = uniqueStrings([
    ...(target.invokeBaseUrls || []),
    ...(incoming.invokeBaseUrl ? [incoming.invokeBaseUrl] : []),
  ]);
  target.musicWsUrls = uniqueStrings([
    ...(target.musicWsUrls || []),
    ...(incoming.musicWsUrl ? [incoming.musicWsUrl] : []),
  ]);
  target.videoInvokePaths = uniqueStrings([
    ...(target.videoInvokePaths || []),
    ...(incoming.videoInvokePath ? [incoming.videoInvokePath] : []),
  ]);
}

function buildHandlePair(conversationId, responseId, source = {}) {
  const normalizedConversationId = normalizeHandleId(conversationId, "c_");
  const normalizedResponseId = normalizeHandleId(responseId, "r_");
  if (!normalizedConversationId || !normalizedResponseId) {
    return null;
  }
  const appPath = `/app/${normalizedConversationId.replace(/^c_/, "")}`;
  return {
    appPath,
    programUrl: `https://gemini.google.com${appPath}`,
    conversationId: normalizedConversationId,
    responseId: normalizedResponseId,
    sourceUrl: source.sourceUrl ?? null,
    sourceRpc: source.sourceRpc ?? null,
    sourceKind: source.sourceKind ?? null,
    sourceSurface: source.sourceSurface ?? null,
    sourceWsUrl: source.sourceWsUrl ?? null,
    sourceTargetDomain: source.sourceTargetDomain ?? null,
    ts: source.ts ?? null,
  };
}

function normalizeHandleId(value, prefix) {
  const text = String(value ?? "").trim();
  if (!text) {
    return null;
  }
  const normalized = text.startsWith(prefix) ? text : `${prefix}${text.replace(/^[_-]+/, "")}`;
  return new RegExp(`^${prefix}[0-9a-f]{8,}$`, "i").test(normalized) ? normalized : null;
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

function trimProgramRpcCaptureText(value, limit = PROGRAM_RPC_TEXT_LIMIT) {
  if (typeof value !== "string") {
    return value ?? null;
  }
  if (
    value.includes("Browser API Proxy Client") ||
    value.includes("Routing Google API requests via WebSocket") ||
    value.includes("System Logs Output")
  ) {
    return value;
  }
  if (value.length <= limit) {
    return value;
  }
  return `${value.slice(0, limit)}...[truncated]`;
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

function pushProgramRpcCapture(state, capture) {
  if (!state.rpcCaptures) {
    state.rpcCaptures = [];
  }
  state.rpcCaptures.push({
    t: Date.now(),
    ...capture,
  });
  if (state.rpcCaptures.length > PROGRAM_RPC_CAPTURE_LIMIT) {
    state.rpcCaptures.splice(0, state.rpcCaptures.length - PROGRAM_RPC_CAPTURE_LIMIT);
  }
}

function extractRequestCookieHeader(headers) {
  if (!headers || typeof headers !== "object") {
    return null;
  }
  const direct = normalizeString(headers.cookie) ?? normalizeString(headers.Cookie);
  return direct && direct.includes("=") ? direct : null;
}

async function captureCookieHeaderFromContext(page, requestUrl) {
  try {
    const target = new URL(requestUrl);
    const origin = `${target.protocol}//${target.host}`;
    const cookies = await page.context().cookies([origin, requestUrl]);
    const cookieHeader = cookies
      .map((cookie) => `${cookie.name}=${cookie.value}`)
      .join("; ")
      .trim();
    return cookieHeader && cookieHeader.includes("=") ? cookieHeader : null;
  } catch {
    return null;
  }
}

function extractHandlePairsFromText(text, source = {}) {
  const matches = [];
  const input = String(text ?? "");
  let match;
  while ((match = CONVERSATION_RESPONSE_PAIR_REGEX.exec(input)) !== null) {
    const pair = buildHandlePair(match[1], match[2], source);
    if (pair) {
      matches.push(pair);
    }
  }
  return dedupeHandlePairs(matches);
}

function mergeHints(target, incoming) {
  target.appPaths = uniqueStrings([...(target.appPaths || []), ...(incoming.appPaths || [])]);
  target.conversationIds = uniqueStrings([
    ...(target.conversationIds || []),
    ...(incoming.conversationIds || []),
  ]);
  target.responseIds = uniqueStrings([...(target.responseIds || []), ...(incoming.responseIds || [])]);
  target.sharePaths = uniqueStrings([...(target.sharePaths || []), ...(incoming.sharePaths || [])]);
}

function dedupeHandlePairs(pairs) {
  const map = new Map();
  for (const pair of pairs || []) {
    if (!pair?.conversationId || !pair?.responseId) {
      continue;
    }
    const key = `${pair.conversationId}|${pair.responseId}|${pair.appPath || ""}`;
    map.set(key, pair);
  }
  return [...map.values()];
}

function mergeHandlePairs(target, incoming) {
  const merged = dedupeHandlePairs([...(target || []), ...(incoming || [])]);
  target.length = 0;
  target.push(...merged);
}

function isCanvasProxyClientText(text) {
  const source = String(text || "");
  return (
    source.includes("Browser API Proxy Client") ||
    source.includes("Routing Google API requests via WebSocket") ||
    source.includes("Server WS Endpoint")
  );
}

function classifyHandlePairSurface(text) {
  if (!isCanvasProxyClientText(text)) {
    return {
      sourceSurface: null,
      sourceWsUrl: null,
      sourceTargetDomain: null,
    };
  }
  return {
    sourceSurface: "canvas_proxy_client",
    sourceWsUrl: extractCanvasProxyWsUrlFromText(text),
    sourceTargetDomain: extractCanvasProxyTargetDomainFromText(text),
  };
}

function extractAppPath(url) {
  if (!url) {
    return null;
  }
  const matches = extractAppPathsFromText(url);
  return matches[0] ?? null;
}

async function collectSnapshot(page, label) {
  const snapshot = await page.evaluate((currentLabel) => {
    const bodyText = document.body?.innerText ?? "";
    const buttons = Array.from(
      document.querySelectorAll('button,[role="button"],a[role="button"]'),
    )
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 200);
    const anchors = Array.from(document.querySelectorAll("a[href]"))
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        href: node.href,
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
        target: node.getAttribute("target"),
        download: node.getAttribute("download"),
      }))
      .filter((entry) => entry.href)
      .slice(0, 200);
    const mediaNodes = [
      ...Array.from(document.querySelectorAll("audio")).map((node, index) => ({
        kind: "audio",
        index,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
      })),
      ...Array.from(document.querySelectorAll("video")).map((node, index) => ({
        kind: "video",
        index,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
        width: node.videoWidth,
        height: node.videoHeight,
        poster: node.getAttribute("poster"),
      })),
      ...Array.from(document.querySelectorAll("img")).map((node, index) => ({
        kind: "image",
        index,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        alt: node.getAttribute("alt"),
        width: node.naturalWidth,
        height: node.naturalHeight,
      })),
    ]
      .filter((entry) => entry.src || entry.currentSrc)
      .slice(0, 120);
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
    let historyState;
    try {
      historyState = JSON.stringify(history.state ?? null);
    } catch {
      historyState = null;
    }
    return {
      label: currentLabel,
      url: location.href,
      title: document.title,
      bodyText,
      historyState,
      buttons,
      anchors,
      mediaNodes,
      textboxes,
    };
  }, label);

  const hints = extractHandleHintsFromText(
    [
      snapshot.url,
      snapshot.historyState,
      snapshot.bodyText,
      ...(snapshot.anchors || []).flatMap((anchor) => [anchor.href, anchor.text, anchor.ariaLabel, anchor.title]),
    ]
      .filter(Boolean)
      .join("\n"),
  );

  const enriched = {
    ...snapshot,
    bodyText: textPreview(snapshot.bodyText, 12000),
    historyState: textPreview(snapshot.historyState, 4000),
    handleHints: hints,
  };

  writeFileSync(path.join(outDir, `${label}.json`), `${JSON.stringify(enriched, null, 2)}\n`, "utf8");
  await page.screenshot({
    path: path.join(outDir, `${label}.png`),
    fullPage: true,
  });
  return enriched;
}

function startNetworkCapture(page, existingState = null) {
  const state = existingState ?? {
    requests: [],
    responses: [],
    audioUrls: [],
    videoUrls: [],
    handlePairs: [],
    rpcCaptures: [],
    handleHints: {
      appPaths: [],
      conversationIds: [],
      responseIds: [],
      sharePaths: [],
    },
    transportHints: {
      invokeBaseUrls: [],
      musicWsUrls: [],
      videoInvokePaths: [],
    },
    actionContract: {
      canvasProgramAction: null,
      canvasProgramActionInput: null,
    },
    invokeContract: null,
  };

  const requestFilter = (url) =>
    /StreamGenerate|batchexecute|assistant\.lamda\.BardFrontendService|predictLongRunning|BidiGenerateMusic|googleapis\.com|googleusercontent\.com|contribution\.usercontent\.google\.com|googlevideo\.com|gvt1\.com|\/share\/|\/app(?:\/|$)/i.test(
      String(url || ""),
    );

  const onRequest = (request) => {
    const url = request.url();
    if (!requestFilter(url)) {
      return;
    }
    const postData = request.postData() ?? "";
    const rawHeaders = request.headers() || {};
    const requestCookieHeader = extractRequestCookieHeader(rawHeaders);
    const hints = extractHandleHintsFromText(`${url}\n${postData}`);
    const pairSurface = classifyHandlePairSurface(`${url}\n${postData}`);
    const handlePairs = extractHandlePairsFromText(`${url}\n${postData}`, {
      sourceKind: "request",
      sourceUrl: url,
      sourceRpc: readRpcIdFromUrl(url),
      ...pairSurface,
      ts: new Date().toISOString(),
    });
    const transportHints = extractTransportHintsFromUrl(url);
    const actionContract = extractCanvasProgramActionContractFromText(`${url}\n${postData}`);
    mergeHints(state.handleHints, hints);
    mergeHandlePairs(state.handlePairs, handlePairs);
    mergeTransportHints(state.transportHints, transportHints);
    mergeActionContract(state.actionContract, actionContract);
    const rpcCapture = classifyProgramRpcCapture(url, request.method());
    if (rpcCapture) {
      pushProgramRpcCapture(state, {
        type: "request",
        ...rpcCapture,
        url,
        cookieHeader: requestCookieHeader,
        bodyText: trimProgramRpcCaptureText(postData),
      });
    }
    if (
      requestCookieHeader &&
      String(rpcCapture?.label || "").toLowerCase() === "streamgenerate"
    ) {
      state.cookieHeader = requestCookieHeader;
    } else if (String(rpcCapture?.label || "").toLowerCase() === "streamgenerate") {
      void captureCookieHeaderFromContext(page, url).then((cookieHeader) => {
        if (cookieHeader) {
          state.cookieHeader = cookieHeader;
        }
      });
    }
    state.requests.push({
      ts: new Date().toISOString(),
      method: request.method(),
      url,
      resourceType: request.resourceType(),
      postDataPreview: textPreview(postData, 1600),
      handleHints: hints,
      handlePairs,
      transportHints,
    });
  };

  const onResponse = async (response) => {
    const url = response.url();
    if (!requestFilter(url)) {
      return;
    }
    const request = response.request();
    const contentType = response.headers()["content-type"] ?? "";
    let bodyText = "";
    try {
      bodyText = await response.text();
    } catch {
      bodyText = "";
    }
    const hints = extractHandleHintsFromText(`${url}\n${bodyText}`);
    const pairSurface = classifyHandlePairSurface(`${url}\n${bodyText}`);
    const handlePairs = extractHandlePairsFromText(`${url}\n${bodyText}`, {
      sourceKind: "response",
      sourceUrl: url,
      sourceRpc: readRpcIdFromUrl(url),
      ...pairSurface,
      ts: new Date().toISOString(),
    });
    const transportHints = extractTransportHintsFromUrl(url);
    mergeHints(state.handleHints, hints);
    mergeHandlePairs(state.handlePairs, handlePairs);
    mergeTransportHints(state.transportHints, transportHints);
    const rpcCapture = classifyProgramRpcCapture(url, request.method());
    if (rpcCapture) {
      pushProgramRpcCapture(state, {
        type: "response",
        ...rpcCapture,
        url,
        status: response.status(),
        contentType,
        bodyText: trimProgramRpcCaptureText(bodyText),
      });
    }
    state.responses.push({
      ts: new Date().toISOString(),
      status: response.status(),
      url,
      contentType: response.headers()["content-type"] ?? null,
      bodyPreview: textPreview(bodyText, 4000),
      ...(rpcCapture
        ? {
            bodyText: trimProgramRpcCaptureText(bodyText),
          }
        : {}),
      handleHints: hints,
      handlePairs,
      transportHints,
    });
    if (operation === "music" && (isAudioLikeMimeType(contentType) || isAudioLikeUrl(url))) {
      pushUniqueMediaUrl(state.audioUrls, {
        url,
        mimeType: contentType.split(";")[0] || null,
      });
    }
    if ((operation === "music" || operation === "video") && isVideoLikeUrl(url)) {
      pushUniqueMediaUrl(state.videoUrls, {
        url,
        mimeType: contentType.split(";")[0] || null,
      });
    }
    mergeActionContract(
      state.actionContract,
      extractCanvasProgramActionContractFromText(`${url}\n${bodyText}`),
    );
  };

  const onWebSocket = (websocket) => {
    const url = websocket.url();
    if (!requestFilter(url)) {
      return;
    }
    const transportHints = extractTransportHintsFromUrl(url);
    mergeTransportHints(state.transportHints, transportHints);
    state.requests.push({
      ts: new Date().toISOString(),
      method: "WEBSOCKET",
      url,
      resourceType: "websocket",
      postDataPreview: null,
      handleHints: null,
      handlePairs: [],
      transportHints,
    });
  };

  page.on("request", onRequest);
  page.on("response", onResponse);
  page.on("websocket", onWebSocket);

  return {
    state,
    stop() {
      page.off("request", onRequest);
      page.off("response", onResponse);
      page.off("websocket", onWebSocket);
    },
  };
}

async function waitForShareSurface(page, deadlineMs) {
  const deadline = Date.now() + deadlineMs;
  while (Date.now() < deadline) {
    const bodyText = await page.evaluate(() => (document.body?.innerText ?? "").slice(0, 4000));
    if (/继续|试用 Gemini Canvas|在新窗口中打开|不要公开个人信息/i.test(bodyText)) {
      return true;
    }
    await page.waitForTimeout(1200);
  }
  return false;
}

async function hasPromptTextbox(page) {
  try {
    const textbox = page
      .locator(
        '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
      )
      .first();
    return (await textbox.count()) > 0;
  } catch {
    return false;
  }
}

async function clickFirstVisible(candidates) {
  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) > 0) {
        await candidate.waitFor({ state: "visible", timeout: 4000 });
        await candidate.click({ timeout: 12000, force: true });
        return true;
      }
    } catch {
      // try next selector
    }
  }
  return false;
}

async function tryFollowShareEntryPoint(page) {
  const candidates = [
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /试用 Gemini Canvas|Try Gemini Canvas/i }).first(),
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /继续|Continue/i }).first(),
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /制作图片|Create image|Create images|Make image/i }).first(),
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /在新窗口中打开|Open in new window/i }).first(),
  ];

  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) === 0) {
        continue;
      }
      await candidate.waitFor({ state: "visible", timeout: 5000 });
      const popupPromise = page.waitForEvent("popup", { timeout: 6000 }).catch(() => null);
      await candidate.click({ timeout: 12000, force: true });
      const popup = await popupPromise;
      if (popup) {
        await popup.waitForLoadState("domcontentloaded", { timeout: 20000 }).catch(() => undefined);
        await popup.waitForTimeout(3000);
        return { kind: "popup", page: popup };
      }
      await page.waitForTimeout(3000);
      return { kind: "same_page", page };
    } catch {
      // try next candidate
    }
  }

  return { kind: "none", page };
}

async function clickNewChat(page) {
  const candidates = [
    page.getByRole("button", { name: /发起新对话|New chat/i }).first(),
    page.locator('button[aria-label*="发起新对话"], button[aria-label*="New chat"]').first(),
    page.getByRole("link", { name: /发起新对话|New chat/i }).first(),
  ];
  const clicked = await clickFirstVisible(candidates);
  if (clicked) {
    await page.waitForTimeout(1200);
  }
  return clicked;
}

async function clickOperationMode(page, selectedOperation) {
  let matcher = null;
  switch (selectedOperation) {
    case "image":
      matcher = /制作图片|Create image|Create images|Make image/i;
      break;
    case "music":
      matcher = /创作音乐|制作音乐|Create music/i;
      break;
    case "video":
      matcher = /创作视频|制作视频|Create video/i;
      break;
    default:
      return false;
  }
  const candidates = [
    page.locator("a,button,[role=\"button\"]").filter({ hasText: matcher }).first(),
    page.getByRole("button", { name: matcher }).first(),
  ];
  const clicked = await clickFirstVisible(candidates);
  if (clicked) {
    await page.waitForTimeout(1500);
  }
  return clicked;
}

async function clickMediaActionButton(page, operation, action, timeoutMs) {
  let matcher = null;
  if (operation === "music" && action === "download") {
    matcher = /下载音乐作品|Download music/i;
  } else if (operation === "music" && action === "play") {
    matcher = /播放视频|播放|Play/i;
  } else if (operation === "video" && action === "download") {
    matcher = /下载视频|Download video/i;
  } else if (operation === "video" && action === "play") {
    matcher = /播放视频|Play video/i;
  } else {
    return false;
  }
  const candidates = [
    page.getByRole("button", { name: matcher }).first(),
    page.locator("button,[role=\"button\"],a").filter({ hasText: matcher }).first(),
  ];
  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) === 0) {
        continue;
      }
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 5_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000), force: true });
      await page.waitForTimeout(1500);
      return true;
    } catch {
      // try next
    }
  }
  return false;
}

async function trySelectMusicStyleCard(page, timeoutMs, attemptIndex = 0) {
  const selection = await page.evaluate((requestedIndex) => {
    const isVisible = (node) => {
      if (!(node instanceof HTMLElement)) {
        return false;
      }
      const style = window.getComputedStyle(node);
      if (style.display === "none" || style.visibility === "hidden") {
        return false;
      }
      const rect = node.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0;
    };
    const isClickable = (node) => {
      if (!(node instanceof HTMLElement)) {
        return false;
      }
      const style = window.getComputedStyle(node);
      return (
        node.matches('button,[role="button"],a,[tabindex]') ||
        typeof node.onclick === "function" ||
        style.cursor === "pointer"
      );
    };
    const clickNode = (node) => {
      node.scrollIntoView({ block: "center", inline: "center" });
      const dispatchClick = (targetNode) => {
        targetNode.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true }));
        targetNode.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
        targetNode.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
        targetNode.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      };
      try {
        if (typeof node.click === "function") {
          node.click();
        } else {
          dispatchClick(node);
        }
        dispatchClick(node);
      } catch {
        dispatchClick(node);
      }
    };
    const findClickableAncestor = (node) => {
      let clickableNode = node;
      let depth = 0;
      while (clickableNode && depth < 8) {
        if (isClickable(clickableNode) && isVisible(clickableNode)) {
          break;
        }
        clickableNode = clickableNode.parentElement;
        depth += 1;
      }
      return clickableNode instanceof HTMLElement && isVisible(clickableNode)
        ? clickableNode
        : node;
    };

    const candidates = Array.from(document.querySelectorAll("img"))
      .filter((node) => isVisible(node))
      .map((node) => ({
        node,
        alt: (node.alt || "").trim(),
        width: node.naturalWidth || 0,
        height: node.naturalHeight || 0,
      }))
      .filter(
        (entry) =>
          entry.alt &&
          !/个人资料照片|profile/i.test(entry.alt) &&
          entry.width >= 300 &&
          entry.height >= 300,
      );
    if (candidates.length === 0) {
      return {
        clicked: false,
        reason: "style_candidate_missing",
        styleCount: 0,
      };
    }

    const selectedIndex = Math.min(Math.max(Number(requestedIndex || 0), 0), candidates.length - 1);
    const selectedCandidate = candidates[selectedIndex];
    const clickableNode = findClickableAncestor(selectedCandidate.node);
    clickNode(clickableNode);
    return {
      clicked: true,
      reason: "style_clicked",
      styleCount: candidates.length,
      selectedIndex,
      selectedAlt: selectedCandidate.alt,
      selectedSrc: selectedCandidate.node.currentSrc || selectedCandidate.node.src || null,
      clickedTag: clickableNode.tagName || null,
      clickedRole: clickableNode.getAttribute?.("role") || null,
      clickedAria: clickableNode.getAttribute?.("aria-label") || null,
      clickedText: (clickableNode.innerText || "").trim().slice(0, 200),
    };
  }, attemptIndex);

  await page.waitForTimeout(1500);
  return selection;
}

async function submitPrompt(page, value) {
  const textbox = page
    .locator(
      '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
    )
    .first();
  await textbox.waitFor({ state: "visible", timeout: 30000 });
  await textbox.focus();
  let populated = false;
  try {
    await textbox.evaluate((node, promptText) => {
      const textValue = String(promptText ?? "");
      const fireInput = () =>
        node.dispatchEvent(
          new InputEvent("input", {
            bubbles: true,
            cancelable: true,
            data: textValue,
            inputType: "insertText",
          }),
        );
      if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) {
        node.value = textValue;
        fireInput();
        node.dispatchEvent(new Event("change", { bubbles: true }));
        return true;
      }
      if (node instanceof HTMLElement && node.isContentEditable) {
        node.textContent = textValue;
        fireInput();
        return true;
      }
      return false;
    }, value);
    populated = true;
  } catch {
    populated = false;
  }

  if (!populated) {
    await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
    await page.keyboard.press("Backspace").catch(() => undefined);
    await page.keyboard.type(value, { delay: 12 });
  }
  await page.waitForTimeout(300);

  const sendCandidates = [
    page.getByRole("button", { name: /发送|Send/i }).first(),
    page
      .locator(
        'button[aria-label*="Send"], button[aria-label*="发送"], button[title*="Send"], button[title*="发送"]',
      )
      .first(),
  ];
  const clicked = await clickFirstVisible(sendCandidates);
  if (!clicked) {
    await page.keyboard.press("Enter").catch(() => undefined);
  }
}

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

function mergeActionContract(target, incoming) {
  if (!target || !incoming) {
    return;
  }
  if (!target.canvasProgramAction && incoming.canvasProgramAction) {
    target.canvasProgramAction = incoming.canvasProgramAction;
  }
  if (!target.canvasProgramActionInput && incoming.canvasProgramActionInput) {
    target.canvasProgramActionInput = incoming.canvasProgramActionInput;
  }
}

function inferMimeTypeFromUrl(url, fallback) {
  const lowered = String(url || "").toLowerCase();
  if (lowered.includes(".png") || lowered.includes("=w0") || lowered.includes("gg-dl")) {
    return "image/png";
  }
  if (lowered.includes(".jpg") || lowered.includes(".jpeg")) {
    return "image/jpeg";
  }
  if (lowered.includes(".webp")) {
    return "image/webp";
  }
  if (lowered.includes(".mp4") || lowered.includes("filename=video.mp4")) {
    return "video/mp4";
  }
  if (lowered.includes(".webm")) {
    return "video/webm";
  }
  if (lowered.includes(".wav")) {
    return "audio/wav";
  }
  if (lowered.includes(".mp3")) {
    return "audio/mpeg";
  }
  if (lowered.includes(".ogg")) {
    return "audio/ogg";
  }
  return fallback;
}

function isBlobLikeUrl(url) {
  return /^blob:/i.test(String(url || "").trim());
}

function isAudioLikeMimeType(mimeType) {
  return /^audio\//i.test(String(mimeType || "").trim());
}

function isAudioLikeUrl(url) {
  return /googlevideo|gvt1|\.wav(\?|$)|\.mp3(\?|$)|\.ogg(\?|$)|filename=.*\.(wav|mp3|ogg)(\b|$)/i.test(
    String(url || ""),
  );
}

function isVideoLikeUrl(url) {
  return /blob:|contribution\.usercontent\.google\.com|googlevideo|gvt1|\.mp4(\?|$)|\.webm(\?|$)|filename=.*\.(mp4|webm)(\b|$)/i.test(
    String(url || ""),
  );
}

function pushUniqueMediaUrl(store, candidate) {
  if (!candidate?.url) {
    return;
  }
  if (store.some((entry) => entry.url === candidate.url)) {
    return;
  }
  store.push(candidate);
}

function scorePlayerReadyTargetCandidate(operation, candidate) {
  const url = normalizeString(candidate?.url);
  if (!url) {
    return -1;
  }
  const source = String(candidate?.source || "");
  const mimeType = String(candidate?.mimeType || "");
  let score = 0;
  if (/^https?:/i.test(url)) {
    score += 160;
  } else if (isBlobLikeUrl(url)) {
    score += 90;
  }
  if (/network/i.test(source)) {
    score += 120;
  } else if (/anchor/i.test(source)) {
    score += 100;
  } else if (/media_node/i.test(source)) {
    score += 80;
  }
  if (/download/i.test(source)) {
    score += 24;
  }
  if (
    /googlevideo\.com|gvt1\.com|contribution\.usercontent\.google\.com|googleusercontent\.com/i.test(url)
  ) {
    score += 45;
  }
  if (operation === "music") {
    if (/^audio\//i.test(mimeType) || isAudioLikeUrl(url)) {
      score += 70;
    } else if (/^video\//i.test(mimeType) || isVideoLikeUrl(url)) {
      score += 48;
    }
  } else if (operation === "video") {
    if (/^video\//i.test(mimeType) || isVideoLikeUrl(url)) {
      score += 70;
    }
  }
  return score;
}

function summarizePlayerReadyTargetCandidate(operation, candidate) {
  const url = normalizeString(candidate?.url);
  if (!url) {
    return null;
  }
  return {
    url,
    source: normalizeString(candidate?.source) ?? null,
    mimeType: normalizeString(candidate?.mimeType) ?? inferMimeTypeFromUrl(url, null),
    kind: normalizeString(candidate?.kind) ?? (operation === "music" ? "audio" : operation),
    download: Boolean(candidate?.download),
    score: scorePlayerReadyTargetCandidate(operation, candidate),
  };
}

function pushPlayerReadyTargetCandidate(store, operation, candidate) {
  const summarized = summarizePlayerReadyTargetCandidate(operation, candidate);
  if (!summarized?.url) {
    return;
  }
  const existingIndex = store.findIndex((entry) => entry.url === summarized.url);
  if (existingIndex === -1) {
    store.push(summarized);
    return;
  }
  if ((store[existingIndex]?.score ?? -1) <= summarized.score) {
    store[existingIndex] = summarized;
  }
}

function collectRpcBodyDownloadCandidates(operation, captureState) {
  const candidates = [];
  const rpcBodies = [...(captureState?.rpcCaptures || [])]
    .reverse()
    .filter(
      (capture) =>
        capture?.type === "response"
        && (
          normalizeString(capture?.label) === "StreamGenerate"
          || /\/StreamGenerate/i.test(String(capture?.url || ""))
        ),
    )
    .map((capture) => String(capture?.bodyText || ""))
    .filter(Boolean);
  for (const bodyText of rpcBodies) {
    const rawMatches =
      bodyText.match(/https:\/\/contribution\.usercontent\.google\.com\/download\?[^"]+/g) || [];
    for (const rawMatch of rawMatches) {
      const decodedUrl = String(rawMatch)
        .replace(/\\u003d/g, "=")
        .replace(/\\u0026/g, "&");
      const inferredMimeType = inferMimeTypeFromUrl(decodedUrl, null);
      const inferredKind =
        /^audio\//i.test(inferredMimeType) || isAudioLikeUrl(decodedUrl)
          ? "audio"
          : /^video\//i.test(inferredMimeType) || isVideoLikeUrl(decodedUrl)
            ? "video"
            : /^text\//i.test(inferredMimeType)
              ? "text"
              : null;
      if (
        !inferredKind
        || (operation === "music" && inferredKind !== "audio" && inferredKind !== "video")
        || (operation === "video" && inferredKind !== "video")
      ) {
        continue;
      }
      candidates.push({
        url: decodedUrl,
        mimeType: inferredMimeType,
        source: "stream_generate_response_body",
        kind: inferredKind,
        download: true,
      });
    }
  }
  return candidates;
}

function collectPlayerReadyTargetCandidates(operation, snapshot, captureState) {
  const candidates = [];
  if (operation === "music") {
    for (const entry of [...(captureState?.audioUrls || [])].reverse()) {
      pushPlayerReadyTargetCandidate(candidates, operation, {
        url: entry?.url,
        mimeType: entry?.mimeType || inferMimeTypeFromUrl(entry?.url, "audio/wav"),
        source: "network_audio_response",
        kind: "audio",
      });
    }
    for (const entry of [...(captureState?.videoUrls || [])].reverse()) {
      pushPlayerReadyTargetCandidate(candidates, operation, {
        url: entry?.url,
        mimeType: entry?.mimeType || inferMimeTypeFromUrl(entry?.url, "video/mp4"),
        source: "network_video_response",
        kind: "video",
      });
    }
    for (const entry of collectRpcBodyDownloadCandidates(operation, captureState)) {
      pushPlayerReadyTargetCandidate(candidates, operation, entry);
    }
  } else if (operation === "video") {
    for (const entry of [...(captureState?.videoUrls || [])].reverse()) {
      pushPlayerReadyTargetCandidate(candidates, operation, {
        url: entry?.url,
        mimeType: entry?.mimeType || inferMimeTypeFromUrl(entry?.url, "video/mp4"),
        source: "network_video_response",
        kind: "video",
      });
    }
    for (const entry of collectRpcBodyDownloadCandidates(operation, captureState)) {
      pushPlayerReadyTargetCandidate(candidates, operation, entry);
    }
  }

  for (const anchor of snapshot?.anchors || []) {
    const href = normalizeString(anchor?.href);
    if (!href) {
      continue;
    }
    const download = normalizeString(anchor?.download);
    const label = [anchor?.text, anchor?.ariaLabel, anchor?.title].filter(Boolean).join("\n");
    if (operation === "music") {
      if (!download && !isAudioLikeUrl(href) && !isVideoLikeUrl(href) && !/下载音乐作品|Download music|播放|Play/i.test(label)) {
        continue;
      }
      pushPlayerReadyTargetCandidate(candidates, operation, {
        url: href,
        mimeType: inferMimeTypeFromUrl(href, isAudioLikeUrl(href) ? "audio/wav" : "video/mp4"),
        source: download ? "anchor_download_href" : "anchor_media_href",
        kind: isAudioLikeUrl(href) ? "audio" : "video",
        download: Boolean(download),
      });
      continue;
    }
    if (operation === "video") {
      if (!download && !isVideoLikeUrl(href) && !/下载视频|Download video|播放视频|Play video/i.test(label)) {
        continue;
      }
      pushPlayerReadyTargetCandidate(candidates, operation, {
        url: href,
        mimeType: inferMimeTypeFromUrl(href, "video/mp4"),
        source: download ? "anchor_download_href" : "anchor_media_href",
        kind: "video",
        download: Boolean(download),
      });
    }
  }

  for (const node of snapshot?.mediaNodes || []) {
    const url = normalizeString(node?.currentSrc) ?? normalizeString(node?.src);
    if (!url) {
      continue;
    }
    if (operation === "music") {
      if (node.kind === "audio" && (isAudioLikeUrl(url) || isBlobLikeUrl(url))) {
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url,
          mimeType: inferMimeTypeFromUrl(url, "audio/wav"),
          source: "media_node_current_src",
          kind: "audio",
        });
      } else if (node.kind === "video" && (isVideoLikeUrl(url) || isBlobLikeUrl(url))) {
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url,
          mimeType: inferMimeTypeFromUrl(url, "video/mp4"),
          source: "media_node_current_src",
          kind: "video",
        });
      }
      continue;
    }
    if (operation === "video" && node.kind === "video" && (isVideoLikeUrl(url) || isBlobLikeUrl(url))) {
      pushPlayerReadyTargetCandidate(candidates, operation, {
        url,
        mimeType: inferMimeTypeFromUrl(url, "video/mp4"),
        source: "media_node_current_src",
        kind: "video",
      });
    }
  }

  return candidates.sort((left, right) => (right.score ?? 0) - (left.score ?? 0));
}

function mergeInvokeContract(target, incoming) {
  if (!incoming) {
    return target;
  }
  if (!target) {
    return { ...incoming };
  }
  const rankUiState = (value) => {
    switch (value) {
      case "music_player_ready":
      case "video_player_ready":
        return 3;
      case "retry_without_app_visible":
        return 2;
      case "music_generating":
      case "video_generating":
        return 1;
      default:
        return 0;
    }
  };
  const scoreContractTarget = (contract) => {
    if (!contract?.target) {
      return -1;
    }
    return scorePlayerReadyTargetCandidate(contract.operation ?? incoming.operation ?? target?.operation, {
      url: contract.target,
      source: contract.targetSource,
      mimeType: contract.targetMimeType,
      kind: contract.operation === "music" ? "audio" : contract.operation,
      download: contract.targetSource ? /download/i.test(String(contract.targetSource)) : false,
    });
  };
  const mergeTargetCandidates = (left, right) => {
    const merged = [];
    for (const candidate of [...(left || []), ...(right || [])]) {
      if (!candidate?.url) {
        continue;
      }
      const existingIndex = merged.findIndex((entry) => entry.url === candidate.url);
      if (existingIndex === -1) {
        merged.push(candidate);
      } else if ((merged[existingIndex]?.score ?? -1) <= (candidate?.score ?? -1)) {
        merged[existingIndex] = candidate;
      }
    }
    return merged.slice(0, 12);
  };
  const useIncomingTarget = scoreContractTarget(incoming) > scoreContractTarget(target);
  const rankLane = (contract) => {
    const transportKind = normalizeString(contract?.transportKind);
    const requestEnvelopeKind = normalizeString(contract?.requestEnvelopeKind);
    if (
      transportKind === "program_music_streamgenerate_candidate" ||
      transportKind === "program_video_streamgenerate_candidate"
    ) {
      return 50;
    }
    if (requestEnvelopeKind === "page_stream_generate_form") {
      return 40;
    }
    if (
      transportKind === "program_music_batchexecute_candidate" ||
      transportKind === "program_video_batchexecute_candidate"
    ) {
      return 30;
    }
    if (transportKind === "canvas_program_ws_candidate") {
      return 10;
    }
    if (requestEnvelopeKind === "canvas_proxy_request") {
      return 5;
    }
    return 0;
  };
  const useIncomingLane =
    rankLane(incoming) > rankLane(target) ||
    (rankLane(incoming) === rankLane(target) && useIncomingTarget);
  const primaryLane = useIncomingLane ? incoming : target;
  const secondaryLane = useIncomingLane ? target : incoming;
  return {
    operation: target.operation ?? incoming.operation ?? null,
    transportKind: primaryLane.transportKind ?? secondaryLane.transportKind ?? null,
    wsUrl: primaryLane.wsUrl ?? secondaryLane.wsUrl ?? null,
    apiStyle: primaryLane.apiStyle ?? secondaryLane.apiStyle ?? null,
    requestPath: primaryLane.requestPath ?? secondaryLane.requestPath ?? null,
    requestEnvelopeKind:
      primaryLane.requestEnvelopeKind ?? secondaryLane.requestEnvelopeKind ?? null,
    requestUrl: primaryLane.requestUrl ?? secondaryLane.requestUrl ?? null,
    requestBody: primaryLane.requestBody ?? secondaryLane.requestBody ?? null,
    requestRpcId: primaryLane.requestRpcId ?? secondaryLane.requestRpcId ?? null,
    responseRpcId:
      primaryLane.responseRpcId ?? secondaryLane.responseRpcId ?? null,
    sourcePath: primaryLane.sourcePath ?? secondaryLane.sourcePath ?? null,
    modelHint: primaryLane.modelHint ?? secondaryLane.modelHint ?? null,
    target: useIncomingTarget ? incoming.target ?? target.target ?? null : target.target ?? incoming.target ?? null,
    targetSource: useIncomingTarget
      ? incoming.targetSource ?? target.targetSource ?? null
      : target.targetSource ?? incoming.targetSource ?? null,
    targetMimeType: useIncomingTarget
      ? incoming.targetMimeType ?? target.targetMimeType ?? null
      : target.targetMimeType ?? incoming.targetMimeType ?? null,
    targetCandidates: mergeTargetCandidates(target.targetCandidates, incoming.targetCandidates),
    actionName: target.actionName ?? incoming.actionName ?? null,
    actionInput: target.actionInput ?? incoming.actionInput ?? null,
    prompt: target.prompt ?? incoming.prompt ?? null,
    durationSeconds: target.durationSeconds ?? incoming.durationSeconds ?? null,
    aspectRatio: target.aspectRatio ?? incoming.aspectRatio ?? null,
    uiState:
      rankUiState(incoming.uiState) > rankUiState(target.uiState)
        ? incoming.uiState ?? null
        : target.uiState ?? incoming.uiState ?? null,
  };
}

function extractCanvasProgramActionContractFromText(rawText) {
  const text = normalizeString(rawText);
  if (!text) {
    return {
      canvasProgramAction: null,
      canvasProgramActionInput: null,
    };
  }
  const actionMatch = text.match(/"action"\s*:\s*"([^"\r\n]+)"/i);
  let actionInput = null;
  const actionInputMarker = '"action_input"';
  const actionInputIndex = text.indexOf(actionInputMarker);
  if (actionInputIndex >= 0) {
    const afterMarker = text.slice(actionInputIndex + actionInputMarker.length);
    const colonIndex = afterMarker.indexOf(":");
    if (colonIndex >= 0) {
      const line = afterMarker
        .slice(colonIndex + 1)
        .split(/\r?\n/, 1)[0]
        .trim()
        .replace(/,$/, "")
        .trim();
      if (line) {
        const normalizedLine =
          line.startsWith('"') && line.endsWith('"') && line.length >= 2
            ? line.slice(1, -1)
            : line;
        actionInput = normalizedLine.trim() || null;
      }
    }
  }
  return {
    canvasProgramAction: actionMatch?.[1]?.trim() || null,
    canvasProgramActionInput: actionInput,
  };
}

function extractQuotedScalar(rawText, fieldNames) {
  const text = normalizeString(rawText)
    ?.replace(/\\"/g, "\"")
    ?.replace(/\\'/g, "'");
  if (!text) {
    return null;
  }
  for (const fieldName of fieldNames) {
    const regex = new RegExp(`[\"']${fieldName}[\"']\\s*:\\s*[\"']([^\"'\\r\\n]+)[\"']`, "i");
    const match = text.match(regex);
    if (match?.[1]?.trim()) {
      return match[1].trim();
    }
  }
  return null;
}

function extractNumberScalar(rawText, fieldNames) {
  const text = normalizeString(rawText);
  if (!text) {
    return null;
  }
  for (const fieldName of fieldNames) {
    const regex = new RegExp(`[\"']${fieldName}[\"']\\s*:\\s*(\\d+(?:\\.\\d+)?)`, "i");
    const match = text.match(regex);
    if (match?.[1]) {
      const parsed = Number(match[1]);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return null;
}

function extractDurationSecondsFromBodyText(rawText) {
  const text = normalizeString(rawText);
  if (!text) {
    return null;
  }
  const match = text.match(/(\d+):(\d{2})\s*\/\s*(\d+):(\d{2})/);
  if (!match) {
    return null;
  }
  const minutes = Number(match[3]);
  const seconds = Number(match[4]);
  if (!Number.isFinite(minutes) || !Number.isFinite(seconds)) {
    return null;
  }
  return minutes * 60 + seconds;
}

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

function bodyIndicatesMusicAcceptedProgress(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  const normalized = text.toLowerCase();
  return (
    (text.includes("music_generation") && text.includes("action_input")) ||
    normalized.includes("track details") ||
    normalized.includes("i've put together a 30-second electronic cue") ||
    normalized.includes("i’ve put together a 30-second electronic cue") ||
    normalized.includes("electronic cue for you")
  );
}

function bodyIndicatesVideoAcceptedProgress(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  const normalized = text.toLowerCase();
  return (
    text.includes("video_placeholder") ||
    normalized.includes("getting a lot of requests right now") ||
    normalized.includes("please try again later")
  );
}

function invokeContractIsProxyOnlyCandidate(invokeContract) {
  return (
    invokeContract?.transportKind === "canvas_program_ws_candidate" &&
    invokeContract?.requestEnvelopeKind === "canvas_proxy_request"
  );
}

function invokeContractIndicatesConcreteProgress(operation, invokeContract, snapshot) {
  if (!invokeContract) {
    return false;
  }
  if (
    invokeContract.uiState === "music_player_ready" ||
    invokeContract.uiState === "video_player_ready"
  ) {
    return true;
  }
  if (
    invokeContract.target &&
    !invokeContractIsProxyOnlyCandidate(invokeContract)
  ) {
    return true;
  }
  if (operation === "music") {
    return (
      invokeContract.actionName === "music_generation" ||
      bodyIndicatesMusicAcceptedProgress(snapshot?.bodyText)
    );
  }
  if (operation === "video") {
    return bodyIndicatesVideoAcceptedProgress(snapshot?.bodyText);
  }
  return false;
}

function rpcCaptureSourcePathMatches(capture, appPath) {
  const sourcePath = normalizeString(capture?.sourcePath);
  const normalizedAppPath = normalizeString(appPath);
  if (!sourcePath || !normalizedAppPath) {
    return false;
  }
  return sourcePath === normalizedAppPath;
}

function rpcCaptureMatchesIds(capture, rpcIds) {
  const rpcId = normalizeString(capture?.rpcId);
  const label = normalizeString(capture?.label);
  return rpcIds.some((candidate) => {
    const normalizedCandidate = normalizeString(candidate);
    return normalizedCandidate && (rpcId === normalizedCandidate || label === normalizedCandidate);
  });
}

function selectProgramRpcCapture(captureState, type, rpcIds, appPath) {
  const captures = [...(captureState?.rpcCaptures || [])].reverse();
  return (
    captures.find(
      (capture) =>
        capture?.type === type &&
        rpcCaptureMatchesIds(capture, rpcIds) &&
        rpcCaptureSourcePathMatches(capture, appPath),
    ) ?? null
  );
}

function selectRecentRpcCapture(captureState, type, rpcIds) {
  const captures = [...(captureState?.rpcCaptures || [])].reverse();
  return (
    captures.find(
      (capture) => capture?.type === type && rpcCaptureMatchesIds(capture, rpcIds),
    ) ?? null
  );
}

function extractModelHintFromRpcText(text) {
  const source = String(text || "");
  const match = source.match(/models\/([^";,\]\\\s]+)/i);
  return normalizeString(match?.[1]);
}

function extractCanvasProxyWsUrlFromText(text) {
  const source = String(text || "");
  const patterns = [
    /DEFAULT_ENDPOINT\s*=\s*"([^"]+)"/i,
    /constructor\s*\(\s*endpoint\s*=\s*"([^"]+)"/i,
    /placeholder="((?:wss?|WSS?):\/\/[^"]+)"/i,
    /\b((?:wss?|WSS?):\/\/127\.0\.0\.1:\d+(?:\/ws)?)\b/i,
  ];
  for (const pattern of patterns) {
    const matched = source.match(pattern)?.[1];
    const normalized = normalizeString(matched);
    if (normalized) {
      return normalized;
    }
  }
  return null;
}

function extractCanvasProxyTargetDomainFromText(text) {
  const source = String(text || "");
  return (
    normalizeString(source.match(/targetDomain\s*=\s*"([^"]+)"/i)?.[1]) ??
    normalizeString(source.match(/https:\/\/([^/"'\s]+generativelanguage\.googleapis\.com)/i)?.[1]) ??
    null
  );
}

function collectCanvasProxyContractTexts(snapshot, captureState) {
  const values = [
    snapshot?.bodyText,
    snapshot?.bodyBase64,
    ...(captureState?.rpcCaptures || []).flatMap((capture) => [capture?.bodyText, capture?.url]),
  ];
  return values
    .map((value) => String(value || ""))
    .filter(
      (value) =>
        value.includes("Browser API Proxy Client") ||
        value.includes("Routing Google API requests via WebSocket") ||
        value.includes("Server WS Endpoint") ||
        value.includes("targetDomain") ||
        value.includes("proxy_request"),
    );
}

function deriveCanvasProxyWsInvokeCandidate(operation, snapshot, captureState) {
  const appPath =
    normalizeString(snapshot?.appPath) ??
    extractAppPath(snapshot?.pageUrl) ??
    extractAppPath(snapshot?.canvasProgramUrl) ??
    extractAppPath(snapshot?.url) ??
    null;
  const texts = collectCanvasProxyContractTexts(snapshot, captureState);
  for (const text of texts) {
    const wsUrl = extractCanvasProxyWsUrlFromText(text);
    const targetDomain = extractCanvasProxyTargetDomainFromText(text);
    if (!wsUrl && !targetDomain) {
      continue;
    }
    return {
      transportKind: "canvas_program_ws_candidate",
      wsUrl,
      apiStyle: targetDomain ? "google_generative_language" : null,
      requestPath: null,
      requestEnvelopeKind: "canvas_proxy_request",
      sourcePath: appPath,
      modelHint: extractModelHintFromRpcText(text),
      operation,
    };
  }
  return null;
}

function deriveProgramRpcInvokeCandidate(operation, snapshot, captureState) {
  const appPath =
    normalizeString(snapshot?.appPath) ??
    extractAppPath(snapshot?.pageUrl) ??
    extractAppPath(snapshot?.canvasProgramUrl) ??
    extractAppPath(snapshot?.url) ??
    null;
  if (!appPath) {
    return null;
  }
  if (operation === "video") {
    const streamRequestCapture =
      selectProgramRpcCapture(captureState, "request", ["StreamGenerate"], appPath) ??
      selectRecentRpcCapture(captureState, "request", ["StreamGenerate"]) ??
      null;
    const streamResponseCapture =
      selectProgramRpcCapture(captureState, "response", ["StreamGenerate"], appPath) ??
      selectRecentRpcCapture(captureState, "response", ["StreamGenerate"]) ??
      null;
    if (streamRequestCapture || streamResponseCapture) {
      return {
        transportKind: "program_video_streamgenerate_candidate",
        requestEnvelopeKind: "page_stream_generate_form",
        requestUrl:
          normalizeString(streamRequestCapture?.url) ??
          normalizeString(streamResponseCapture?.url),
        requestBody: normalizeString(streamRequestCapture?.bodyText),
        requestRpcId: normalizeString(streamRequestCapture?.rpcId),
        responseRpcId: normalizeString(streamResponseCapture?.rpcId),
        sourcePath:
          normalizeString(streamRequestCapture?.sourcePath) ??
          normalizeString(streamResponseCapture?.sourcePath) ??
          normalizeString(appPath),
        modelHint:
          extractModelHintFromRpcText(streamResponseCapture?.bodyText) ??
          extractModelHintFromRpcText(streamRequestCapture?.bodyText),
        cookieHeader: normalizeString(streamRequestCapture?.cookieHeader),
      };
    }
    const requestCapture =
      selectProgramRpcCapture(captureState, "request", ["hNvQHb", "kwDCne"], appPath) ?? null;
    const responseCapture =
      selectProgramRpcCapture(captureState, "response", ["hNvQHb", "MUAZcd", "kwDCne"], appPath) ??
      null;
    if (!requestCapture && !responseCapture) {
      return null;
    }
    return {
      transportKind: "program_video_batchexecute_candidate",
      requestEnvelopeKind: "page_rpc_form",
      requestUrl: normalizeString(requestCapture?.url) ?? normalizeString(responseCapture?.url),
      requestBody: normalizeString(requestCapture?.bodyText),
      requestRpcId: normalizeString(requestCapture?.rpcId),
      responseRpcId: normalizeString(responseCapture?.rpcId),
      sourcePath: normalizeString(requestCapture?.sourcePath) ?? normalizeString(responseCapture?.sourcePath),
      modelHint:
        extractModelHintFromRpcText(responseCapture?.bodyText) ??
        extractModelHintFromRpcText(requestCapture?.bodyText),
    };
  }
  if (operation === "music") {
    const streamRequestCapture =
      selectProgramRpcCapture(captureState, "request", ["StreamGenerate"], appPath) ??
      selectRecentRpcCapture(captureState, "request", ["StreamGenerate"]) ??
      null;
    const streamResponseCapture =
      selectProgramRpcCapture(captureState, "response", ["StreamGenerate"], appPath) ??
      selectRecentRpcCapture(captureState, "response", ["StreamGenerate"]) ??
      null;
    if (streamRequestCapture || streamResponseCapture) {
      return {
        transportKind: "program_music_streamgenerate_candidate",
        requestEnvelopeKind: "page_stream_generate_form",
        requestUrl:
          normalizeString(streamRequestCapture?.url) ??
          normalizeString(streamResponseCapture?.url),
        requestBody: normalizeString(streamRequestCapture?.bodyText),
        requestRpcId: normalizeString(streamRequestCapture?.rpcId),
        responseRpcId: normalizeString(streamResponseCapture?.rpcId),
        sourcePath:
          normalizeString(streamRequestCapture?.sourcePath) ??
          normalizeString(streamResponseCapture?.sourcePath) ??
          normalizeString(appPath),
        modelHint:
          extractModelHintFromRpcText(streamResponseCapture?.bodyText) ??
          extractModelHintFromRpcText(streamRequestCapture?.bodyText),
        cookieHeader: normalizeString(streamRequestCapture?.cookieHeader),
      };
    }
    const requestCapture =
      selectProgramRpcCapture(captureState, "request", ["hNvQHb", "kwDCne"], appPath) ?? null;
    const responseCapture =
      selectProgramRpcCapture(captureState, "response", ["hNvQHb", "MUAZcd", "kwDCne"], appPath) ??
      null;
    if (!requestCapture && !responseCapture) {
      return null;
    }
    return {
      transportKind: "program_music_batchexecute_candidate",
      requestEnvelopeKind: "page_rpc_form",
      requestUrl: normalizeString(requestCapture?.url) ?? normalizeString(responseCapture?.url),
      requestBody: normalizeString(requestCapture?.bodyText),
      requestRpcId: normalizeString(requestCapture?.rpcId),
      responseRpcId: normalizeString(responseCapture?.rpcId),
      sourcePath: normalizeString(requestCapture?.sourcePath) ?? normalizeString(responseCapture?.sourcePath),
      modelHint:
        extractModelHintFromRpcText(responseCapture?.bodyText) ??
        extractModelHintFromRpcText(requestCapture?.bodyText),
    };
  }
  return null;
}

function buildCanvasProgramInvokeContract(
  operation,
  actionContract,
  transportHints,
  snapshot,
  fallbackPrompt = null,
  captureState = null,
) {
  const transport = transportHints || {};
  const actionInput = actionContract?.canvasProgramActionInput ?? null;
  const prompt =
    extractQuotedScalar(actionInput, ["prompt"]) ??
    extractQuotedScalar(snapshot?.bodyText, ["prompt"]) ??
    normalizeString(fallbackPrompt);
  const durationSeconds =
    extractNumberScalar(actionInput, ["duration_seconds", "durationSeconds", "duration"]) ??
    extractDurationSecondsFromBodyText(snapshot?.bodyText);
  const aspectRatio = extractQuotedScalar(actionInput, ["aspect_ratio", "aspectRatio", "aspect"]);
  const uiState = inferInvokeUiState(operation, snapshot);
  const playerReadyTargetCandidates =
    operation === "music" || operation === "video"
      ? collectPlayerReadyTargetCandidates(operation, snapshot, captureState)
      : [];
  const playerReadyTarget = playerReadyTargetCandidates[0] ?? null;
  const wsInvokeCandidate = deriveCanvasProxyWsInvokeCandidate(operation, snapshot, captureState);
  const rpcInvokeCandidate = deriveProgramRpcInvokeCandidate(operation, snapshot, captureState);
  const preferredInvokeCandidate =
    rpcInvokeCandidate ??
    wsInvokeCandidate ??
    null;

  let transportKind = null;
  let target = null;
  if (operation === "music") {
    const laneTarget = [...(transport.musicWsUrls || [])].reverse()[0] ?? null;
    target =
      (uiState === "music_player_ready" ? playerReadyTarget?.url : null) ??
      rpcInvokeCandidate?.requestUrl ??
      wsInvokeCandidate?.wsUrl ??
      laneTarget;
    transportKind = rpcInvokeCandidate?.transportKind
      ?? wsInvokeCandidate?.transportKind
      ?? (
        [...(transport.musicWsUrls || [])].length
          ? "app_music_ws"
          : (uiState ? "official_music_ws_candidate" : null)
      );
  } else if (operation === "video") {
    const laneTarget =
      [...(transport.videoInvokePaths || [])].reverse()[0] ??
      [...(transport.invokeBaseUrls || [])].reverse()[0] ??
      null;
    target =
      (uiState === "video_player_ready" ? playerReadyTarget?.url : null) ??
      rpcInvokeCandidate?.requestUrl ??
      wsInvokeCandidate?.wsUrl ??
      laneTarget;
    transportKind = rpcInvokeCandidate?.transportKind
      ?? wsInvokeCandidate?.transportKind
      ?? (
        ([...(transport.videoInvokePaths || [])].length || [...(transport.invokeBaseUrls || [])].length)
          ? "app_video_http"
          : (uiState ? "official_video_http_candidate" : null)
      );
  } else if (operation === "image") {
    transportKind = wsInvokeCandidate?.transportKind ?? "app_image_http_candidate";
    target = wsInvokeCandidate?.wsUrl ?? null;
  }

  if (
    !transportKind &&
    !actionContract?.canvasProgramAction &&
    !actionInput &&
    !prompt &&
    durationSeconds == null &&
    !aspectRatio &&
    !uiState
  ) {
    return null;
  }

  return {
    operation,
    transportKind,
    target,
    wsUrl: preferredInvokeCandidate?.wsUrl ?? null,
    apiStyle: preferredInvokeCandidate?.apiStyle ?? null,
    requestPath: preferredInvokeCandidate?.requestPath ?? null,
    requestEnvelopeKind: preferredInvokeCandidate?.requestEnvelopeKind ?? null,
    requestUrl: preferredInvokeCandidate?.requestUrl ?? null,
    requestBody: preferredInvokeCandidate?.requestBody ?? null,
    requestRpcId: preferredInvokeCandidate?.requestRpcId ?? null,
    responseRpcId: preferredInvokeCandidate?.responseRpcId ?? null,
    sourcePath: preferredInvokeCandidate?.sourcePath ?? null,
    modelHint: preferredInvokeCandidate?.modelHint ?? null,
    cookieHeader:
      preferredInvokeCandidate?.cookieHeader ??
      normalizeString(captureState?.cookieHeader) ??
      null,
    targetSource: playerReadyTarget?.source ?? null,
    targetMimeType: playerReadyTarget?.mimeType ?? null,
    targetCandidates: playerReadyTargetCandidates,
    actionName: actionContract?.canvasProgramAction ?? null,
    actionInput,
    prompt,
    durationSeconds: Number.isFinite(durationSeconds) && durationSeconds > 0 ? durationSeconds : null,
    aspectRatio,
    uiState,
  };
}

function selectProgramConversationPair(handlePairs, stableAppPath) {
  const reversed = [...(handlePairs || [])].reverse();
  const shareProxyPair = reversed.find(
    (pair) =>
      pair?.sourceSurface === "canvas_proxy_client" &&
      /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")) &&
      pair.appPath &&
      pair.conversationId &&
      pair.responseId,
  );
  if (shareProxyPair) {
    return shareProxyPair;
  }
  const proxyPair = reversed.find(
    (pair) =>
      pair?.sourceSurface === "canvas_proxy_client" &&
      pair.appPath &&
      pair.conversationId &&
      pair.responseId,
  );
  if (proxyPair) {
    return proxyPair;
  }
  const preferredByPath =
    stableAppPath &&
    reversed.find(
      (pair) =>
        pair.appPath === stableAppPath &&
        /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
    );
  if (preferredByPath) {
    return preferredByPath;
  }
  const preferredSharePair = reversed.find((pair) =>
    /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
  );
  if (preferredSharePair) {
    return preferredSharePair;
  }
  return reversed.find((pair) => pair.appPath && pair.conversationId && pair.responseId) ?? null;
}

function selectLatestResponsePair(handlePairs) {
  return (
    [...(handlePairs || [])]
      .reverse()
      .find((pair) => pair.conversationId && pair.responseId) ?? null
  );
}

function hasCanvasProxyProgramCandidate(handlePairs, invokeContract = null) {
  if (invokeContract?.transportKind === "canvas_program_ws_candidate") {
    return true;
  }
  return [...(handlePairs || [])].some((pair) => pair?.sourceSurface === "canvas_proxy_client");
}

function concreteAppPathFromUrl(url) {
  const matched = extractAppPath(url);
  return matched && /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(matched) ? matched : null;
}

function selectCanonicalProgramPair(handlePairs, pageUrl, stableAppPath = null) {
  const stablePair = selectProgramConversationPair(handlePairs, stableAppPath);
  const latestPair = selectLatestResponsePair(handlePairs);
  const finalAppPath = concreteAppPathFromUrl(pageUrl);
  if (
    finalAppPath &&
    latestPair?.appPath === finalAppPath &&
    latestPair?.conversationId &&
    latestPair?.responseId
  ) {
    return latestPair;
  }
  const shareProxyPair =
    [...(handlePairs || [])]
      .reverse()
      .find(
        (pair) =>
          pair?.sourceSurface === "canvas_proxy_client" &&
          /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")) &&
          pair.appPath &&
          pair.conversationId &&
          pair.responseId,
      ) ?? null;
  if (shareProxyPair) {
    return shareProxyPair;
  }
  const proxyPair =
    [...(handlePairs || [])]
      .reverse()
      .find(
        (pair) =>
          pair?.sourceSurface === "canvas_proxy_client" &&
          pair.appPath &&
          pair.conversationId &&
          pair.responseId,
      ) ?? null;
  if (proxyPair) {
    return proxyPair;
  }
  return stablePair ?? latestPair;
}

function deriveShareId(shareUrlValue) {
  const match = String(shareUrlValue || "").match(/\/share\/([0-9a-z]{8,})/i);
  return match?.[1] ?? null;
}

function deriveConversationIdForAppPath(appPath, conversationIds) {
  const suffix = String(appPath || "").split("/").pop()?.trim();
  if (!suffix) {
    return [...(conversationIds || [])].reverse()[0] ?? null;
  }
  const matched = [...(conversationIds || [])]
    .reverse()
    .find((value) => String(value).replace(/^c_/, "") === suffix);
  return matched ?? [...(conversationIds || [])].reverse()[0] ?? null;
}

const context = await chromium.launchPersistentContext(profileDir, {
  executablePath,
  headless: false,
  locale: "zh-CN",
  args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
});

try {
  const initialPage = context.pages()[0] ?? (await context.newPage());
  let activePage = initialPage;
  let capture = startNetworkCapture(activePage);
  const aggregateHints = {
    appPaths: [],
    conversationIds: [],
    responseIds: [],
    sharePaths: [],
  };
  const adoptActivePage = async (nextPage) => {
    if (!nextPage || nextPage === activePage) {
      return;
    }
    capture.stop();
    capture = startNetworkCapture(nextPage, capture.state);
    const previousPage = activePage;
    activePage = nextPage;
    if (previousPage && previousPage !== nextPage && !previousPage.isClosed()) {
      await previousPage.close().catch(() => undefined);
    }
  };

  await activePage.goto(shareUrl, {
    waitUntil: "domcontentloaded",
    timeout: timeoutMs,
  });
  await waitForShareSurface(activePage, Math.min(timeoutMs, 20000));
  await activePage.waitForTimeout(1500);
  const shareBefore = await collectSnapshot(activePage, "share-before");
  mergeHints(aggregateHints, shareBefore.handleHints);
  mergeActionContract(capture.state.actionContract, extractCanvasProgramActionContractFromText(shareBefore.bodyText));
  capture.state.invokeContract = mergeInvokeContract(
    capture.state.invokeContract,
    buildCanvasProgramInvokeContract(
      operation,
      capture.state.actionContract,
      capture.state.transportHints,
      shareBefore,
      prompt,
      capture.state,
    ),
  );

  let shareFollow = await tryFollowShareEntryPoint(activePage);
  if (shareFollow.page) {
    await adoptActivePage(shareFollow.page);
  }
  await activePage.waitForTimeout(3000);
  const shareAfter = await collectSnapshot(activePage, "share-after-cta");
  mergeHints(aggregateHints, shareAfter.handleHints);
  mergeActionContract(capture.state.actionContract, extractCanvasProgramActionContractFromText(shareAfter.bodyText));
  capture.state.invokeContract = mergeInvokeContract(
    capture.state.invokeContract,
    buildCanvasProgramInvokeContract(
      operation,
      capture.state.actionContract,
      capture.state.transportHints,
      shareAfter,
      prompt,
      capture.state,
    ),
  );

  if (activePage.url().includes("/share/") && !(await hasPromptTextbox(activePage))) {
    shareFollow = await tryFollowShareEntryPoint(activePage);
    if (shareFollow.page) {
      await adoptActivePage(shareFollow.page);
    }
    await activePage.waitForTimeout(3000);
  }

  if (!strongestHandle(aggregateHints)) {
    if (!discoveryOnly) {
      await activePage.goto(appUrl, {
        waitUntil: "domcontentloaded",
        timeout: timeoutMs,
      });
      await activePage.waitForTimeout(3000);
    }
  }

  const before = await collectSnapshot(activePage, "before");
  mergeHints(aggregateHints, before.handleHints);
  mergeActionContract(capture.state.actionContract, extractCanvasProgramActionContractFromText(before.bodyText));
  capture.state.invokeContract = mergeInvokeContract(
    capture.state.invokeContract,
    buildCanvasProgramInvokeContract(
      operation,
      capture.state.actionContract,
      capture.state.transportHints,
      before,
      prompt,
      capture.state,
    ),
  );

  if (!discoveryOnly && !(await hasPromptTextbox(activePage))) {
    await clickOperationMode(activePage, operation).catch(() => false);
    await activePage.waitForTimeout(2000);
  }

  const shouldStayOnProxyDiscoverySurface =
    discoveryOnly &&
    hasCanvasProxyProgramCandidate(capture.state.handlePairs, capture.state.invokeContract);
  const newChatClicked = shouldStayOnProxyDiscoverySurface ? false : await clickNewChat(activePage);
  const modeSelected = shouldStayOnProxyDiscoverySurface
    ? false
    : await clickOperationMode(activePage, operation);
  const afterNewChat = shouldStayOnProxyDiscoverySurface
    ? before
    : await collectSnapshot(activePage, "after-new-chat");
  mergeHints(aggregateHints, afterNewChat.handleHints);
  mergeActionContract(
    capture.state.actionContract,
    extractCanvasProgramActionContractFromText(afterNewChat.bodyText),
  );
  capture.state.invokeContract = mergeInvokeContract(
    capture.state.invokeContract,
    buildCanvasProgramInvokeContract(
      operation,
      capture.state.actionContract,
      capture.state.transportHints,
      afterNewChat,
      prompt,
      capture.state,
    ),
  );

  if (
    !discoveryOnly &&
    operation === "music" &&
    /选择要混合制作的曲目/i.test(afterNewChat.bodyText)
  ) {
    const styleSelection = await trySelectMusicStyleCard(activePage, timeoutMs).catch(() => null);
    if (styleSelection?.clicked) {
      const afterStyleSelection = await collectSnapshot(activePage, "after-style-selection");
      mergeHints(aggregateHints, afterStyleSelection.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(afterStyleSelection.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          capture.state.actionContract,
          capture.state.transportHints,
          afterStyleSelection,
          prompt,
          capture.state,
        ),
      );
    }
  }

  if (!discoveryOnly) {
    await submitPrompt(activePage, prompt);
  }

  const deadline = Date.now() + timeoutMs;
  let lastSnapshot = afterNewChat;
  let acceptedProgressReadyAt = null;
  while (Date.now() < deadline) {
    await activePage.waitForTimeout(1800);
    lastSnapshot = await collectSnapshot(activePage, "after");
    mergeHints(aggregateHints, lastSnapshot.handleHints);
    mergeHints(aggregateHints, capture.state.handleHints);
    mergeActionContract(
      capture.state.actionContract,
      extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
    );
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        capture.state.actionContract,
        capture.state.transportHints,
        lastSnapshot,
        prompt,
        capture.state,
      ),
    );
    const strongHandleReady = Boolean(strongestHandle(aggregateHints));
    const transportReady = hasTransportHints(capture.state.transportHints);
    const invokeReady = invokeContractIndicatesConcreteProgress(
      operation,
      capture.state.invokeContract,
      lastSnapshot,
    );
    if (invokeReady && !acceptedProgressReadyAt) {
      acceptedProgressReadyAt = Date.now();
    }
    if (
      discoveryOnly &&
      strongHandleReady &&
      (transportReady ||
        Boolean(capture.state.invokeContract?.transportKind) ||
        Boolean(capture.state.invokeContract?.actionName) ||
        Boolean(capture.state.invokeContract?.uiState))
    ) {
      break;
    }
    if (
      !discoveryOnly &&
      strongHandleReady &&
      (
        operation === "image" ||
        transportReady ||
        (
          invokeReady &&
          (
            operation !== "music" ||
            capture.state.invokeContract?.uiState === "music_player_ready" ||
            Date.now() - acceptedProgressReadyAt >= 12_000
          )
        )
      )
    ) {
      break;
    }
  }

  const stableProgramPair = selectProgramConversationPair(capture.state.handlePairs, null);
  const latestResponsePair = selectLatestResponsePair(capture.state.handlePairs);
  const actionContract = {
    canvasProgramAction:
      capture.state.actionContract.canvasProgramAction ??
      extractCanvasProgramActionContractFromText(lastSnapshot.bodyText).canvasProgramAction,
    canvasProgramActionInput:
      capture.state.actionContract.canvasProgramActionInput ??
      extractCanvasProgramActionContractFromText(lastSnapshot.bodyText).canvasProgramActionInput,
  };
  let invokeContract = mergeInvokeContract(
    capture.state.invokeContract,
    buildCanvasProgramInvokeContract(
      operation,
      actionContract,
      capture.state.transportHints,
      lastSnapshot,
      prompt,
      capture.state,
    ),
  );
  if (
    !discoveryOnly &&
    invokeContract &&
    !invokeContract.target &&
    (invokeContract.uiState === "music_player_ready" ||
      invokeContract.uiState === "video_player_ready")
  ) {
    const playClicked = await clickMediaActionButton(activePage, operation, "play", timeoutMs);
    if (playClicked) {
      await activePage.waitForTimeout(2000);
      lastSnapshot = await collectSnapshot(activePage, "after-play");
      mergeHints(aggregateHints, lastSnapshot.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          capture.state.actionContract,
          capture.state.transportHints,
          lastSnapshot,
          prompt,
          capture.state,
        ),
      );
      invokeContract = capture.state.invokeContract;
    }
  }
  if (
    !discoveryOnly &&
    invokeContract &&
    !invokeContract.target &&
    (invokeContract.uiState === "music_player_ready" ||
      invokeContract.uiState === "video_player_ready")
  ) {
    const downloadClicked = await clickMediaActionButton(activePage, operation, "download", timeoutMs);
    if (downloadClicked) {
      await activePage.waitForTimeout(2500);
      lastSnapshot = await collectSnapshot(activePage, "after-download");
      mergeHints(aggregateHints, lastSnapshot.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          capture.state.actionContract,
          capture.state.transportHints,
          lastSnapshot,
          prompt,
          capture.state,
        ),
      );
      invokeContract = capture.state.invokeContract;
    }
  }

  const canonicalProgramPair =
    selectCanonicalProgramPair(capture.state.handlePairs, lastSnapshot.url, null) ??
    stableProgramPair;
  const finalConcreteAppPath = concreteAppPathFromUrl(lastSnapshot.url);

  const result = {
    ok: true,
    profileDir,
    executablePath,
    outDir,
    shareUrl,
    appUrl,
    operation,
    prompt,
    discoveryOnly,
    shareFollowKind: shareFollow.kind,
    newChatClicked,
    modeSelected,
    beforeUrl: before.url,
    finalUrl: lastSnapshot.url,
    appPath:
      finalConcreteAppPath ??
      canonicalProgramPair?.appPath ??
      extractAppPath(lastSnapshot.url) ??
      [...aggregateHints.appPaths].reverse().find((value) =>
        /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value)),
      ) ??
      null,
    strongestHandle: strongestHandle(aggregateHints),
    aggregateHints,
    transportHints: capture.state.transportHints,
    candidatePairs: capture.state.handlePairs,
    stableProgramPair,
    latestResponsePair,
    canvasProgramAction: actionContract.canvasProgramAction,
    canvasProgramActionInput: actionContract.canvasProgramActionInput,
    canvasProgramInvokeContract: invokeContract,
    rpcCaptures: capture.state.rpcCaptures,
    networkSummary: {
      requestCount: capture.state.requests.length,
      responseCount: capture.state.responses.length,
      rpcCaptureCount: capture.state.rpcCaptures.length,
    },
    note: discoveryOnly
      ? "This probe stayed in discovery-only mode: it harvested the Canvas app contract without switching back into generic Gemini chat generation."
      : "A concrete /app/<id>, c_<id>, or stable share-derived entrypoint is a stronger program owner candidate than generic /app, but still does not by itself prove a separate Canvas quota lane.",
  };

  const normalizedProgramHandle = {
    ok: true,
    capturedAt: new Date().toISOString(),
    shareUrl,
    shareId: deriveShareId(shareUrl),
    beforeUrl: before.url,
    finalUrl: lastSnapshot.url,
    shareFollowKind: shareFollow.kind,
    newChatClicked,
    modeSelected,
    discoveryOnly,
    pageUrl: lastSnapshot.url,
    cookieHeader:
      normalizeString(invokeContract?.cookieHeader) ??
      normalizeString(capture.state.cookieHeader) ??
      null,
    programUrl:
      (finalConcreteAppPath ? `https://gemini.google.com${finalConcreteAppPath}` : null) ??
      canonicalProgramPair?.programUrl ??
      (result.appPath && /^\/app\//i.test(result.appPath)
        ? `https://gemini.google.com${result.appPath}`
        : null),
    appPath: finalConcreteAppPath ?? canonicalProgramPair?.appPath ?? result.appPath,
    conversationId:
      canonicalProgramPair?.conversationId ??
      deriveConversationIdForAppPath(result.appPath, aggregateHints.conversationIds),
    responseId: canonicalProgramPair?.responseId ?? null,
    invokeBaseUrl: [...(capture.state.transportHints.invokeBaseUrls || [])].reverse()[0] ?? null,
    musicWsUrl: [...(capture.state.transportHints.musicWsUrls || [])].reverse()[0] ?? null,
    videoInvokePath: [...(capture.state.transportHints.videoInvokePaths || [])].reverse()[0] ?? null,
    canvasProgramAction: actionContract.canvasProgramAction,
    canvasProgramActionInput: actionContract.canvasProgramActionInput,
    canvasProgramInvokeContract: invokeContract,
    rpcCaptures: capture.state.rpcCaptures,
    lastSeenConversationId: latestResponsePair?.conversationId ?? null,
    lastSeenResponseId:
      latestResponsePair?.responseId ?? [...aggregateHints.responseIds].reverse()[0] ?? null,
    candidatePairs: capture.state.handlePairs,
    aggregateHints,
    runtimeProfileDir: profileDir,
    runtimeStateObjectKey: toObjectKeyFromLocalPath(profileDir),
    sourceSummaryPath: path.join(outDir, "summary.json"),
    note: discoveryOnly
      ? "This handle was materialized in discovery-only mode. The browser stayed on Canvas app discovery and did not switch back into generic Gemini chat generation."
      : "This is a browser-probed Canvas program handle candidate. It is stronger than generic /app but still not proof of the final browserless quota lane.",
  };

  writeFileSync(
    path.join(outDir, "network-requests.json"),
    `${JSON.stringify(capture.state.requests, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    path.join(outDir, "network-responses.json"),
    `${JSON.stringify(capture.state.responses, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    path.join(outDir, "program-rpc-captures.json"),
    `${JSON.stringify(capture.state.rpcCaptures, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(path.join(outDir, "summary.json"), `${JSON.stringify(result, null, 2)}\n`, "utf8");
  writeFileSync(
    path.join(outDir, "program-handle.json"),
    `${JSON.stringify(normalizedProgramHandle, null, 2)}\n`,
    "utf8",
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await context.close().catch(() => undefined);
}
