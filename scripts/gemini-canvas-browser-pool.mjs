import { chromium } from "playwright-core";
import { WebSocketServer } from "ws";
import { createServer } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { GetObjectCommand, S3Client } from "@aws-sdk/client-s3";
import selfsigned from "selfsigned";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 42321;
const DEFAULT_TLS_PORT = 42322;
const DEFAULT_IDLE_TIMEOUT_MS = 30 * 60 * 1000;
const DEFAULT_MAX_CONTEXTS = 4;
const DEFAULT_TIMEOUT_MS = 8 * 60 * 1000;
const DEFAULT_LOCALE = "zh-CN";
const PROGRAM_RPC_CAPTURE_LIMIT = 80;
const PROGRAM_RPC_TEXT_LIMIT = 40000;
const INTERESTING_PROGRAM_RPC_IDS = new Set([
  "ujx1Bf",
  "ESY5D",
  "L5adhe",
  "XhaU0b",
  "PCck7e",
  "aPya6c",
  "MaZiqc",
  "hNvQHb",
  "kwDCne",
  "MUAZcd",
  "qpEbW",
  "k81mDb",
]);
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const APP_ROOT = path.resolve(SCRIPT_DIR, "..");

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function closePageSafely(page, contextLabel, timeoutMs = 3_000) {
  if (!page) {
    return;
  }
  let settled = false;
  await Promise.race([
    page.close().catch(() => undefined).finally(() => {
      settled = true;
    }),
    delay(timeoutMs),
  ]);
  if (!settled) {
    log("page close timed out", JSON.stringify({ contextLabel, timeoutMs }));
  }
}

const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
];
const MACOS_EDGE_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];
const LINUX_EDGE_PATHS = [
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];

const contexts = new Map();
const initializingContexts = new Map();
const connectedClients = new Map();
const pendingConnectedProxyRequests = new Map();
let objectStorageClient = null;
let connectedClientBootstrapSource = null;

function log(...parts) {
  console.log("[gemini-canvas-pool]", ...parts);
}

function isBrowserDownloadAssetUrl(url) {
  return (
    typeof url === "string" &&
    /gg-dl|rd-gg-dl|googleusercontent|work\.fife\.usercontent\.google\.com/i.test(url)
  );
}

function geminiCanvasConnectedClientScriptPath() {
  return path.resolve(SCRIPT_DIR, "gemini-canvas-connected-client.js");
}

function loadConnectedClientBootstrapSource() {
  if (connectedClientBootstrapSource) {
    return connectedClientBootstrapSource;
  }
  const scriptPath = geminiCanvasConnectedClientScriptPath();
  connectedClientBootstrapSource = readFileSync(scriptPath, "utf8");
  return connectedClientBootstrapSource;
}

function normalizeClientLabel(value) {
  return normalizeString(value)?.slice(0, 128) ?? null;
}

function geminiCanvasBrowserClientApiKey() {
  return (
    normalizeString(process.env.GEMINI_CANVAS_BROWSER_CLIENT_API_KEY) ??
    normalizeString(process.env.GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN) ??
    null
  );
}

function nextConnectedRequestId() {
  return `gemini_canvas_req_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

function connectedProxyRequestTimeoutMs() {
  return Math.max(
    Number(process.env.GEMINI_CANVAS_CONNECTED_REQUEST_TIMEOUT_MS || "120000"),
    10_000,
  );
}

function listConnectedClients() {
  return [...connectedClients.values()].filter((entry) => entry.authenticated);
}

function pickConnectedClient() {
  return listConnectedClients()[0] ?? null;
}

function cleanupConnectedClient(connectionId) {
  const entry = connectedClients.get(connectionId);
  if (!entry) {
    return;
  }
  connectedClients.delete(connectionId);
  for (const pending of pendingConnectedProxyRequests.values()) {
    if (pending.connectionId !== connectionId) {
      continue;
    }
    clearTimeout(pending.timeoutHandle);
    pending.reject(
      Object.assign(new Error("Gemini Canvas connected browser client disconnected."), {
        status: 503,
        code: "gemini_canvas_connected_client_disconnected",
      }),
    );
    pendingConnectedProxyRequests.delete(pending.requestId);
  }
}

function handleConnectedClientMessage(connectionId, rawMessage) {
  let payload;
  try {
    payload = JSON.parse(String(rawMessage));
  } catch {
    return;
  }
  const entry = connectedClients.get(connectionId);
  if (!entry) {
    return;
  }

  if (payload?.event_type === "authenticate") {
    const expectedApiKey = geminiCanvasBrowserClientApiKey();
    const providedApiKey = normalizeString(payload.apiKey);
    const clientLabel = normalizeClientLabel(payload.clientLabel) ?? `gemini-canvas-${connectionId}`;
    const authorized = !expectedApiKey || providedApiKey === expectedApiKey;
    entry.clientLabel = clientLabel;
    entry.authenticated = authorized;
    entry.ws.send(
      JSON.stringify({
        event_type: "auth_ack",
        authorized,
        message: authorized ? "" : "Invalid or missing API key",
      }),
    );
    if (!authorized) {
      entry.ws.close(4001, "invalid_api_key");
      return;
    }
    log("connected browser client authenticated", clientLabel);
    return;
  }

  const requestId = normalizeString(payload?.request_id);
  if (!requestId) {
    return;
  }
  const pending = pendingConnectedProxyRequests.get(requestId);
  if (!pending) {
    return;
  }

  switch (payload?.event_type) {
    case "response_headers":
      pending.status = Number(payload.status || 200);
      pending.headers = normalizeObject(payload.headers);
      return;
    case "chunk":
      if (typeof payload.data === "string") {
        pending.chunks.push(payload.data);
      }
      return;
    case "stream_close":
      clearTimeout(pending.timeoutHandle);
      pending.resolve({
        status: pending.status ?? 200,
        headers: pending.headers,
        bodyText: pending.chunks.join(""),
      });
      pendingConnectedProxyRequests.delete(requestId);
      return;
    case "error":
      clearTimeout(pending.timeoutHandle);
      pending.reject(
        Object.assign(new Error(normalizeString(payload.message) ?? "Connected browser client failed."), {
          status: Number(payload.status || 500),
          code: "gemini_canvas_connected_client_error",
          bodyText: normalizeString(payload.message) ?? null,
        }),
      );
      pendingConnectedProxyRequests.delete(requestId);
      return;
    default:
      return;
  }
}

async function dispatchConnectedProxyRequest(requestSpec) {
  const client = pickConnectedClient();
  if (!client) {
    throw Object.assign(new Error("No authenticated Gemini Canvas connected browser client is available."), {
      status: 503,
      code: "gemini_canvas_connected_client_unavailable",
    });
  }

  const requestId = nextConnectedRequestId();
  const requestAttemptId = `${requestId}_attempt_1`;
  return await new Promise((resolve, reject) => {
    const timeoutHandle = setTimeout(() => {
      pendingConnectedProxyRequests.delete(requestId);
      reject(
        Object.assign(new Error("Timed out waiting for Gemini Canvas connected browser client response."), {
          status: 504,
          code: "gemini_canvas_connected_client_timeout",
        }),
      );
    }, connectedProxyRequestTimeoutMs());

    pendingConnectedProxyRequests.set(requestId, {
      requestId,
      connectionId: client.connectionId,
      status: null,
      headers: {},
      chunks: [],
      resolve,
      reject,
      timeoutHandle,
    });

    client.ws.send(
      JSON.stringify({
        event_type: "proxy_request",
        request_id: requestId,
        request_attempt_id: requestAttemptId,
        request_attempt_number: 1,
        streaming_mode: "fake",
        is_generative: true,
        ...requestSpec,
      }),
    );
  });
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeObject(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

export function shouldAttemptConnectedClientFetchFallback(error, options = {}) {
  const message = error instanceof Error ? error.message : String(error ?? "");
  const method = normalizeString(options.method)?.toUpperCase() ?? "GET";
  return (
    options.useCanvasProxyMode !== true &&
    method === "POST" &&
    /Failed to fetch/i.test(message)
  );
}

export function inspectContextEntryForReuse(entry) {
  if (!entry?.context) {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  if (
    entry.attachedCdp === true &&
    entry.browser &&
    typeof entry.browser.isConnected === "function" &&
    entry.browser.isConnected() === false
  ) {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  let openPages = [];
  try {
    openPages =
      typeof entry.context.pages === "function"
        ? entry.context.pages().filter((page) => !(typeof page?.isClosed === "function" && page.isClosed()))
        : [];
  } catch {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  if (entry.page && typeof entry.page.isClosed === "function" && entry.page.isClosed() === false) {
    return {
      recreate: false,
      adoptedPage: false,
    };
  }

  const replacementPage = openPages[0] ?? null;
  if (replacementPage) {
    entry.page = replacementPage;
    return {
      recreate: false,
      adoptedPage: true,
    };
  }

  return {
    recreate: false,
    adoptedPage: false,
  };
}

function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
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

function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_EDGE_PATHS
      : process.platform === "darwin"
        ? MACOS_EDGE_PATHS
        : LINUX_EDGE_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

function parseCookieHeader(rawHeader) {
  const result = new Map();
  const normalized = normalizeString(rawHeader);
  if (!normalized) {
    return result;
  }
  for (const segment of normalized.split(";")) {
    const trimmed = segment.trim();
    if (!trimmed) {
      continue;
    }
    const separatorIndex = trimmed.indexOf("=");
    if (separatorIndex <= 0) {
      continue;
    }
    const name = trimmed.slice(0, separatorIndex).trim();
    const value = trimmed.slice(separatorIndex + 1).trim();
    if (name) {
      result.set(name, value);
    }
  }
  return result;
}

function shouldSyncGeminiCookie(name) {
  return /^(?:COMPASS|NID|SID|HSID|SSID|APISID|SAPISID|SIDCC|__Secure-(?:1|3)PSID(?:TS|CC|RTS)?|__Secure-(?:1|3)PAPISID)$/.test(
    String(name || ""),
  );
}

function buildGeminiCookieSyncObjects(rawHeader, baseUrl) {
  const cookieMap = parseCookieHeader(rawHeader);
  if (!cookieMap.size) {
    return [];
  }
  const originUrl = normalizeString(baseUrl) ?? "https://gemini.google.com";
  const cookies = [];
  for (const [name, value] of cookieMap.entries()) {
    if (!name || !value) {
      continue;
    }
    if (!shouldSyncGeminiCookie(name)) {
      continue;
    }
    const secure = name.startsWith("__Secure-") || name.startsWith("__Host-") || originUrl.startsWith("https://");
    if (name.startsWith("__Host-")) {
      cookies.push({
        name,
        value,
        url: originUrl,
        path: "/",
        secure: true,
        httpOnly: false,
      });
      continue;
    }
    cookies.push({
      name,
      value,
      domain: ".google.com",
      path: "/",
      secure,
      httpOnly: false,
    });
  }
  const deduped = [];
  const seen = new Set();
  for (const cookie of cookies) {
    const key = `${cookie.name}|${cookie.domain ?? cookie.url ?? ""}|${cookie.path ?? "/"}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    deduped.push(cookie);
  }
  return deduped;
}

async function syncCookieHeaderIntoContext(context, rawHeader, baseUrl) {
  const cookies = buildGeminiCookieSyncObjects(rawHeader, baseUrl);
  if (!cookies.length) {
    return 0;
  }
  try {
    await context.addCookies(cookies);
    return cookies.length;
  } catch (error) {
    let accepted = 0;
    for (const cookie of cookies) {
      const domain = normalizeString(cookie.domain);
      const path = normalizeString(cookie.path) ?? "/";
      const candidates = [
        cookie,
        domain?.startsWith(".")
          ? {
              ...cookie,
              domain: domain.slice(1),
              path,
            }
          : null,
      ].filter(Boolean);
      let synced = false;
      for (const candidate of candidates) {
        try {
          await context.addCookies([candidate]);
          accepted += 1;
          synced = true;
          break;
        } catch {
          // Try the next normalized shape below.
        }
      }
      if (!synced) {
        log(
          "cookie sync skipped invalid cookie",
          JSON.stringify({
            name: cookie.name,
            domain: cookie.domain ?? null,
            hasUrl: Boolean(cookie.url),
            path: cookie.path ?? null,
          }),
        );
      }
    }
    if (accepted > 0) {
      return accepted;
    }
    throw error;
  }
}

async function contextHasGeminiAuthCookies(context, baseUrl) {
  try {
    const origin = normalizeString(baseUrl) ?? "https://gemini.google.com";
    const cookies = await context.cookies([origin]);
    return cookies.some((cookie) =>
      /^(?:SID|HSID|SSID|APISID|SAPISID|SIDCC|__Secure-(?:1|3)PSID(?:TS|CC|RTS)?|__Secure-(?:1|3)PAPISID)$/.test(
        String(cookie?.name || ""),
      ),
    );
  } catch {
    return false;
  }
}

function inferFetchOrigin(baseUrl, url) {
  const preferred = normalizeString(baseUrl);
  if (preferred) {
    return new URL(preferred).origin;
  }
  return new URL(url).origin;
}

function inferGoogleAuthUser(pageUrl) {
  const current = normalizeString(pageUrl);
  if (!current) {
    return "0";
  }
  try {
    const parsed = new URL(current);
    const explicit = parsed.searchParams.get("authuser");
    if (explicit && /^\d+$/.test(explicit)) {
      return explicit;
    }
    const match = parsed.pathname.match(/\/u\/(\d+)\b/i);
    if (match?.[1]) {
      return match[1];
    }
  } catch {
    // Ignore malformed page URLs and fall back to the primary account slot.
  }
  return "0";
}

async function installCanvasProxyPreviewAuthIndexBridge(page, authIndex = "0") {
  const normalizedAuthIndex = /^\d+$/.test(String(authIndex ?? "").trim())
    ? String(authIndex).trim()
    : "0";

  const installSource = ({ authIndexValue }) => {
    const installBridge = () => {
      if (window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__) {
        return window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__;
      }
      window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__ = {
        installedAt: Date.now(),
        authIndex: authIndexValue,
        events: [],
      };
      const pushEvent = (event) => {
        try {
          window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__.events.push({
            t: Date.now(),
            ...event,
          });
        } catch {
          // ignore event capture failures
        }
      };
      if (!window.chrome) {
        window.chrome = {};
      }
      window.chrome._contextId = Number(authIndexValue);
      pushEvent({ kind: "install", authIndex: authIndexValue });

      if (!window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__) {
        const NativeWebSocket = window.WebSocket;
        if (typeof NativeWebSocket === "function") {
          const rewriteUrl = (value) => {
            const source = typeof value === "string" ? value : String(value ?? "");
            if (!/^ws:\/\/127\.0\.0\.1:9998(?:\/|\?|$)/i.test(source)) {
              return source;
            }
            try {
              const parsed = new URL(source);
              parsed.protocol = "wss:";
              pushEvent({
                kind: "ws_rewrite",
                from: source,
                to: parsed.toString(),
              });
              return parsed.toString();
            } catch {
              const rewritten = source.replace(/^ws:/i, "wss:");
              pushEvent({
                kind: "ws_rewrite",
                from: source,
                to: rewritten,
              });
              return rewritten;
            }
          };
          const WrappedWebSocket = function(url, protocols) {
            const rewrittenUrl = rewriteUrl(url);
            return protocols === undefined
              ? new NativeWebSocket(rewrittenUrl)
              : new NativeWebSocket(rewrittenUrl, protocols);
          };
          WrappedWebSocket.prototype = NativeWebSocket.prototype;
          Object.setPrototypeOf(WrappedWebSocket, NativeWebSocket);
          window.WebSocket = WrappedWebSocket;
          window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__ = true;
        }
      }

      window.addEventListener("message", (event) => {
        const data = event?.data;
        let messagePreview = null;
        try {
          messagePreview =
            typeof data === "string"
              ? data.slice(0, 1200)
              : JSON.stringify(data ?? null).slice(0, 1200);
        } catch {
          messagePreview = "[[message preview unavailable]]";
        }
        pushEvent({
          kind: "message",
          origin: String(event?.origin ?? ""),
          type:
            data && typeof data === "object" && "type" in data
              ? String(data.type)
              : typeof data,
          endpoint:
            data && typeof data === "object" && "endpoint" in data
              ? String(data.endpoint ?? "")
              : null,
          errorMessage:
            data && typeof data === "object" && "message" in data
              ? String(data.message ?? "")
              : null,
          messagePreview,
        });
        if (data && typeof data === "object" && data.type === "requestAuthIndex") {
          try {
            event.source?.postMessage(
              {
                type: "authIndexResponse",
                authIndex: authIndexValue,
              },
              "*",
            );
            pushEvent({
              kind: "reply",
              type: "authIndexResponse",
              authIndex: authIndexValue,
            });
          } catch (error) {
            pushEvent({
              kind: "reply_error",
              errorMessage: error instanceof Error ? error.message : String(error),
            });
          }
        }
      });
      return window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__;
    };

    const bridge = installBridge();
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", installBridge, { once: true });
    }
    return {
      authIndex: String(window.chrome?._contextId ?? authIndexValue),
      installedAt: bridge?.installedAt ?? null,
      eventCount: Array.isArray(bridge?.events) ? bridge.events.length : 0,
    };
  };

  await page
    .addInitScript(installSource, { authIndexValue: normalizedAuthIndex })
    .catch(() => undefined);

  return await page
    .evaluate(installSource, { authIndexValue: normalizedAuthIndex })
    .catch(() => ({
      authIndex: normalizedAuthIndex,
      installedAt: null,
      eventCount: 0,
    }));
}

function isFixtureCanvasBaseUrl(baseUrl) {
  const normalized = normalizeString(baseUrl);
  if (!normalized) {
    return false;
  }
  try {
    const origin = new URL(normalized);
    return !/gemini\.google\.com$/i.test(origin.hostname);
  } catch {
    return false;
  }
}

async function buildGoogleFetchAuthHeaders(entry, baseUrl, url) {
  const target = new URL(url);
  const hostname = target.hostname.toLowerCase();
  if (!hostname.endsWith("googleapis.com") && !hostname.endsWith("google.com")) {
    return {};
  }

  const origin = inferFetchOrigin(baseUrl, url);
  const cookies = await entry.context.cookies([origin, url]);
  const cookieHeader = cookies.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
  const cookieMap = parseCookieHeader(cookieHeader);
  const sapisid =
    cookieMap.get("__Secure-1PAPISID") ||
    cookieMap.get("__Secure-3PAPISID") ||
    cookieMap.get("SAPISID");
  if (!sapisid) {
    return {};
  }

  const timestamp = Math.floor(Date.now() / 1000);
  const hash = createHash("sha1")
    .update(`${timestamp} ${sapisid} ${origin}`, "utf8")
    .digest("hex");
  const authUser = inferGoogleAuthUser(entry.page?.url?.());
  return {
    Authorization: `SAPISIDHASH ${timestamp}_${hash} SAPISID1PHASH ${timestamp}_${hash} SAPISID3PHASH ${timestamp}_${hash}`,
    "X-Origin": origin,
    "X-Goog-AuthUser": authUser,
  };
}

function sanitizeTransportHintUrl(rawUrl) {
  const normalized = normalizeString(rawUrl);
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
  const normalized = normalizeString(rawUrl);
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
  const normalized = normalizeString(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    if (!/:predictLongRunning(?:$|\?)/i.test(parsed.pathname)) {
      return null;
    }
    return parsed.pathname;
  } catch {
    return null;
  }
}

function deriveMusicWsUrlFromRequestUrl(rawUrl) {
  const normalized = normalizeString(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    if (!/BidiGenerateMusic/i.test(parsed.pathname)) {
      return null;
    }
    parsed.search = "";
    parsed.hash = "";
    return parsed.toString();
  } catch {
    return null;
  }
}

function extractTransportHintsFromNetworkUrl(rawUrl) {
  return {
    invokeBaseUrl: deriveInvokeBaseUrlFromRequestUrl(rawUrl),
    musicWsUrl: deriveMusicWsUrlFromRequestUrl(rawUrl),
    videoInvokePath: deriveVideoInvokePathFromRequestUrl(rawUrl),
  };
}

function mergeTransportHints(target, incoming) {
  if (!incoming) {
    return;
  }
  if (incoming.invokeBaseUrl) {
    target.invokeBaseUrls = uniqueStrings([...(target.invokeBaseUrls || []), incoming.invokeBaseUrl]);
  }
  if (incoming.musicWsUrl) {
    target.musicWsUrls = uniqueStrings([...(target.musicWsUrls || []), incoming.musicWsUrl]);
  }
  if (incoming.videoInvokePath) {
    target.videoInvokePaths = uniqueStrings([...(target.videoInvokePaths || []), incoming.videoInvokePath]);
  }
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

function inferProgramInvokeMediaTarget(operation, snapshot, captureState) {
  return collectPlayerReadyTargetCandidates(
    operation,
    snapshot || { mediaNodes: [], anchorNodes: [] },
    captureState || { imageUrls: [], audioUrls: [], videoUrls: [] },
  )[0]?.url ?? null;
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

  for (const anchor of snapshot?.anchorNodes || snapshot?.anchors || []) {
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

function decodeEscapedCanvasProxyHtml(rawText) {
  const source = String(rawText || "");
  if (!source) {
    return null;
  }
  let decoded = source;
  for (let iteration = 0; iteration < 4; iteration += 1) {
    const next = decoded
      .replace(/\\\\u003c/g, "<")
      .replace(/\\\\u003e/g, ">")
      .replace(/\\\\u003d/g, "=")
      .replace(/\\\\u0026/g, "&")
      .replace(/\\\\u0027/g, "'")
      .replace(/\\\\u0022/g, "\"")
      .replace(/\\\\u005c/g, "\\")
      .replace(/\\\\n/g, "\n")
      .replace(/\\\\r/g, "\r")
      .replace(/\\\\t/g, "\t")
      .replace(/\\\\\"/g, "\"")
      .replace(/\\"/g, "\"")
      .replace(/\\\\'/g, "'")
      .replace(/\\'/g, "'")
      .replace(/\\\\\//g, "\\/");
    if (next === decoded) {
      break;
    }
    decoded = next;
  }
  const start = decoded.indexOf("<!DOCTYPE html>");
  const htmlStart = start >= 0 ? start : decoded.indexOf("<html");
  const htmlEnd = decoded.indexOf("</html>", htmlStart);
  if (htmlStart < 0 || htmlEnd <= htmlStart) {
    return null;
  }
  return decoded.slice(htmlStart, htmlEnd + "</html>".length);
}

function injectForcedCanvasProxyAuthIndex(html, authIndex = "0") {
  const source = String(html || "");
  if (!source) {
    return null;
  }
  const numericAuthIndex = /^\d+$/.test(String(authIndex ?? "").trim())
    ? Number(String(authIndex).trim())
    : 0;
  const bootstrapScript =
    `<script>` +
    `(function(){` +
    `window.chrome=window.chrome||{};` +
    `window.chrome._contextId=${numericAuthIndex};` +
    `window.__NEURO_FORCED_AUTH_INDEX__=${numericAuthIndex};` +
    `})();` +
    `</script>`;
  let patched = source
    .replace(
      /window\.__authIndexReady\s*=\s*new\s+Promise\(function\(resolve\)\s*\{\s*resolveAuthIndex\s*=\s*resolve;\s*\}\);/i,
      (matched) =>
        `${matched}\n` +
        `            if (Number.isInteger(window.__NEURO_FORCED_AUTH_INDEX__) && window.__NEURO_FORCED_AUTH_INDEX__ >= 0) {\n` +
        `                if (!window.chrome) window.chrome = {};\n` +
        `                window.chrome._contextId = window.__NEURO_FORCED_AUTH_INDEX__;\n` +
        `                resolveAuthIndex(window.__NEURO_FORCED_AUTH_INDEX__);\n` +
        `            }`,
    )
    .replace(/\bws:\/\/127\.0\.0\.1:9998\b/g, "wss://127.0.0.1:9998")
    .replace(
      /const response = await responsePromise;/m,
      `Logger.output("Response promise awaiting completed");\n` +
        `                const response = await responsePromise;\n` +
        `                Logger.output(\`Response promise resolved: status=\${response?.status ?? "unknown"} hasBody=\${Boolean(response?.body)}\`);`,
    )
    .replace(
      /reader = response\.body\.getReader\(\);/m,
      `if (!response.body) {\n` +
        `                    Logger.output("Response body missing before reader acquisition");\n` +
        `                    throw new Error("Response body missing before reader acquisition");\n` +
        `                }\n` +
        `                reader = response.body.getReader();\n` +
        `                Logger.output("Response reader acquired");`,
    )
    .replace(
      /this\._sendErrorResponse\(error, operationId, requestAttemptId\);/m,
      `Logger.output(\`Sending error response back to relay: \${error?.name || "Error"} \${error?.message || error}\`);\n` +
        `                    this._sendErrorResponse(error, operationId, requestAttemptId);`,
    )
    .replace(
      /const config = \{\s*headers: this\._sanitizeHeaders\(requestSpec\.headers\),\s*method: requestSpec\.method,\s*signal,\s*\};/m,
      `const config = {\n` +
        `                headers: this._sanitizeHeaders(requestSpec.headers),\n` +
        `                method: requestSpec.method,\n` +
        `                signal,\n` +
        `                credentials: "include",\n` +
        `            };`,
    )
    .replace(
      /const response = await fetch\(requestUrl, requestConfig\);/m,
      `Logger.output(\`Fetch start: \${requestUrl}\`);\n` +
        `                    Logger.output(\`Fetch config: method=\${requestConfig.method || "GET"} credentials=\${requestConfig.credentials || "default"} hasBody=\${Boolean(requestConfig.body)}\`);\n` +
        `                    const proxyFetchAbortController = new AbortController();\n` +
        `                    proxyFetchAbortController.signal.addEventListener("abort", () => {\n` +
        `                        const reason = proxyFetchAbortController.signal.reason;\n` +
        `                        const reasonText = reason && typeof reason === "object" && "message" in reason\n` +
        `                            ? String(reason.message)\n` +
        `                            : String(reason || "unknown");\n` +
        `                        Logger.output(\`Fetch abort signaled: \${reasonText}\`);\n` +
        `                    }, { once: true });\n` +
        `                    const forwardAbort = () => proxyFetchAbortController.abort(new DOMException("Proxy fetch aborted", "AbortError"));\n` +
        `                    if (requestConfig.signal) {\n` +
        `                        if (requestConfig.signal.aborted) {\n` +
        `                            forwardAbort();\n` +
        `                        } else {\n` +
        `                            requestConfig.signal.addEventListener("abort", forwardAbort, { once: true });\n` +
        `                        }\n` +
        `                    }\n` +
        `                    const fetchTimeoutId = setTimeout(() => {\n` +
        `                        Logger.output("Fetch timeout reached; aborting request");\n` +
        `                        proxyFetchAbortController.abort(new DOMException("Proxy fetch timeout (30s)", "AbortError"));\n` +
        `                    }, 30000);\n` +
        `                    requestConfig.signal = proxyFetchAbortController.signal;\n` +
        `                    let response;\n` +
        `                    try {\n` +
        `                        response = await fetch(requestUrl, requestConfig);\n` +
        `                    } catch (fetchError) {\n` +
        `                        Logger.output(\`Fetch threw: \${fetchError?.name || "Error"} \${fetchError?.message || fetchError}\`);\n` +
        `                        throw fetchError;\n` +
        `                    } finally {\n` +
        `                        clearTimeout(fetchTimeoutId);\n` +
        `                    }\n` +
        `                    Logger.output(\`Fetch done: \${response.status} \${response.url}\`);\n` +
        `                    Logger.output(\`Fetch body present: \${Boolean(response.body)}\`);`,
    );
  if (/<head[^>]*>/i.test(patched)) {
    patched = patched.replace(/<head[^>]*>/i, (matched) => `${matched}\n${bootstrapScript}`);
  } else {
    patched = `${bootstrapScript}\n${patched}`;
  }
  return patched;
}

function extractCanvasProxyClientHtmlFromTexts(values) {
  for (const value of values || []) {
    const decoded = decodeEscapedCanvasProxyHtml(value);
    if (decoded && /Browser (?:API )?Proxy Client/i.test(decoded)) {
      return decoded;
    }
  }
  return null;
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

function sanitizeCanvasProxyHeaders(headers) {
  const normalized = normalizeObject(headers);
  const forbidden = new Set([
    "host",
    "connection",
    "content-length",
    "origin",
    "referer",
    "user-agent",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-dest",
  ]);
  return Object.fromEntries(
    Object.entries(normalized).filter(([key]) => !forbidden.has(String(key).toLowerCase())),
  );
}

function sanitizeBrowserFetchHeaders(headers) {
  const normalized = normalizeObject(headers);
  const forbiddenPrefixes = ["sec-", "proxy-"];
  const forbidden = new Set([
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "host",
    "keep-alive",
    "origin",
    "permissions-policy",
    "priority",
    "referer",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "user-agent",
    "via",
  ]);
  return Object.fromEntries(
    Object.entries(normalized).filter(([key]) => {
      const lowered = String(key).toLowerCase();
      if (forbidden.has(lowered)) {
        return false;
      }
      return !forbiddenPrefixes.some((prefix) => lowered.startsWith(prefix));
    }),
  );
}

function resolveObjectStorageConfig() {
  const driver =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER) ??
    normalizeString(process.env.OBJECT_STORAGE_DRIVER) ??
    "local";
  const localDir =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.OBJECT_STORAGE_LOCAL_DIR) ??
    ".runtime/ai-gateway-objects";

  return {
    driver,
    localDir,
    bucket:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_BUCKET) ??
      normalizeString(process.env.OBJECT_STORAGE_BUCKET),
    region:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_REGION) ??
      normalizeString(process.env.OBJECT_STORAGE_REGION) ??
      "auto",
    endpoint:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ENDPOINT) ??
      normalizeString(process.env.OBJECT_STORAGE_ENDPOINT),
    accessKeyId:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID) ??
      normalizeString(process.env.OBJECT_STORAGE_ACCESS_KEY_ID),
    secretAccessKey:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY) ??
      normalizeString(process.env.OBJECT_STORAGE_SECRET_ACCESS_KEY),
    forcePathStyle: ["1", "true", "yes", "on"].includes(
      (
        process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
        process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
        ""
      )
        .trim()
        .toLowerCase(),
    ),
  };
}

function getStorageRoot() {
  const config = resolveObjectStorageConfig();
  return path.isAbsolute(config.localDir)
    ? config.localDir
    : path.resolve(process.cwd(), config.localDir);
}

function launchProfileCloneRoot() {
  return path.join(getStorageRoot(), "credential-runtime", "gemini-canvas-browser-launch-clones");
}

function profileClonePathForSource(sourcePath) {
  const hash = createHash("sha1").update(String(sourcePath || "")).digest("hex").slice(0, 12);
  return path.join(launchProfileCloneRoot(), `${hash}-${Date.now()}`);
}

function copyProfileTreeBestEffort(sourcePath, destinationPath) {
  const sourceStat = lstatSync(sourcePath);
  if (sourceStat.isDirectory()) {
    mkdirSync(destinationPath, { recursive: true });
    for (const entry of readdirSync(sourcePath, { withFileTypes: true })) {
      copyProfileTreeBestEffort(
        path.join(sourcePath, entry.name),
        path.join(destinationPath, entry.name),
      );
    }
    return;
  }
  if (!sourceStat.isFile()) {
    return;
  }
  mkdirSync(path.dirname(destinationPath), { recursive: true });
  try {
    copyFileSync(sourcePath, destinationPath);
  } catch (error) {
    log(
      "skipped locked profile entry during clone",
      JSON.stringify({
        sourcePath,
        destinationPath,
        message: error instanceof Error ? error.message : String(error),
      }),
    );
  }
}

function cloneProfileDirectory(sourcePath) {
  const destinationPath = profileClonePathForSource(sourcePath);
  mkdirSync(path.dirname(destinationPath), { recursive: true });
  log(
    "cloning live browser profile for isolated launch",
    JSON.stringify({ sourcePath, destinationPath }),
  );
  copyProfileTreeBestEffort(sourcePath, destinationPath);
  return destinationPath;
}

function getTlsAssetRoot() {
  const explicit = normalizeString(process.env.GEMINI_CANVAS_BROWSER_TLS_DIR);
  if (explicit) {
    return path.isAbsolute(explicit) ? explicit : path.resolve(process.cwd(), explicit);
  }
  return path.resolve(process.cwd(), "gateway/.runtime/gemini-canvas-browser-pool-tls");
}

function normalizeTlsAssetSlug(value) {
  return String(value || "local").replace(/[^a-z0-9._-]+/gi, "_");
}

function loadOrCreateTlsCertificate(host) {
  const tlsRoot = getTlsAssetRoot();
  mkdirSync(tlsRoot, { recursive: true });
  const slug = normalizeTlsAssetSlug(host);
  const keyPath = path.join(tlsRoot, `${slug}.key.pem`);
  const certPath = path.join(tlsRoot, `${slug}.cert.pem`);

  if (existsSync(keyPath) && existsSync(certPath)) {
    return {
      key: readFileSync(keyPath, "utf8"),
      cert: readFileSync(certPath, "utf8"),
      keyPath,
      certPath,
      generated: false,
    };
  }

  const attrs = [{ name: "commonName", value: host }];
  const altNames = [{ type: 2, value: "localhost" }];
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(host)) {
    altNames.push({ type: 7, ip: host });
  } else {
    altNames.push({ type: 2, value: host });
  }
  altNames.push({ type: 7, ip: "127.0.0.1" });
  const generated = selfsigned.generate(attrs, {
    algorithm: "sha256",
    days: 30,
    keySize: 2048,
    extensions: [{ name: "subjectAltName", altNames }],
  });
  writeFileSync(keyPath, generated.private, "utf8");
  writeFileSync(certPath, generated.cert, "utf8");
  return {
    key: generated.private,
    cert: generated.cert,
    keyPath,
    certPath,
    generated: true,
  };
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw Object.assign(new Error("Remote object storage is not fully configured."), {
      status: 500,
      code: "gemini_canvas_object_storage_not_configured",
    });
  }
  objectStorageClient = new S3Client({
    region: config.region,
    endpoint: config.endpoint,
    forcePathStyle: config.forcePathStyle,
    credentials: {
      accessKeyId: config.accessKeyId,
      secretAccessKey: config.secretAccessKey,
    },
  });
  return objectStorageClient;
}

async function mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath) {
  const client = getS3Client(config);
  const response = await client.send(
    new GetObjectCommand({
      Bucket: config.bucket,
      Key: runtimeStateObjectKey,
    }),
  );
  const body = response.Body;
  if (!body || typeof body.transformToByteArray !== "function") {
    throw Object.assign(
      new Error("Gemini Canvas runtime-state download did not return a readable object body."),
      {
        status: 500,
        code: "gemini_canvas_remote_profile_body_missing",
      },
    );
  }
  const bytes = await body.transformToByteArray();
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, Buffer.from(bytes));
  return absolutePath;
}

async function resolveRuntimeStateSource(runtimeStateObjectKey, options = {}) {
  const { allowFixtureEmptyProfile = false } = options;
  const config = resolveObjectStorageConfig();
  const normalizedKey = normalizeString(runtimeStateObjectKey);
  const absolutePath =
    normalizedKey && path.isAbsolute(normalizedKey)
      ? normalizedKey
      : path.join(getStorageRoot(), ...String(runtimeStateObjectKey || "").split("/"));
  if (!existsSync(absolutePath) && config.driver !== "local") {
    try {
      await mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath);
    } catch (error) {
      if (allowFixtureEmptyProfile) {
        mkdirSync(absolutePath, { recursive: true });
        return {
          mode: "profile_dir",
          absolutePath,
        };
      }
      const message = error instanceof Error ? error.message : String(error);
      throw Object.assign(
        new Error(
          `Gemini Canvas browser profiles currently require a locally mirrored browser profile directory or storageState JSON. Automatic mirror for '${runtimeStateObjectKey}' failed: ${message}`,
        ),
        {
          status: 500,
          code: "gemini_canvas_remote_profile_not_supported",
        },
      );
    }
  }
  if (existsSync(absolutePath)) {
    const stat = lstatSync(absolutePath);
    if (stat.isDirectory()) {
      return {
        mode: "profile_dir",
        absolutePath,
      };
    }
    if (stat.isFile() && absolutePath.toLowerCase().endsWith(".json")) {
      return {
        mode: "storage_state_file",
        absolutePath,
      };
    }
    throw Object.assign(
      new Error(
        `Gemini Canvas runtimeStateObjectKey must point to either a persistent browser profile directory or a Playwright storageState JSON file, but ${absolutePath} is neither.`,
      ),
      {
        status: 400,
        code: "gemini_canvas_runtime_state_invalid_path",
      },
    );
  }

  if (config.driver !== "local") {
    throw Object.assign(
      new Error(
        "Gemini Canvas browser profiles currently require a locally mirrored browser profile directory or storageState JSON. Remote object storage bundles without a local mirror are not supported yet.",
      ),
      {
        status: 500,
        code: "gemini_canvas_remote_profile_not_supported",
      },
    );
  }

  mkdirSync(absolutePath, { recursive: true });

  return {
    mode: "profile_dir",
    absolutePath,
  };
}

async function createContextEntry(args) {
  const browserCdpUrl = normalizeString(args.browserCdpUrl);
  const executablePath = resolveExecutablePath(
    args.browserExecutablePath ?? process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH ?? null,
  );
  if (!browserCdpUrl && !executablePath) {
    throw Object.assign(
      new Error(
        "Unable to locate a Chromium-compatible browser. Set GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH.",
      ),
      {
        status: 500,
        code: "gemini_canvas_browser_not_found",
      },
    );
  }

  let browser = null;
  let context;
  let launchClonedProfile = false;
  let runtimeStateSource = null;
  let launchRuntimePath = browserCdpUrl ?? args.runtimeStateObjectKey;
  let defaultProfileDir = null;
  if (browserCdpUrl) {
    const requestedTimeoutMs = Number(args.timeoutMs || DEFAULT_TIMEOUT_MS);
    const cdpConnectTimeoutMs = Number.isFinite(requestedTimeoutMs)
      ? Math.max(5_000, Math.min(requestedTimeoutMs, 120_000))
      : 120_000;
    browser = await chromium.connectOverCDP(browserCdpUrl, {
      timeout: cdpConnectTimeoutMs,
    });
    context =
      browser.contexts()[0] ??
      (await browser.newContext({
        locale: normalizeString(args.locale) ?? DEFAULT_LOCALE,
        ignoreHTTPSErrors: true,
        bypassCSP: true,
      }));
  } else {
    runtimeStateSource = await resolveRuntimeStateSource(args.runtimeStateObjectKey, {
      allowFixtureEmptyProfile: isFixtureCanvasBaseUrl(args.baseUrl),
    });
    launchRuntimePath = runtimeStateSource.absolutePath;
    defaultProfileDir =
      runtimeStateSource.mode === "profile_dir"
        ? path.join(launchRuntimePath, "Default")
        : null;
    const launchOptions = {
      executablePath,
      headless: parseBoolean(process.env.GEMINI_CANVAS_BROWSER_HEADLESS, true),
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-features=BlockInsecurePrivateNetworkRequests,PrivateNetworkAccessRespectPreflightResults",
        "--ignore-certificate-errors",
        "--allow-insecure-localhost",
        ...(defaultProfileDir && existsSync(defaultProfileDir) ? ["--profile-directory=Default"] : []),
      ],
    };
    if (runtimeStateSource.mode === "profile_dir") {
    const persistentOptions = {
      ...launchOptions,
      locale: normalizeString(args.locale) ?? DEFAULT_LOCALE,
      ignoreHTTPSErrors: true,
      bypassCSP: true,
    };
    try {
      context = await chromium.launchPersistentContext(launchRuntimePath, persistentOptions);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const shouldCloneRetry =
        /Target page, context or browser has been closed|exitCode=21|Process singleton|Opening in existing browser session/i.test(
          message,
        );
      if (!shouldCloneRetry) {
        throw error;
      }
      launchRuntimePath = cloneProfileDirectory(runtimeStateSource.absolutePath);
      launchClonedProfile = true;
      context = await chromium.launchPersistentContext(launchRuntimePath, persistentOptions);
    }
    } else {
      browser = await chromium.launch(launchOptions);
      context = await browser.newContext({
        locale: normalizeString(args.locale) ?? DEFAULT_LOCALE,
        storageState: runtimeStateSource.absolutePath,
        ignoreHTTPSErrors: true,
        bypassCSP: true,
      });
    }
  }
  const page = context.pages()[0] ?? (await context.newPage());
  const entry = {
    browser,
    context,
    page,
    busy: false,
    lastUsedAt: Date.now(),
    runtimeStatePath: browserCdpUrl ?? runtimeStateSource.absolutePath,
    launchRuntimePath,
    runtimeStateMode: browserCdpUrl ? "browser_cdp" : runtimeStateSource.mode,
    launchClonedProfile,
    attachedCdp: Boolean(browserCdpUrl),
  };
  contexts.set(args.runtimeStateObjectKey, entry);
  return entry;
}

async function ensureContext(args) {
  const existing = contexts.get(args.runtimeStateObjectKey);
  if (existing) {
    const reuseInspection = inspectContextEntryForReuse(existing);
    if (reuseInspection.recreate) {
      log("evicting stale Gemini Canvas context", args.runtimeStateObjectKey);
      await closeContext(args.runtimeStateObjectKey);
    } else {
      if (reuseInspection.adoptedPage) {
        log("adopted surviving Gemini Canvas page for cached context", args.runtimeStateObjectKey);
      }
      existing.lastUsedAt = Date.now();
      return existing;
    }
  }

  const inflight = initializingContexts.get(args.runtimeStateObjectKey);
  if (inflight) {
    return inflight;
  }

  const creation = createContextEntry(args).finally(() => {
    initializingContexts.delete(args.runtimeStateObjectKey);
  });
  initializingContexts.set(args.runtimeStateObjectKey, creation);
  return creation;
}

async function closeContext(runtimeStateObjectKey) {
  const entry = contexts.get(runtimeStateObjectKey);
  if (!entry) {
    return;
  }
  contexts.delete(runtimeStateObjectKey);
  if (entry.attachedCdp) {
    await entry.browser?.close().catch(() => undefined);
    return;
  }
  await entry.context.close().catch(() => undefined);
  if (entry.browser) {
    await entry.browser.close().catch(() => undefined);
  }
}

async function evictIdleContexts() {
  const idleTimeoutMs = Number(
    process.env.GEMINI_CANVAS_BROWSER_IDLE_TIMEOUT_MS || DEFAULT_IDLE_TIMEOUT_MS,
  );
  const now = Date.now();
  for (const [key, entry] of contexts.entries()) {
    if (entry.busy) {
      continue;
    }
    if (now - entry.lastUsedAt < idleTimeoutMs) {
      continue;
    }
    log("evicting idle context", key);
    await closeContext(key);
  }
}

async function evictIfOverCapacity() {
  const maxContexts = Number(
    process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS || DEFAULT_MAX_CONTEXTS,
  );
  if (contexts.size < maxContexts) {
    return;
  }

  const candidates = [...contexts.entries()]
    .filter(([, entry]) => !entry.busy)
    .sort((left, right) => left[1].lastUsedAt - right[1].lastUsedAt);
  const victim = candidates[0]?.[0];
  if (victim) {
    log("evicting least-recently-used context", victim);
    await closeContext(victim);
  }
}

async function ensureAppPage(entry, baseUrl, timeoutMs, options = {}) {
  if (entry.page.isClosed()) {
    throw Object.assign(new Error("Gemini Canvas browser page was unexpectedly closed."), {
      status: 500,
      code: "gemini_canvas_page_closed",
    });
  }

  const normalizedBaseUrl = baseUrl.replace(/\/+$/, "");
  const currentPageUrl = normalizeString(entry.page.url());
  const hasExplicitAuthUser =
    typeof currentPageUrl === "string" &&
    (/[?&]authuser=\d+\b/i.test(currentPageUrl) || /\/u\/\d+\b/i.test(currentPageUrl));
  const authUser = hasExplicitAuthUser ? inferGoogleAuthUser(currentPageUrl) : null;
  const appUrl =
    /^\d+$/.test(String(authUser ?? "").trim()) && String(authUser ?? "").trim() !== ""
      ? `${normalizedBaseUrl}/u/${String(authUser).trim()}/app`
      : `${normalizedBaseUrl}/app`;
  let shouldNavigate = true;
  let currentBodyText = "";
  try {
    const current = new URL(entry.page.url() || appUrl);
    const target = new URL(appUrl);
    shouldNavigate =
      current.origin !== target.origin ||
      current.pathname.replace(/\/+$/, "") !== target.pathname.replace(/\/+$/, "");
  } catch {
    shouldNavigate = !entry.page.url().startsWith(appUrl);
  }

  if (options?.skipInitialNavigationWhenAppSurfaceReady === true) {
    currentBodyText = await entry.page
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
    if (
      pageLooksLikeReusableGeminiAppSurface(
        currentPageUrl,
        currentBodyText,
        normalizedBaseUrl,
      )
    ) {
      shouldNavigate = false;
      log(
        "ensureAppPage reusing attached app surface without hard reload",
        JSON.stringify({
          appUrl,
          currentPageUrl,
          runtimeStatePath: entry.runtimeStatePath,
        }),
      );
    }
  }

  if (shouldNavigate) {
    log("ensureAppPage navigating", JSON.stringify({ appUrl, runtimeStatePath: entry.runtimeStatePath }));
    await entry.page.goto(appUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
  }

  await entry.page.waitForTimeout(2000);
  const consentResolved = await tryResolveGoogleConsent(entry.page, timeoutMs).catch(() => false);
  log("ensureAppPage consent pass", JSON.stringify({ consentResolved, finalUrl: entry.page.url() }));
  if (!consentResolved) {
    const consentUrl = entry.page.url();
    const consentBodyText = await entry.page
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
    if (bodyIndicatesGoogleConsent(consentBodyText, consentUrl)) {
      const consentButtons = await collectButtonSnapshot(entry.page).catch(() => []);
      throw Object.assign(new Error("Google consent surface is still blocking Gemini app interaction."), {
        status: 409,
        code: "gemini_canvas_google_consent_unresolved",
        bodyText: JSON.stringify(
          {
            pageUrl: consentUrl,
            bodyText: consentBodyText,
            buttons: consentButtons.slice(0, 120),
            runtimeStatePath: entry.runtimeStatePath,
            runtimeStateMode: entry.runtimeStateMode,
            launchClonedProfile: entry.launchClonedProfile === true,
          },
          null,
          2,
        ),
      });
    }
  }
  const finalUrl = entry.page.url();
  const loweredUrl = finalUrl.toLowerCase();
  if (
    loweredUrl.includes("signin") ||
    loweredUrl.includes("servicelogin") ||
    loweredUrl.includes("accounts.google.com")
  ) {
    throw Object.assign(new Error(`Gemini Canvas navigation redirected to ${finalUrl}.`), {
      status: 401,
      code: "gemini_canvas_auth_redirect",
    });
  }

  let bodyText = await entry.page
    .evaluate(() => document.body?.innerText ?? "")
    .catch(() => "");
  const isSignedOutLanding = (text) =>
    bodyIndicatesGeminiSignedOutLanding(text) ||
    (/sign in|登录|登入|继续登录/i.test(text) && !bodyTextIndicatesGeminiAppSurface(text));
  if (isSignedOutLanding(bodyText) && normalizeString(options.cookieHeader) && options.cookieRehydrateAttempted !== true) {
    const forcedCookieSyncCount = await syncCookieHeaderIntoContext(
      entry.context,
      options.cookieHeader,
      normalizedBaseUrl,
    ).catch((error) => {
      log(
        "ensureAppPage forced cookie rehydrate failed",
        JSON.stringify({
          runtimeStatePath: entry.runtimeStatePath,
          message: error instanceof Error ? error.message : String(error),
        }),
      );
      return 0;
    });
    if (forcedCookieSyncCount > 0) {
      log(
        "ensureAppPage forced cookie rehydrate",
        JSON.stringify({
          runtimeStatePath: entry.runtimeStatePath,
          runtimeStateMode: entry.runtimeStateMode,
          launchClonedProfile: entry.launchClonedProfile === true,
          forcedCookieSyncCount,
        }),
      );
      await entry.page.goto(appUrl, {
        waitUntil: "domcontentloaded",
        timeout: timeoutMs,
      });
      await entry.page.waitForTimeout(2000);
      const postRehydrateConsentResolved = await tryResolveGoogleConsent(entry.page, timeoutMs).catch(() => false);
      log(
        "ensureAppPage post-rehydrate consent pass",
        JSON.stringify({ postRehydrateConsentResolved, finalUrl: entry.page.url() }),
      );
      bodyText = await entry.page
        .evaluate(() => document.body?.innerText ?? "")
        .catch(() => "");
    }
  }
  if (isSignedOutLanding(bodyText)) {
    const authGateGraceDeadline = Date.now() + Math.min(timeoutMs, 8_000);
    while (Date.now() < authGateGraceDeadline) {
      await entry.page.waitForTimeout(1_000);
      const graceConsentResolved = await tryResolveGoogleConsent(entry.page, timeoutMs).catch(() => false);
      if (graceConsentResolved) {
        log("ensureAppPage grace consent resolved", JSON.stringify({ pageUrl: entry.page.url() }));
      }
      const reloadedUrl = entry.page.url();
      const loweredReloadedUrl = reloadedUrl.toLowerCase();
      if (
        loweredReloadedUrl.includes("signin") ||
        loweredReloadedUrl.includes("servicelogin") ||
        loweredReloadedUrl.includes("accounts.google.com")
      ) {
        throw Object.assign(new Error(`Gemini Canvas navigation redirected to ${reloadedUrl}.`), {
          status: 401,
          code: "gemini_canvas_auth_redirect",
        });
      }
      bodyText = await entry.page
        .evaluate(() => document.body?.innerText ?? "")
        .catch(() => "");
      if (
        bodyTextIndicatesGeminiAppSurface(bodyText) &&
        !bodyIndicatesGeminiSignedOutLanding(bodyText)
      ) {
        return;
      }
    }
    log(
      "ensureAppPage auth gate",
      JSON.stringify({
        finalUrl,
        bodyPreview: String(bodyText || "").slice(0, 600),
        runtimeStatePath: entry.runtimeStatePath,
        runtimeStateMode: entry.runtimeStateMode,
      }),
    );
    throw Object.assign(
      new Error("Gemini Canvas browser context appears to have fallen back to a sign-in page."),
      {
        status: 401,
        code: "gemini_canvas_auth_required",
      },
    );
  }
}

function resolveProgramPageUrl(baseUrl, args) {
  const isConcreteProgramUrl = (candidate) => {
    const value = normalizeString(candidate);
    if (!value) {
      return false;
    }
    try {
      const parsed = new URL(value);
      return /\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(parsed.pathname);
    } catch {
      return /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(value);
    }
  };
  const explicitProgramUrl =
    normalizeString(args?.canvasProgramUrl) ?? normalizeString(args?.programUrl);
  if (isConcreteProgramUrl(explicitProgramUrl)) {
    return explicitProgramUrl;
  }
  const appPath = normalizeString(args?.appPath);
  if (appPath && /^\/app\//i.test(appPath)) {
    return `${baseUrl.replace(/\/+$/, "")}${appPath}`;
  }
  const conversationId = normalizeString(args?.conversationId);
  if (conversationId && /^c_[0-9a-f]{8,}$/i.test(conversationId)) {
    return `${baseUrl.replace(/\/+$/, "")}/app/${conversationId.replace(/^c_/, "")}`;
  }
  const pageUrl = normalizeString(args?.pageUrl);
  if (isConcreteProgramUrl(pageUrl)) {
    return pageUrl;
  }
  return null;
}

async function ensureProgramPage(entry, baseUrl, programPageUrl, timeoutMs) {
  const targetUrl = normalizeString(programPageUrl);
  if (!targetUrl) {
    return false;
  }
  if (entry.page.isClosed()) {
    throw Object.assign(new Error("Gemini Canvas browser page was unexpectedly closed."), {
      status: 500,
      code: "gemini_canvas_page_closed",
    });
  }

  let shouldNavigate = true;
  try {
    const current = new URL(entry.page.url() || targetUrl);
    const target = new URL(targetUrl);
    shouldNavigate =
      current.origin !== target.origin ||
      current.pathname.replace(/\/+$/, "") !== target.pathname.replace(/\/+$/, "");
  } catch {
    shouldNavigate = entry.page.url() !== targetUrl;
  }

  if (shouldNavigate) {
    log("navigating to concrete canvas program page", targetUrl);
    await entry.page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
  }

  await entry.page.waitForTimeout(1800);
  const finalUrl = entry.page.url();
  const loweredUrl = finalUrl.toLowerCase();
  if (
    loweredUrl.includes("signin") ||
    loweredUrl.includes("servicelogin") ||
    loweredUrl.includes("accounts.google.com")
  ) {
    throw Object.assign(
      new Error(`Gemini Canvas program navigation redirected to ${finalUrl}.`),
      {
        status: 401,
        code: "gemini_canvas_program_auth_redirect",
      },
    );
  }
  return true;
}

async function ensureLoopbackConnectedClient(entry) {
  const host = normalizeString(process.env.GEMINI_CANVAS_BROWSER_HOST) ?? DEFAULT_HOST;
  const tlsPort = Number(process.env.GEMINI_CANVAS_BROWSER_POOL_TLS_PORT || DEFAULT_TLS_PORT);
  const endpoint = `wss://${host}:${tlsPort}/ws`;
  const apiKey = geminiCanvasBrowserClientApiKey() ?? "";
  const clientLabel = `gemini-canvas-loopback-${process.pid}`;

  await entry.page.addScriptTag({
    content: loadConnectedClientBootstrapSource(),
  });

  return await entry.page.evaluate(
    async ({ endpoint, apiKey, clientLabel }) => {
      const client = window.__neuroGeminiCanvasConnectedClient;
      if (!client) {
        throw new Error("Gemini Canvas connected client bootstrap did not install.");
      }
      return await client.connect({
        endpoint,
        apiKey,
        clientLabel,
      });
    },
    {
      endpoint,
      apiKey,
      clientLabel,
    },
  );
}

async function ensureSharePage(entry, baseUrl, shareId, timeoutMs) {
  if (entry.page.isClosed()) {
    throw Object.assign(new Error("Gemini Canvas browser page was unexpectedly closed."), {
      status: 500,
      code: "gemini_canvas_page_closed",
    });
  }

  const targetShareId = normalizeString(shareId);
  if (!targetShareId) {
    throw Object.assign(new Error("Gemini Canvas shareId is required for connected fetch mode."), {
      status: 400,
      code: "gemini_canvas_missing_share_id",
    });
  }

  const shareUrl = `${baseUrl.replace(/\/+$/, "")}/share/${targetShareId}`;
  let shouldNavigate = true;
  try {
    const current = new URL(entry.page.url() || shareUrl);
    const target = new URL(shareUrl);
    shouldNavigate =
      current.origin !== target.origin ||
      current.pathname.replace(/\/+$/, "") !== target.pathname.replace(/\/+$/, "");
  } catch {
    shouldNavigate = !entry.page.url().startsWith(shareUrl);
  }

  if (shouldNavigate) {
    log("navigating to share page", shareUrl);
    await entry.page.goto(shareUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
  }

  await waitForShareSurface(entry.page, Math.min(timeoutMs, 20_000));
  await entry.page.waitForTimeout(1500);
  const finalUrl = entry.page.url();
  const loweredUrl = finalUrl.toLowerCase();
  if (
    loweredUrl.includes("signin") ||
    loweredUrl.includes("servicelogin") ||
    loweredUrl.includes("accounts.google.com")
  ) {
    throw Object.assign(new Error(`Gemini Canvas share navigation redirected to ${finalUrl}.`), {
      status: 401,
      code: "gemini_canvas_auth_redirect",
    });
  }

  const bodyText = await entry.page
    .evaluate(() => document.body?.innerText ?? "")
    .catch(() => "");
  const shareSurfaceReady = /试用 Gemini Canvas|Try Gemini Canvas|在新窗口中打开|Open in new window|报告不安全的内容/i.test(
    bodyText,
  );
  if (/sign in|登录|登入|继续登录/i.test(bodyText) && !shareSurfaceReady) {
    throw Object.assign(
      new Error("Gemini Canvas connected page appears to require a fresh sign-in."),
      {
        status: 401,
        code: "gemini_canvas_auth_required",
      },
    );
  }
  log("share page ready", finalUrl);
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

function bodyIndicatesGoogleConsent(text, url = null) {
  const normalizedText = String(text || "");
  const normalizedUrl = String(url || "").toLowerCase();
  return (
    normalizedUrl.includes("consent.google.com") ||
    /Before you continue to Google|We use cookies and data to|Accept all|Reject all|More options|在您继续使用 Google 之前|我们会使用 Cookie 和数据|接受全部|全部接受|拒绝全部|全部拒绝|更多选项|g\.co\/privacytools/i.test(
      normalizedText,
    )
  );
}

function bodyIndicatesGeminiSignedOutLanding(text) {
  const normalizedText = String(text || "");
  return (
    /Sign in|登录|登入/i.test(normalizedText) &&
    (/Meet Gemini, your personal AI assistant|personal AI assistant/i.test(normalizedText) ||
      (/认识\s*Gemini/i.test(normalizedText) && /私人\s*AI/i.test(normalizedText)))
  );
}

function bodyTextIndicatesGeminiAppSurface(text) {
  return /与 Gemini 对话|Talk to Gemini|Conversation with Gemini|发起新对话|New chat|快速|Fast|制作图片|Create image|Make image|创作音乐|Create music|创作视频|Create video/i.test(
    String(text || ""),
  );
}

export function pageLooksLikeReusableGeminiAppSurface(pageUrl, bodyText, baseUrl) {
  const normalizedBaseUrl = normalizeString(baseUrl)?.replace(/\/+$/, "") ?? "";
  const normalizedPageUrl = normalizeString(pageUrl) ?? "";
  if (!normalizedBaseUrl || !normalizedPageUrl.startsWith(normalizedBaseUrl)) {
    return false;
  }
  if (!/\/app(?:\/|$|\?)/i.test(normalizedPageUrl)) {
    return false;
  }
  return (
    bodyTextIndicatesGeminiAppSurface(bodyText) &&
    !bodyIndicatesGeminiSignedOutLanding(bodyText)
  );
}

export async function findAttachedGeminiAppPage(entry, baseUrl) {
  if (!entry?.attachedCdp || !entry?.context || typeof entry.context.pages !== "function") {
    return null;
  }
  const pages = entry.context.pages().filter((page) => !page.isClosed());
  for (const page of pages) {
    const pageUrl = normalizeString(page.url()) ?? "";
    const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    if (pageLooksLikeReusableGeminiAppSurface(pageUrl, bodyText, baseUrl)) {
      return page;
    }
  }
  return null;
}

async function acquireMediaOperationPage(entry, baseUrl) {
  const attachedPage = await findAttachedGeminiAppPage(entry, baseUrl);
  if (attachedPage) {
    await attachedPage.bringToFront().catch(() => undefined);
    log(
      "reusing attached Gemini app page for media operation",
      JSON.stringify({
        pageUrl: normalizeString(attachedPage.url()) ?? "",
        runtimeStatePath: entry.runtimeStatePath,
      }),
    );
    return {
      page: attachedPage,
      closeWhenDone: false,
    };
  }

  const page = await entry.context.newPage();
  await page.bringToFront().catch(() => undefined);
  return {
    page,
    closeWhenDone: true,
  };
}

async function tryResolveGoogleConsent(page, timeoutMs) {
  const currentUrl = page.url();
  const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
  if (!bodyIndicatesGoogleConsent(bodyText, currentUrl)) {
    return false;
  }

  log(
    "google consent surface detected",
    JSON.stringify({
      currentUrl,
      bodyPreview: String(bodyText || "").slice(0, 400),
    }),
  );

  const clickConsentAction = async (labelMatchers, actionLabel) => {
    const candidates = [];
    for (const matcher of labelMatchers) {
      candidates.push(
        page.getByRole("button", { name: matcher }).first(),
        page.locator('button,[role="button"],a').filter({ hasText: matcher }).first(),
      );
    }
    let clicked = await clickFirstVisible(candidates);
    if (!clicked) {
      clicked = await page
        .evaluate((rawMatchers) => {
          const labels = rawMatchers.map((entry) => new RegExp(entry.pattern, entry.flags));
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
          const clickNode = (node) => {
            node.scrollIntoView({ block: "center", inline: "center" });
            node.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true }));
            node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
            node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
            node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
          };
          const nodes = Array.from(document.querySelectorAll("button,[role='button'],a,input[type='button'],input[type='submit']"));
          for (const matcher of labels) {
            for (const node of nodes) {
              const text = (node.innerText || node.getAttribute("value") || node.getAttribute("aria-label") || "").trim();
              if (!text || !matcher.test(text) || !isVisible(node)) {
                continue;
              }
              clickNode(node);
              return {
                clicked: true,
                text,
                tag: node.tagName || null,
              };
            }
          }
          return { clicked: false };
        }, labelMatchers.map((matcher) => ({ pattern: matcher.source, flags: matcher.flags })))
        .then((result) => {
          if (result?.clicked) {
            log("google consent dom fallback clicked", JSON.stringify({ actionLabel, ...result }));
            return true;
          }
          return false;
        })
        .catch(() => false);
    }
    if (clicked) {
      log("google consent action clicked", actionLabel);
    }
    return clicked;
  };

  const attempts = [
    { label: "reject_all", matchers: [/Reject all|拒绝全部|全部拒绝/i] },
    { label: "accept_all", matchers: [/Accept all|接受全部|全部接受/i] },
    { label: "agree", matchers: [/I agree|我同意/i] },
  ];

  for (const attempt of attempts) {
    const clicked = await clickConsentAction(attempt.matchers, attempt.label);
    if (!clicked) {
      continue;
    }
    const deadline = Date.now() + Math.min(timeoutMs, 8_000);
    while (Date.now() < deadline) {
      await page.waitForTimeout(800);
      const nextUrl = page.url();
      const nextBodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
      if (!bodyIndicatesGoogleConsent(nextBodyText, nextUrl)) {
        log(
          "google consent surface resolved",
          JSON.stringify({
            actionLabel: attempt.label,
            nextUrl,
            bodyPreview: String(nextBodyText || "").slice(0, 300),
          }),
        );
        return true;
      }
    }
  }

  log("google consent surface unresolved", "timed out waiting for consent page to dismiss");
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

async function tryOpenCanvasProxyPreview(page, timeoutMs) {
  const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
  if (!/Browser API Proxy Client/i.test(bodyText)) {
    return {
      clicked: false,
      reason: "canvas_proxy_client_not_visible",
      bodyPreview: String(bodyText || "").slice(0, 400),
    };
  }

  const bridge = await installCanvasProxyPreviewAuthIndexBridge(
    page,
    inferGoogleAuthUser(page.url()),
  ).catch(() => null);
  let stampedFrames = [];

  const candidates = [
    page.getByRole("button", { name: /预览|Preview/i }).first(),
    page.getByRole("tab", { name: /预览|Preview/i }).first(),
    page.locator("button,[role=\"button\"],[role=\"tab\"]").filter({ hasText: /预览|Preview/i }).first(),
  ];

  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) === 0) {
        continue;
      }
      await candidate.waitFor({ state: "visible", timeout: 4_000 });
      await candidate.click({ timeout: Math.min(timeoutMs, 12_000), force: true });
      const previewDeadline = Date.now() + Math.min(timeoutMs, 20_000);
      while (Date.now() < previewDeadline) {
        await page.waitForTimeout(1200);
        const bridgeEventCount = await page
          .evaluate(() => window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__?.events?.length ?? 0)
          .catch(() => 0);
        const iframeCount = await page.locator("iframe").count().catch(() => 0);
        if (iframeCount > 0 && bridgeEventCount <= 1) {
          stampedFrames = await stampCanvasProxyPreviewFrames(
            page,
            inferGoogleAuthUser(page.url()),
          ).catch(() => stampedFrames);
        }
        const bodyPreview = await page
          .evaluate(() => (document.body?.innerText ?? "").slice(0, 1200))
          .catch(() => "");
        if (
          bridgeEventCount > 1 ||
          /显示控制台|System Logs Output|Connecting\.\.\.|Connected|Disconnected|出了点问题|修正错误/i.test(bodyPreview)
        ) {
          break;
        }
      }
      const afterBodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
      const bridgeEvents = await page
        .evaluate(() => window.__NEURO_CANVAS_PROXY_AUTH_BRIDGE__?.events ?? [])
        .catch(() => []);
      const iframeNodes = await page
        .evaluate(() =>
          Array.from(document.querySelectorAll("iframe")).map((node, index) => ({
            index,
            id: node.id || null,
            name: node.getAttribute("name") || null,
            title: node.getAttribute("title") || null,
            src: node.getAttribute("src") || null,
          })),
        )
        .catch(() => []);
      const frameUrls = page.frames().map((frame, index) => ({
        index,
        name: frame.name() || null,
        url: frame.url(),
      }));
      return {
        clicked: true,
        reason: "preview_clicked",
        pageUrl: page.url(),
        bodyPreview: String(afterBodyText || "").slice(0, 600),
        bridge,
        bridgeEvents: Array.isArray(bridgeEvents) ? bridgeEvents.slice(-12) : [],
        stampedFrames,
        iframeNodes,
        frameUrls,
      };
    } catch {
      // try next candidate
    }
  }

  return {
    clicked: false,
    reason: "preview_button_not_clickable",
    pageUrl: page.url(),
    bodyPreview: String(bodyText || "").slice(0, 600),
    bridge,
    stampedFrames,
  };
}

async function stampCanvasProxyPreviewFrames(page, authIndex = "0", previewFetchRequest = null) {
  const numericAuthIndex = /^\d+$/.test(String(authIndex ?? "").trim())
    ? Number(String(authIndex).trim())
    : 0;
  const normalizedPreviewFetchRequest =
    previewFetchRequest && typeof previewFetchRequest === "object" && normalizeString(previewFetchRequest.url)
      ? {
          url: normalizeString(previewFetchRequest.url),
          method: normalizeString(previewFetchRequest.method)?.toUpperCase() ?? "POST",
          headers: normalizeObject(previewFetchRequest.headers),
          bodyText:
            typeof previewFetchRequest.bodyText === "string" ? previewFetchRequest.bodyText : null,
        }
      : null;
  const results = [];
  for (const [index, frame] of page.frames().entries()) {
    const frameUrl = frame.url();
    if (index === 0) {
      continue;
    }
    if (!/scf\.usercontent\.goog|^blob:/i.test(String(frameUrl || ""))) {
      continue;
    }
    try {
        const stamped = await frame.evaluate(({ authValue, previewFetchRequest }) => {
        window.chrome = window.chrome || {};
        window.chrome._contextId = authValue;
        if (!window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__) {
          const NativeWebSocket = window.WebSocket;
          if (typeof NativeWebSocket === "function") {
            const WrappedWebSocket = function(url, protocols) {
              const source = typeof url === "string" ? url : String(url ?? "");
              const rewritten = /^ws:\/\/127\.0\.0\.1:9998(?:\/|\?|$)/i.test(source)
                ? source.replace(/^ws:/i, "wss:")
                : source;
              return protocols === undefined
                ? new NativeWebSocket(rewritten)
                : new NativeWebSocket(rewritten, protocols);
            };
            WrappedWebSocket.prototype = NativeWebSocket.prototype;
            Object.setPrototypeOf(WrappedWebSocket, NativeWebSocket);
            window.WebSocket = WrappedWebSocket;
            window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__ = true;
          }
        }
        if (
          typeof ConnectionManager === "function" &&
          !window.__NEURO_CANVAS_PROXY_CONNECTION_MANAGER_PATCHED__
        ) {
          const originalEstablish = ConnectionManager.prototype.establish;
          ConnectionManager.prototype.establish = async function(...args) {
            if (typeof this.endpoint === "string" && /^ws:\/\/127\.0\.0\.1:9998(?:\/|$|\?)/i.test(this.endpoint)) {
              this.endpoint = this.endpoint.replace(/^ws:/i, "wss:");
            }
            return await originalEstablish.apply(this, args);
          };
          window.__NEURO_CANVAS_PROXY_CONNECTION_MANAGER_PATCHED__ = true;
        }
        try {
          window.postMessage({ type: "authIndexResponse", authIndex: authValue }, "*");
        } catch {
          // ignore self-post failures
        }
        if (
          typeof initializeProxySystem === "function" &&
          !window.__NEURO_CANVAS_PROXY_FORCE_REINITIALIZED__
        ) {
          window.__NEURO_CANVAS_PROXY_FORCE_REINITIALIZED__ = true;
          try {
            initializeProxySystem();
          } catch {
            // ignore reinit failures; page logs will capture them
          }
        }
        let probeFetch = null;
        try {
          const controller = new AbortController();
          const timer = setTimeout(() => controller.abort(), 15000);
          const sanitizeHeaders = (rawHeaders) => {
            const normalized =
              rawHeaders && typeof rawHeaders === "object" ? { ...rawHeaders } : {};
            const forbiddenHeaders = [
              "host",
              "connection",
              "content-length",
              "origin",
              "referer",
              "user-agent",
              "sec-fetch-mode",
              "sec-fetch-site",
              "sec-fetch-dest",
            ];
            for (const header of forbiddenHeaders) {
              delete normalized[header];
              delete normalized[header.toLowerCase()];
              delete normalized[header.toUpperCase()];
            }
            return normalized;
          };
          const defaultProbeRequest = {
            url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key=",
            method: "POST",
            headers: { "content-type": "application/json" },
            bodyText: JSON.stringify({
              contents: [
                {
                  role: "user",
                  parts: [{ text: "Reply with exactly: ok" }],
                },
              ],
            }),
          };
          const effectiveProbeRequest = previewFetchRequest?.url
            ? previewFetchRequest
            : defaultProbeRequest;
          let requestUrl = String(effectiveProbeRequest.url || defaultProbeRequest.url);
          try {
            const parsed = new URL(requestUrl);
            if (
              /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
              !parsed.searchParams.has("key")
            ) {
              parsed.searchParams.set("key", "");
              requestUrl = parsed.toString();
            }
          } catch {
            // keep original request URL
          }
          probeFetch = fetch(
            requestUrl,
            {
              method: String(effectiveProbeRequest.method || "POST"),
              headers: sanitizeHeaders(
                effectiveProbeRequest.headers && typeof effectiveProbeRequest.headers === "object"
                  ? effectiveProbeRequest.headers
                  : defaultProbeRequest.headers,
              ),
              body:
                typeof effectiveProbeRequest.bodyText === "string"
                  ? effectiveProbeRequest.bodyText
                  : defaultProbeRequest.bodyText,
              credentials: "include",
              mode: "cors",
              signal: controller.signal,
            },
          )
            .then(async (response) => {
              clearTimeout(timer);
              const responseText = await response.text();
              return {
                ok: response.ok,
                status: response.status,
                url: response.url,
                contentType: response.headers.get("content-type"),
                bodyText: responseText,
                bodyPreview: responseText.slice(0, 800),
              };
            })
            .catch((error) => ({
              ok: false,
              errorMessage: error instanceof Error ? error.message : String(error),
            }));
        } catch (error) {
          probeFetch = Promise.resolve({
            ok: false,
            errorMessage: error instanceof Error ? error.message : String(error),
          });
        }
        return Promise.resolve(probeFetch).then((probeFetchResult) => ({
          href: location.href,
          readyState: document.readyState,
          bodyPreview: (document.body?.innerText ?? "").slice(0, 400),
          chromeContextId: window.chrome?._contextId ?? null,
          hasAuthIndexReady: typeof window.__authIndexReady !== "undefined",
          hasInitializeProxySystem: typeof initializeProxySystem === "function",
          wsRewriteInstalled: window.__NEURO_CANVAS_PROXY_WS_REWRITE_INSTALLED__ === true,
          connectionManagerPatched: window.__NEURO_CANVAS_PROXY_CONNECTION_MANAGER_PATCHED__ === true,
          probeFetchResult,
        }));
      }, {
        authValue: numericAuthIndex,
        previewFetchRequest: normalizedPreviewFetchRequest,
      });
      results.push({
        index,
        url: frameUrl,
        stamped: true,
        ...stamped,
      });
    } catch (error) {
      results.push({
        index,
        url: frameUrl,
        stamped: false,
        errorMessage: error instanceof Error ? error.message : String(error),
      });
    }
  }
  return results;
}

async function tryLaunchCanvasProxyClientFromCapturedHtml(
  context,
  sourcePage,
  snapshot,
  captureState,
  timeoutMs,
) {
  const decodedHtml = extractCanvasProxyClientHtmlFromTexts(
    collectCanvasProxyContractTexts(snapshot, captureState),
  );
  if (!decodedHtml) {
    return {
      launched: false,
      reason: "canvas_proxy_client_html_missing",
    };
  }
  const authIndex = inferGoogleAuthUser(sourcePage?.url?.());
  const page = await context.newPage();
  const consoleEvents = [];
  const networkEvents = [];
  const onConsole = (message) => {
    const entry = {
      type: message.type(),
      text: message.text(),
    };
    consoleEvents.push(entry);
    if (consoleEvents.length > 60) {
      consoleEvents.shift();
    }
    log("canvas proxy direct launch console", JSON.stringify(entry));
  };
  const onPageError = (error) => {
    const entry = {
      type: "pageerror",
      text: error instanceof Error ? error.message : String(error),
    };
    consoleEvents.push(entry);
    if (consoleEvents.length > 60) {
      consoleEvents.shift();
    }
    log("canvas proxy direct launch pageerror", JSON.stringify(entry));
  };
  const pushNetworkEvent = (event) => {
    networkEvents.push(event);
    if (networkEvents.length > 80) {
      networkEvents.shift();
    }
    log("canvas proxy direct launch network", JSON.stringify(event));
  };
  const shouldCaptureDirectLaunchNetwork = (url) =>
    /generativelanguage\.googleapis\.com|127\.0\.0\.1:9998/i.test(String(url || ""));
  const onRequest = (request) => {
    const url = request.url();
    if (!shouldCaptureDirectLaunchNetwork(url)) {
      return;
    }
    pushNetworkEvent({
      type: "request",
      method: request.method(),
      url,
      headers: Object.fromEntries(
        Object.entries(request.headers() || {}).filter(([key]) =>
          ["content-type", "origin", "referer", "cookie"].includes(String(key || "").toLowerCase()),
        ),
      ),
      postData:
        typeof request.postData() === "string"
          ? request.postData().slice(0, 2000)
          : null,
    });
  };
  const onResponse = async (response) => {
    const url = response.url();
    if (!shouldCaptureDirectLaunchNetwork(url)) {
      return;
    }
    let bodyPreview = null;
    try {
      if ((response.headers()["content-type"] || "").includes("json")) {
        bodyPreview = (await response.text()).slice(0, 2000);
      }
    } catch {
      bodyPreview = null;
    }
    pushNetworkEvent({
      type: "response",
      url,
      status: response.status(),
      headers: Object.fromEntries(
        Object.entries(response.headers() || {}).filter(([key]) =>
          ["content-type", "access-control-allow-origin", "www-authenticate"].includes(
            String(key || "").toLowerCase(),
          ),
        ),
      ),
      bodyPreview,
    });
  };
  const onRequestFailed = (request) => {
    const url = request.url();
    if (!shouldCaptureDirectLaunchNetwork(url)) {
      return;
    }
    pushNetworkEvent({
      type: "requestfailed",
      method: request.method(),
      url,
      failureText: request.failure()?.errorText ?? null,
    });
  };
  const onRequestFinished = (request) => {
    const url = request.url();
    if (!shouldCaptureDirectLaunchNetwork(url)) {
      return;
    }
    pushNetworkEvent({
      type: "requestfinished",
      method: request.method(),
      url,
    });
  };
  page.on("console", onConsole);
  page.on("pageerror", onPageError);
  page.on("request", onRequest);
  page.on("response", onResponse);
  page.on("requestfailed", onRequestFailed);
  page.on("requestfinished", onRequestFinished);
  try {
    await page.bringToFront().catch(() => undefined);
    const patchedHtml = injectForcedCanvasProxyAuthIndex(decodedHtml, authIndex);
    const artifactDir = path.resolve(
      process.cwd(),
      ".runtime",
      "canvas-proxy-client-direct-launch",
    );
    await mkdir(artifactDir, { recursive: true });
    const artifactPath = path.join(
      artifactDir,
      `launch-${Date.now()}.html`,
    );
    await writeFile(artifactPath, patchedHtml, "utf8");
    await page.addInitScript(
      ({ authIndexValue }) => {
        window.chrome = window.chrome || {};
        window.chrome._contextId = authIndexValue;
        window.__NEURO_FORCED_AUTH_INDEX__ = authIndexValue;
      },
      { authIndexValue: Number(authIndex) || 0 },
    );
    const launchMode = "about_blank_set_content";
    await page.setContent(patchedHtml, {
      waitUntil: "domcontentloaded",
      timeout: Math.min(timeoutMs, 20_000),
    });
    const launchDeadline = Date.now() + Math.min(timeoutMs, 18_000);
    let bodyPreview = "";
    while (Date.now() < launchDeadline) {
      await page.waitForTimeout(800);
      bodyPreview = await page
        .evaluate(() => (document.body?.innerText ?? "").slice(0, 1600))
        .catch(() => "");
      if (
        /System Logs Output|Connecting\.\.\.|Connected|Disconnected|Connection successful/i.test(bodyPreview) ||
        consoleEvents.length > 0
      ) {
        break;
      }
    }
    const runtimeDiagnostics = await page
      .evaluate(() => ({
        url: location.href,
        readyState: document.readyState,
        scriptCount: document.scripts.length,
        chromeContextId: window.chrome?._contextId ?? null,
        forcedAuthIndex: window.__NEURO_FORCED_AUTH_INDEX__ ?? null,
        proxySystemType:
          typeof ProxySystem === "undefined"
            ? "undefined"
            : typeof ProxySystem,
        connectionManagerType:
          typeof ConnectionManager === "undefined"
            ? "undefined"
            : typeof ConnectionManager,
      }))
      .catch((error) => ({
        errorMessage: error instanceof Error ? error.message : String(error),
      }));
    return {
      launched: true,
      reason: launchMode,
      authIndex,
      pageUrl: page.url(),
      bodyPreview: String(bodyPreview || "").slice(0, 800),
      consoleEvents: consoleEvents.slice(-20),
      networkEvents: networkEvents.slice(-40),
      runtimeDiagnostics,
      artifactPath,
      htmlBytes: patchedHtml.length,
      htmlContainsRequestAuthIndex: /requestAuthIndex/.test(patchedHtml),
      htmlContainsProxySystemClass: /class\s+ProxySystem\b/.test(patchedHtml),
      htmlContainsProxySystemInitCall: /\.initialize\(/.test(patchedHtml) || /new\s+ProxySystem\b/.test(patchedHtml),
      htmlContainsSystemInitializingText: /System initializing/.test(patchedHtml),
      page,
    };
  } catch (error) {
    await page.close().catch(() => undefined);
    return {
      launched: false,
      reason: "captured_html_launch_error",
      authIndex,
      errorMessage: error instanceof Error ? error.message : String(error),
      consoleEvents: consoleEvents.slice(-20),
    };
  } finally {
    page.off("console", onConsole);
    page.off("pageerror", onPageError);
    page.off("request", onRequest);
    page.off("response", onResponse);
    page.off("requestfailed", onRequestFailed);
    page.off("requestfinished", onRequestFinished);
  }
}

function isCanvasProxyPreviewFrameUrl(url) {
  return /scf\.usercontent\.goog|^blob:/i.test(String(url || ""));
}

async function ensureCanvasProxyPreviewFrame(
  entry,
  baseUrl,
  shareId,
  timeoutMs,
  previewFetchRequest = null,
) {
  const adoptPage = async (nextPage) => {
    if (!nextPage || nextPage === entry.page) {
      return;
    }
    const previousPage = entry.page;
    entry.page = nextPage;
    if (previousPage && previousPage !== nextPage && !previousPage.isClosed()) {
      await previousPage.close().catch(() => undefined);
    }
  };

  await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
  let activePage = entry.page;
  let shareFollow = await tryFollowShareEntryPoint(activePage);
  if (shareFollow.page) {
    await adoptPage(shareFollow.page);
    activePage = entry.page;
  }
  await activePage.waitForTimeout(2500);

  let bodyText = await activePage
    .evaluate(() => document.body?.innerText ?? "")
    .catch(() => "");
  if (!/Browser API Proxy Client/i.test(bodyText)) {
    await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
    activePage = entry.page;
    shareFollow = await tryFollowShareEntryPoint(activePage);
    if (shareFollow.page) {
      await adoptPage(shareFollow.page);
      activePage = entry.page;
    }
    await activePage.waitForTimeout(2500);
    bodyText = await activePage
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
  }

  const preview = await tryOpenCanvasProxyPreview(activePage, timeoutMs);
  if (preview?.page) {
    await adoptPage(preview.page);
    activePage = entry.page;
  }
  await activePage.waitForTimeout(1200);

  const previewFrameDeadline = Date.now() + Math.min(Math.max(timeoutMs, 8000), 20000);
  let lastStampedFrames = [];
  let lastBodyText = bodyText;
  while (Date.now() < previewFrameDeadline) {
    lastStampedFrames = await stampCanvasProxyPreviewFrames(
      activePage,
      inferGoogleAuthUser(activePage.url()),
      previewFetchRequest,
    );
    const frames = activePage.frames();
    const preferredStampedFrame = lastStampedFrames.find(
      (entry) => entry?.stamped && entry?.probeFetchResult?.ok && frames[entry.index],
    );
    if (preferredStampedFrame) {
      return {
        page: activePage,
        frame: frames[preferredStampedFrame.index],
        preview,
        stampedFrames: lastStampedFrames,
      };
    }
    const fallbackStampedFrame = lastStampedFrames.find(
      (entry) => entry?.stamped && frames[entry.index],
    );
    if (fallbackStampedFrame) {
      return {
        page: activePage,
        frame: frames[fallbackStampedFrame.index],
        preview,
        stampedFrames: lastStampedFrames,
      };
    }
    const fallbackFrame = frames.find(
      (frame, index) => index > 0 && isCanvasProxyPreviewFrameUrl(frame.url()),
    );
    if (fallbackFrame) {
      return {
        page: activePage,
        frame: fallbackFrame,
        preview,
        stampedFrames: lastStampedFrames,
      };
    }
    await activePage.waitForTimeout(1000);
    lastBodyText = await activePage
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => lastBodyText);
  }
  throw Object.assign(
    new Error("Gemini Canvas preview frame was not available for no-key invocation."),
    {
      status: 503,
      code: "gemini_canvas_preview_frame_missing",
      bodyText: String(lastBodyText || "").slice(0, 800),
    },
  );
}

async function executeCanvasProxyPreviewNoKeyFetch(frame, request, timeoutMs) {
  return await frame.evaluate(
    async ({ url, method, headers, bodyText, timeoutMs }) => {
      const normalizeString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const appendEmptyKeyIfMissing = (rawUrl) => {
        const normalized = normalizeString(rawUrl);
        if (!normalized) {
          return null;
        }
        try {
          const parsed = new URL(normalized);
          if (
            /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
            !parsed.searchParams.has("key")
          ) {
            parsed.searchParams.set("key", "");
          }
          return parsed.toString();
        } catch {
          return normalized;
        }
      };
      const sanitizeHeaders = (rawHeaders) => {
        const normalized =
          rawHeaders && typeof rawHeaders === "object" ? { ...rawHeaders } : {};
        const forbiddenHeaders = [
          "host",
          "connection",
          "content-length",
          "origin",
          "referer",
          "user-agent",
          "sec-fetch-mode",
          "sec-fetch-site",
          "sec-fetch-dest",
        ];
        for (const header of forbiddenHeaders) {
          delete normalized[header];
          delete normalized[header.toLowerCase()];
          delete normalized[header.toUpperCase()];
        }
        return normalized;
      };
      const requestUrl = appendEmptyKeyIfMissing(url);
      const controller = new AbortController();
      const timer = setTimeout(() => {
        controller.abort(new DOMException("Canvas preview no-key fetch timeout", "AbortError"));
      }, timeoutMs);
      try {
        const response = await fetch(requestUrl, {
          method,
          headers: sanitizeHeaders(headers),
          body: typeof bodyText === "string" ? bodyText : undefined,
          credentials: "include",
          mode: "cors",
          signal: controller.signal,
        });
        const bodyBuffer = new Uint8Array(await response.arrayBuffer());
        const responseHeaders = {};
        response.headers.forEach((value, key) => {
          responseHeaders[key] = value;
        });
        const contentType = response.headers.get("content-type");
        const bodyTextResult =
          contentType && /(json|text|javascript|xml|html)/i.test(contentType)
            ? new TextDecoder().decode(bodyBuffer)
            : null;
        const toBase64 = (bytes) => {
          let binary = "";
          const chunkSize = 0x8000;
          for (let index = 0; index < bytes.length; index += chunkSize) {
            const chunk = bytes.subarray(index, index + chunkSize);
            binary += String.fromCharCode(...chunk);
          }
          return btoa(binary);
        };
        return {
          status: response.status,
          ok: response.ok,
          finalUrl: response.url || null,
          contentType,
          headers: responseHeaders,
          bodyText: bodyTextResult,
          bodyBase64: toBase64(bodyBuffer),
        };
      } catch (error) {
        return {
          status: 599,
          ok: false,
          finalUrl: null,
          contentType: null,
          headers: {},
          bodyText: null,
          bodyBase64: null,
          errorMessage: error instanceof Error ? error.message : String(error),
          errorName: error instanceof Error ? error.name : "Error",
        };
      } finally {
        clearTimeout(timer);
      }
    },
    {
      url: request.url,
      method: request.method,
      headers: request.headers,
      bodyText: request.bodyText,
      timeoutMs,
    },
  );
}

async function executeCanvasProxyPreviewNoKeyMusic(frame, request, timeoutMs) {
  return await frame.evaluate(
    async ({ url, requestBody, timeoutMs }) => {
      const normalizeString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const appendEmptyKeyIfMissing = (rawUrl) => {
        const normalized = normalizeString(rawUrl);
        if (!normalized) {
          return null;
        }
        try {
          const parsed = new URL(normalized);
          if (
            /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
            !parsed.searchParams.has("key")
          ) {
            parsed.searchParams.set("key", "");
          }
          return parsed.toString();
        } catch {
          return normalized;
        }
      };
      const decodeBase64Chunk = (raw) => {
        const binary = atob(raw);
        const bytes = new Uint8Array(binary.length);
        for (let index = 0; index < binary.length; index += 1) {
          bytes[index] = binary.charCodeAt(index);
        }
        return bytes;
      };
      const encodeBytesToBase64 = (chunks) => {
        let totalLength = 0;
        for (const chunk of chunks) {
          totalLength += chunk.length;
        }
        const merged = new Uint8Array(totalLength);
        let offset = 0;
        for (const chunk of chunks) {
          merged.set(chunk, offset);
          offset += chunk.length;
        }
        let binary = "";
        const chunkSize = 0x8000;
        for (let index = 0; index < merged.length; index += chunkSize) {
          const slice = merged.subarray(index, index + chunkSize);
          binary += String.fromCharCode(...slice);
        }
        return btoa(binary);
      };
      const requestPayload =
        requestBody && typeof requestBody === "object" ? requestBody : {};
      const setupFrame =
        requestPayload.setup && typeof requestPayload.setup === "object"
          ? requestPayload.setup
          : null;
      const clientContentFrame =
        requestPayload.client_content &&
        typeof requestPayload.client_content === "object"
          ? requestPayload.client_content
          : null;
      const musicGenerationConfigFrame =
        requestPayload.music_generation_config &&
        typeof requestPayload.music_generation_config === "object"
          ? requestPayload.music_generation_config
          : null;
      const playbackControlFrame =
        normalizeString(requestPayload.playback_control) ?? "PLAY";
      const candidateUrls = [];
      const requestedUrl = appendEmptyKeyIfMissing(url);
      if (requestedUrl) {
        candidateUrls.push(requestedUrl);
      }
      if (
        requestedUrl &&
        requestedUrl.includes("?key=") &&
        !candidateUrls.includes(requestedUrl.replace(/\?key=$/, ""))
      ) {
        candidateUrls.push(requestedUrl.replace(/\?key=$/, ""));
      }
      let lastFailure = {
        ok: false,
        status: 599,
        errorMessage: "canvas preview music websocket did not run",
        errorName: "PreviewMusicNoKeyError",
        events: [],
      };
      for (const requestUrl of candidateUrls) {
        const attemptResult = await new Promise((resolve) => {
          const events = [];
          let settled = false;
          let setupComplete = false;
          let sawAudio = false;
          let audioMimeType = "audio/L16;codec=pcm;rate=48000;channels=2";
          const audioChunks = [];
          let idleTimer = null;
          let timeoutHandle = null;
          const finish = (value) => {
            if (settled) {
              return;
            }
            settled = true;
            if (idleTimer) {
              clearTimeout(idleTimer);
            }
            if (timeoutHandle) {
              clearTimeout(timeoutHandle);
            }
            try {
              socket.close();
            } catch {
              // ignore close errors
            }
            resolve({
              setupComplete,
              events: events.slice(-20),
              ...value,
            });
          };
          const pushEvent = (value) => {
            events.push(String(value ?? "").slice(0, 1200));
            if (events.length > 40) {
              events.shift();
            }
          };
          const socket = new WebSocket(requestUrl);
          timeoutHandle = setTimeout(() => {
            finish({
              ok: false,
              status: 599,
              errorName: "PreviewMusicNoKeyTimeout",
              errorMessage: "Canvas preview music websocket timed out.",
            });
          }, timeoutMs);
          const resetIdleTimer = () => {
            if (!sawAudio) {
              return;
            }
            if (idleTimer) {
              clearTimeout(idleTimer);
            }
            idleTimer = setTimeout(() => {
              finish({
                ok: audioChunks.length > 0,
                status: audioChunks.length > 0 ? 101 : 599,
                contentType: audioMimeType,
                bodyBase64:
                  audioChunks.length > 0 ? encodeBytesToBase64(audioChunks) : null,
                bodyText:
                  audioChunks.length > 0
                    ? JSON.stringify({
                        audioChunkCount: audioChunks.length,
                        mimeType: audioMimeType,
                      })
                    : null,
              });
            }, 1800);
          };

          socket.addEventListener("open", () => {
            pushEvent(`open ${requestUrl}`);
            if (setupFrame) {
              socket.send(JSON.stringify({ setup: setupFrame }));
            } else {
              finish({
                ok: false,
                status: 400,
                errorName: "PreviewMusicNoKeyInvalidRequest",
                errorMessage: "music preview request is missing setup frame",
              });
            }
          });

          socket.addEventListener("message", async (event) => {
            let text;
            if (typeof event.data === "string") {
              text = event.data;
            } else if (event.data && typeof event.data.text === "function") {
              text = await event.data.text();
            } else {
              text = String(event.data);
            }
            pushEvent(text);
            let parsed = null;
            try {
              parsed = JSON.parse(text);
            } catch {
              parsed = null;
            }
            if (!setupComplete && (parsed?.setupComplete || parsed?.setup_complete)) {
              setupComplete = true;
              if (clientContentFrame) {
                socket.send(JSON.stringify({ client_content: clientContentFrame }));
              }
              if (
                musicGenerationConfigFrame &&
                Object.keys(musicGenerationConfigFrame).length > 0
              ) {
                socket.send(
                  JSON.stringify({
                    music_generation_config: musicGenerationConfigFrame,
                  }),
                );
              }
              socket.send(
                JSON.stringify({
                  playback_control: playbackControlFrame,
                }),
              );
              return;
            }
            if (parsed?.filteredPrompt || parsed?.filtered_prompt) {
              finish({
                ok: false,
                status: 400,
                errorName: "PreviewMusicNoKeyFilteredPrompt",
                errorMessage: String(
                  parsed.filteredPrompt ?? parsed.filtered_prompt ?? "filtered prompt",
                ),
              });
              return;
            }
            const chunks =
              parsed?.serverContent?.audioChunks ??
              parsed?.server_content?.audio_chunks ??
              null;
            if (!Array.isArray(chunks) || !chunks.length) {
              return;
            }
            for (const chunk of chunks) {
              const raw = normalizeString(chunk?.data);
              if (!raw) {
                continue;
              }
              sawAudio = true;
              audioMimeType =
                normalizeString(chunk?.mimeType) ??
                normalizeString(chunk?.mime_type) ??
                audioMimeType;
              audioChunks.push(decodeBase64Chunk(raw));
            }
            resetIdleTimer();
          });

          socket.addEventListener("error", (event) => {
            finish({
              ok: false,
              status: 599,
              errorName: "PreviewMusicNoKeyWsError",
              errorMessage:
                event?.message ?? event?.error?.message ?? "music websocket error",
            });
          });

          socket.addEventListener("close", (event) => {
            if (settled) {
              return;
            }
            if (audioChunks.length > 0) {
              finish({
                ok: true,
                status: 101,
                contentType: audioMimeType,
                bodyBase64: encodeBytesToBase64(audioChunks),
                bodyText: JSON.stringify({
                  audioChunkCount: audioChunks.length,
                  mimeType: audioMimeType,
                  closeCode: event.code,
                  closeReason: event.reason || "",
                }),
              });
              return;
            }
            finish({
              ok: false,
              status: 599,
              errorName: "PreviewMusicNoKeyWsClosed",
              errorMessage: `music websocket closed before audio: code=${event.code} reason=${event.reason || ""}`.trim(),
            });
          });
        });
        if (attemptResult.ok) {
          return attemptResult;
        }
        lastFailure = attemptResult;
      }
      return lastFailure;
    },
    {
      url: request.url,
      requestBody:
        request && typeof request === "object"
          ? request.jsonBody ??
            (typeof request.bodyText === "string"
              ? (() => {
                  try {
                    return JSON.parse(request.bodyText);
                  } catch {
                    return null;
                  }
                })()
              : null)
          : null,
      timeoutMs,
    },
  );
}

async function executeCanvasProgramPageNoKeyMusic(page, request, timeoutMs) {
  return await page.evaluate(
    async ({ url, requestBody, timeoutMs }) => {
      const normalizeString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const appendEmptyKeyIfMissing = (rawUrl) => {
        const normalized = normalizeString(rawUrl);
        if (!normalized) {
          return null;
        }
        try {
          const parsed = new URL(normalized);
          if (
            /generativelanguage\.googleapis\.com$/i.test(parsed.hostname) &&
            !parsed.searchParams.has("key")
          ) {
            parsed.searchParams.set("key", "");
          }
          return parsed.toString();
        } catch {
          return normalized;
        }
      };
      const decodeBase64Chunk = (raw) => {
        const binary = atob(raw);
        const bytes = new Uint8Array(binary.length);
        for (let index = 0; index < binary.length; index += 1) {
          bytes[index] = binary.charCodeAt(index);
        }
        return bytes;
      };
      const encodeBytesToBase64 = (chunks) => {
        let totalLength = 0;
        for (const chunk of chunks) {
          totalLength += chunk.length;
        }
        const merged = new Uint8Array(totalLength);
        let offset = 0;
        for (const chunk of chunks) {
          merged.set(chunk, offset);
          offset += chunk.length;
        }
        let binary = "";
        const chunkSize = 0x8000;
        for (let index = 0; index < merged.length; index += chunkSize) {
          const slice = merged.subarray(index, index + chunkSize);
          binary += String.fromCharCode(...slice);
        }
        return btoa(binary);
      };
      const requestPayload =
        requestBody && typeof requestBody === "object" ? requestBody : {};
      const setupFrame =
        requestPayload.setup && typeof requestPayload.setup === "object"
          ? requestPayload.setup
          : null;
      const clientContentFrame =
        requestPayload.client_content &&
        typeof requestPayload.client_content === "object"
          ? requestPayload.client_content
          : null;
      const musicGenerationConfigFrame =
        requestPayload.music_generation_config &&
        typeof requestPayload.music_generation_config === "object"
          ? requestPayload.music_generation_config
          : null;
      const playbackControlFrame =
        normalizeString(requestPayload.playback_control) ?? "PLAY";
      const candidateUrls = [];
      const requestedUrl = appendEmptyKeyIfMissing(url);
      if (requestedUrl) {
        candidateUrls.push(requestedUrl);
      }
      if (
        requestedUrl &&
        requestedUrl.includes("?key=") &&
        !candidateUrls.includes(requestedUrl.replace(/\?key=$/, ""))
      ) {
        candidateUrls.push(requestedUrl.replace(/\?key=$/, ""));
      }
      let lastFailure = {
        ok: false,
        status: 599,
        errorMessage: "canvas program page no-key music websocket did not run",
        errorName: "PageMusicNoKeyError",
        events: [],
      };
      for (const requestUrl of candidateUrls) {
        const attemptResult = await new Promise((resolve) => {
          const events = [];
          let settled = false;
          let setupComplete = false;
          let sawAudio = false;
          let audioMimeType = "audio/L16;codec=pcm;rate=48000;channels=2";
          const audioChunks = [];
          let idleTimer = null;
          let timeoutHandle = null;
          const finish = (value) => {
            if (settled) {
              return;
            }
            settled = true;
            if (idleTimer) {
              clearTimeout(idleTimer);
            }
            if (timeoutHandle) {
              clearTimeout(timeoutHandle);
            }
            try {
              socket.close();
            } catch {
              // ignore close errors
            }
            resolve({
              setupComplete,
              events: events.slice(-20),
              ...value,
            });
          };
          const pushEvent = (value) => {
            events.push(String(value ?? "").slice(0, 1200));
            if (events.length > 40) {
              events.shift();
            }
          };
          const socket = new WebSocket(requestUrl);
          timeoutHandle = setTimeout(() => {
            finish({
              ok: false,
              status: 599,
              errorName: "PageMusicNoKeyTimeout",
              errorMessage: "Canvas program page no-key music websocket timed out.",
            });
          }, timeoutMs);
          const resetIdleTimer = () => {
            if (!sawAudio) {
              return;
            }
            if (idleTimer) {
              clearTimeout(idleTimer);
            }
            idleTimer = setTimeout(() => {
              finish({
                ok: audioChunks.length > 0,
                status: audioChunks.length > 0 ? 101 : 599,
                contentType: audioMimeType,
                bodyBase64:
                  audioChunks.length > 0 ? encodeBytesToBase64(audioChunks) : null,
                bodyText:
                  audioChunks.length > 0
                    ? JSON.stringify({
                        audioChunkCount: audioChunks.length,
                        mimeType: audioMimeType,
                      })
                    : null,
              });
            }, 1800);
          };

          socket.addEventListener("open", () => {
            pushEvent(`open ${requestUrl}`);
            if (setupFrame) {
              socket.send(JSON.stringify({ setup: setupFrame }));
            } else {
              finish({
                ok: false,
                status: 400,
                errorName: "PageMusicNoKeyInvalidRequest",
                errorMessage: "music page no-key request is missing setup frame",
              });
            }
          });

          socket.addEventListener("message", async (event) => {
            let text;
            if (typeof event.data === "string") {
              text = event.data;
            } else if (event.data && typeof event.data.text === "function") {
              text = await event.data.text();
            } else {
              text = String(event.data);
            }
            pushEvent(text);
            let parsed = null;
            try {
              parsed = JSON.parse(text);
            } catch {
              parsed = null;
            }
            if (!setupComplete && (parsed?.setupComplete || parsed?.setup_complete)) {
              setupComplete = true;
              if (clientContentFrame) {
                socket.send(JSON.stringify({ client_content: clientContentFrame }));
              }
              if (
                musicGenerationConfigFrame &&
                Object.keys(musicGenerationConfigFrame).length > 0
              ) {
                socket.send(
                  JSON.stringify({
                    music_generation_config: musicGenerationConfigFrame,
                  }),
                );
              }
              socket.send(
                JSON.stringify({
                  playback_control: playbackControlFrame,
                }),
              );
              return;
            }
            if (parsed?.filteredPrompt || parsed?.filtered_prompt) {
              finish({
                ok: false,
                status: 400,
                errorName: "PageMusicNoKeyFilteredPrompt",
                errorMessage: String(
                  parsed.filteredPrompt ?? parsed.filtered_prompt ?? "filtered prompt",
                ),
              });
              return;
            }
            const chunks =
              parsed?.serverContent?.audioChunks ??
              parsed?.server_content?.audio_chunks ??
              null;
            if (!Array.isArray(chunks) || !chunks.length) {
              return;
            }
            for (const chunk of chunks) {
              const raw = normalizeString(chunk?.data);
              if (!raw) {
                continue;
              }
              sawAudio = true;
              audioMimeType =
                normalizeString(chunk?.mimeType) ??
                normalizeString(chunk?.mime_type) ??
                audioMimeType;
              audioChunks.push(decodeBase64Chunk(raw));
            }
            resetIdleTimer();
          });

          socket.addEventListener("error", (event) => {
            finish({
              ok: false,
              status: 599,
              errorName: "PageMusicNoKeyWsError",
              errorMessage:
                event?.message ?? event?.error?.message ?? "music websocket error",
            });
          });

          socket.addEventListener("close", (event) => {
            if (settled) {
              return;
            }
            if (audioChunks.length > 0) {
              finish({
                ok: true,
                status: 101,
                contentType: audioMimeType,
                bodyBase64: encodeBytesToBase64(audioChunks),
                bodyText: JSON.stringify({
                  audioChunkCount: audioChunks.length,
                  mimeType: audioMimeType,
                  closeCode: event.code,
                  closeReason: event.reason || "",
                }),
              });
              return;
            }
            finish({
              ok: false,
              status: 599,
              errorName: "PageMusicNoKeyWsClosed",
              errorMessage: `music websocket closed before audio: code=${event.code} reason=${event.reason || ""}`.trim(),
            });
          });
        });
        if (attemptResult.ok) {
          return attemptResult;
        }
        lastFailure = attemptResult;
      }
      return lastFailure;
    },
    {
      url: request.url,
      requestBody:
        request && typeof request === "object"
          ? request.jsonBody ??
            (typeof request.bodyText === "string"
              ? (() => {
                  try {
                    return JSON.parse(request.bodyText);
                  } catch {
                    return null;
                  }
                })()
              : null)
          : null,
      timeoutMs,
    },
  );
}

async function resetConversation(
  page,
  baseUrl,
  timeoutMs,
  preferredEntryUrl = null,
  options = {},
) {
  const appUrl = normalizeString(preferredEntryUrl) || `${baseUrl.replace(/\/+$/, "")}/app`;
  const preserveProgramContext =
    Boolean(normalizeString(preferredEntryUrl)) &&
    /\/app\/|\/canvas(?:\/|$)/i.test(String(preferredEntryUrl));

  const skipInitialNavigationWhenAppSurfaceReady =
    options?.skipInitialNavigationWhenAppSurfaceReady === true;
  let currentPageUrl = normalizeString(page.url()) ?? "";
  let currentBodyText = "";
  if (skipInitialNavigationWhenAppSurfaceReady) {
    currentBodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
  }
  const currentPageLooksReusable =
    skipInitialNavigationWhenAppSurfaceReady &&
    currentPageUrl.startsWith(baseUrl.replace(/\/+$/, "")) &&
    /\/app(?:\/|$|\?)/i.test(currentPageUrl) &&
    bodyTextIndicatesGeminiAppSurface(currentBodyText) &&
    !bodyIndicatesGeminiSignedOutLanding(currentBodyText);

  if (!currentPageLooksReusable) {
    await page.goto(appUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await page.waitForTimeout(1200);
    await dismissGeminiAppInterstitials(page, timeoutMs);
  } else {
    log(
      "resetConversation reusing attached app surface without hard reload",
      JSON.stringify({
        appUrl,
        currentPageUrl,
      }),
    );
  }

  if (preserveProgramContext) {
    return;
  }

  const newChatCandidates = [
    page.getByRole("button", { name: /发起新对话|New chat/i }).first(),
    page.locator('button[aria-label*="发起新对话"], button[aria-label*="New chat"]').first(),
    page.getByRole("link", { name: /发起新对话|New chat/i }).first(),
    page.getByRole("button", { name: /^Gemini$/i }).first(),
    page.getByRole("link", { name: /Gemini/i }).first(),
  ];

  for (const candidate of newChatCandidates) {
    try {
      if ((await candidate.count()) > 0) {
        await candidate.click({
          timeout: Math.min(timeoutMs, 10_000),
          force: true,
        });
        await page.waitForTimeout(1200);
        await dismissGeminiAppInterstitials(page, timeoutMs);
        break;
      }
    } catch {
      // Best effort only; navigation to /app already gives us a fresh-enough baseline.
    }
  }
}

async function dismissGeminiAppInterstitials(page, timeoutMs) {
  const candidates = [
    page.getByRole("button", { name: /以后再说|稍后再说|Not now|Maybe later/i }).first(),
    page.getByRole("link", { name: /以后再说|稍后再说|Not now|Maybe later/i }).first(),
    page.getByRole("button", { name: /知道了|Got it|Close/i }).first(),
    page.getByRole("button", { name: /跳过|Skip/i }).first(),
  ];

  for (const candidate of candidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 2_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 6_000), force: true });
      await page.waitForTimeout(800);
    } catch {
      // Best effort only.
    }
  }
}

function operationConfig(operation) {
  switch (operation) {
    case "text":
      return {
        buttonName: null,
        modeIndicator: null,
        resultTimeoutMs: 2 * 60 * 1000,
      };
    case "tts":
      return {
        buttonName: null,
        modeIndicator: null,
        resultTimeoutMs: 3 * 60 * 1000,
      };
    case "image":
      return {
        buttonName: /制作图片|Create image|Create images|Make image/i,
        selectedButtonName: /取消选择.?制作图片|取消选择\"制作图片\"|Deselect.*image|Cancel selection.*image/i,
        modeIndicator: /为图片选择风格|正在创建您的图片|Choose a style for your image|Creating your image/i,
        resultTimeoutMs: 4 * 60 * 1000,
      };
    case "music":
      return {
        buttonName: /创作音乐|Create music/i,
        modeIndicator: /制作音乐|创作音乐|Create music/i,
        resultTimeoutMs: 8 * 60 * 1000,
      };
    case "video":
      return {
        buttonName: /创作视频|制作视频|Create video/i,
        selectedButtonName: /取消选择.?创作视频|取消选择.?制作视频|Deselect.*video|Cancel selection.*video/i,
        modeIndicator: /挑选一个模板|开始制作你的视频|choose a template|start creating your video|视频生成模板图片|video generation template/i,
        resultTimeoutMs: 12 * 60 * 1000,
      };
    default:
      throw Object.assign(new Error(`Unsupported Gemini Canvas operation '${operation}'.`), {
        status: 400,
        code: "gemini_canvas_invalid_operation",
      });
  }
}

function isInterestingNetworkUrl(url) {
  const lowered = String(url || "").toLowerCase();
  return (
    lowered.includes("gemini.google.com") ||
    lowered.includes("generativelanguage.googleapis.com") ||
    lowered.includes("googleapis.com") ||
    lowered.includes("geminiweb-pa.clients6.google.com") ||
    lowered.includes("googleusercontent.com") ||
    lowered.includes("contribution.usercontent.google.com") ||
    lowered.includes("googlevideo.com") ||
    lowered.includes("gvt1.com")
  );
}

function isLikelyAvatarUrl(url) {
  const lowered = String(url || "").toLowerCase();
  return (
    lowered.includes("lh3.googleusercontent.com/a/") ||
    lowered.includes("lh3.googleusercontent.com/u/0/ogw") ||
    lowered.includes("=s64-")
  );
}

function isLikelyNoiseMediaUrl(url) {
  const lowered = String(url || "").toLowerCase();
  return (
    lowered.includes("googleadservices.com") ||
    lowered.includes("/pagead/") ||
    lowered.includes("1p-conversion") ||
    lowered.includes("doubleclick.net") ||
    lowered.includes("google-analytics.com")
  );
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
  if (lowered.includes(".mp4") || lowered.includes("filename=video.mp4") || lowered.includes("filename=ivory_rain.mp4")) {
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
  return /googlevideo|gvt1|\.mp4(\?|$)|\.webm(\?|$)|\.mov(\?|$)|filename=.*\.(mp4|webm|mov)(\b|$)/i.test(
    String(url || ""),
  );
}

function normalizeGeminiBrowserAssetUrl(url) {
  const value = normalizeString(url);
  if (!value) {
    return null;
  }
  try {
    const parsed = new URL(value);
    if (
      parsed.protocol === "http:" &&
      (
        /(^|\\.)googleusercontent\\.com$/i.test(parsed.hostname) ||
        /(^|\\.)googlevideo\\.com$/i.test(parsed.hostname) ||
        /(^|\\.)gvt1\\.com$/i.test(parsed.hostname)
      )
    ) {
      parsed.protocol = "https:";
      return parsed.toString();
    }
  } catch {
    // Keep the original value when URL parsing fails.
  }
  return value;
}

function pushUniqueMediaUrl(store, candidate) {
  const normalizedUrl = normalizeGeminiBrowserAssetUrl(candidate?.url);
  if (
    !normalizedUrl ||
    isLikelyAvatarUrl(normalizedUrl) ||
    isLikelyNoiseMediaUrl(normalizedUrl)
  ) {
    return;
  }
  if (store.some((entry) => entry.url === normalizedUrl)) {
    return;
  }
  store.push({
    ...candidate,
    url: normalizedUrl,
  });
}

const PROGRAM_APP_PATH_REGEX = /(?:https:\/\/gemini\.google\.com)?(\/app\/(?:[0-9a-f]{8,}|\d{13,}))/gi;
const PROGRAM_CONVERSATION_ID_REGEX = /\bc_([0-9a-f]{8,})\b/gi;
const PROGRAM_RESPONSE_ID_REGEX = /\br_([0-9a-f]{8,})\b/gi;
const PROGRAM_PAIR_REGEX =
  /(?:\\?")?(c_[0-9a-f]{8,})(?:\\?")?\s*,\s*(?:\\?")?(r_[0-9a-f]{8,})(?:\\?")?/gi;
const PROGRAM_SHARE_PATH_REGEX = /(\/share\/[0-9a-z]{8,})/gi;

function uniqueStrings(values) {
  return [...new Set((values || []).filter(Boolean).map((value) => String(value).trim()).filter(Boolean))];
}

function extractProgramAppPaths(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_APP_PATH_REGEX.exec(source)) !== null) {
    values.push(match[1]);
  }
  return [...new Set(values)];
}

function extractAppPath(url) {
  if (!url) {
    return null;
  }
  return extractProgramAppPaths(String(url))[0] ?? null;
}

function extractProgramConversationIds(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_CONVERSATION_ID_REGEX.exec(source)) !== null) {
    values.push(`c_${match[1]}`);
  }
  return [...new Set(values)];
}

function extractProgramResponseIds(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_RESPONSE_ID_REGEX.exec(source)) !== null) {
    values.push(`r_${match[1]}`);
  }
  return [...new Set(values)];
}

function extractProgramSharePaths(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_SHARE_PATH_REGEX.exec(source)) !== null) {
    values.push(match[1]);
  }
  return uniqueStrings(values);
}

function extractProgramHandleHintsFromText(text) {
  const appPaths = extractProgramAppPaths(text);
  const conversationIds = extractProgramConversationIds(text);
  const responseIds = extractProgramResponseIds(text);
  const sharePaths = extractProgramSharePaths(text);
  const derivedAppPaths = conversationIds.map((id) => `/app/${id.replace(/^c_/, "")}`);
  return {
    appPaths: uniqueStrings([...appPaths, ...derivedAppPaths]),
    conversationIds,
    responseIds,
    sharePaths,
  };
}

function mergeProgramHandleHints(target, incoming) {
  target.appPaths = uniqueStrings([...(target.appPaths || []), ...(incoming.appPaths || [])]);
  target.conversationIds = uniqueStrings([
    ...(target.conversationIds || []),
    ...(incoming.conversationIds || []),
  ]);
  target.responseIds = uniqueStrings([...(target.responseIds || []), ...(incoming.responseIds || [])]);
  target.sharePaths = uniqueStrings([...(target.sharePaths || []), ...(incoming.sharePaths || [])]);
}

function strongestProgramHandleHint(hints) {
  const reversedAppPaths = [...(hints?.appPaths || [])].reverse();
  const concreteAppPath = reversedAppPaths.find((value) =>
    /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value)),
  );
  if (concreteAppPath) {
    return concreteAppPath;
  }
  const reversedConversationIds = [...(hints?.conversationIds || [])].reverse();
  if (reversedConversationIds[0]) {
    return reversedConversationIds[0];
  }
  const reversedSharePaths = [...(hints?.sharePaths || [])].reverse();
  return reversedSharePaths[0] ?? null;
}

function deriveConversationIdForAppPath(appPath, conversationIds) {
  const suffix = String(appPath || "")
    .split("/")
    .filter(Boolean)
    .at(-1);
  if (!suffix) {
    return [...(conversationIds || [])].reverse()[0] ?? null;
  }
  const matched = [...(conversationIds || [])]
    .reverse()
    .find((value) => String(value).replace(/^c_/, "") === suffix);
  return matched ?? [...(conversationIds || [])].reverse()[0] ?? null;
}

function isInterestingMutation(url, method) {
  return String(method || "GET").toUpperCase() === "POST" &&
    (
      String(url || "").includes("generateContent") ||
      String(url || "").includes("streamGenerateContent") ||
      String(url || "").includes(":predict") ||
      String(url || "").includes("/predict") ||
      String(url || "").includes("imagen")
    );
}

function dedupeProgramHandlePairs(pairs) {
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

function readRpcIdFromRequestUrl(url) {
  try {
    return new URL(String(url || "")).searchParams.get("rpcids");
  } catch {
    return null;
  }
}

function extractProgramHandlePairs(text, source = {}) {
  const pairs = [];
  const input = String(text || "");
  let match;
  while ((match = PROGRAM_PAIR_REGEX.exec(input)) !== null) {
    const conversationId = String(match[1] || "").trim();
    const responseId = String(match[2] || "").trim();
    if (!/^c_[0-9a-f]{8,}$/i.test(conversationId) || !/^r_[0-9a-f]{8,}$/i.test(responseId)) {
      continue;
    }
    const appPath = `/app/${conversationId.replace(/^c_/, "")}`;
    pairs.push({
      appPath,
      programUrl: `https://gemini.google.com${appPath}`,
      conversationId,
      responseId,
      sourceUrl: source.sourceUrl || null,
      sourceRpc: source.sourceRpc || null,
      sourceKind: source.sourceKind || null,
      sourceSurface: source.sourceSurface || null,
      sourceWsUrl: source.sourceWsUrl || null,
      sourceTargetDomain: source.sourceTargetDomain || null,
      ts: source.ts || null,
    });
  }
  return dedupeProgramHandlePairs(pairs);
}

function isConcreteProgramUrlCandidate(candidate) {
  const value = normalizeString(candidate);
  if (!value) {
    return false;
  }
  try {
    const parsed = new URL(value);
    return /\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(parsed.pathname);
  } catch {
    return /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(value);
  }
}

function shouldCaptureProgramHandleTraffic(url, method, operation) {
  const normalizedMethod = String(method || "GET").toUpperCase();
  if (normalizedMethod !== "POST") {
    return false;
  }
  if (/\/StreamGenerate|\/batchexecute|assistant\.lamda\.BardFrontendService|clients6\.google\.com\/punctual/i.test(url)) {
    return true;
  }
  if (operation === "image" || operation === "music" || operation === "video" || operation === "bootstrap_program") {
    return isInterestingMutation(url, normalizedMethod);
  }
  return false;
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
  const rpcId = readRpcIdFromRequestUrl(url);
  if (rpcId && INTERESTING_PROGRAM_RPC_IDS.has(rpcId)) {
    return {
      rpcId,
      label: rpcId,
      sourcePath: (() => {
        try {
          return new URL(url).searchParams.get("source-path");
        } catch {
          return null;
        }
      })(),
      method: String(method || "GET").toUpperCase(),
    };
  }
  if (/\/StreamGenerate/i.test(url)) {
    return {
      rpcId: null,
      label: "StreamGenerate",
      sourcePath: null,
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

function startNetworkCapture(page, operation, existingState = null) {
  const state = existingState ?? {
    streamGenerateRequestAt: null,
    streamGenerateResponseAt: null,
    imageUrls: [],
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
    events: [],
  };

  const pushEvent = (event) => {
    state.events.push({
      t: Date.now(),
      ...event,
    });
    if (state.events.length > 160) {
      state.events.shift();
    }
  };

  const summarizeHeaders = (headers) =>
    Object.fromEntries(
      Object.entries(headers || {}).flatMap(([key, value]) => {
        const normalized = String(key || "").toLowerCase();
        if (![
          "authorization",
          "x-origin",
          "x-goog-authuser",
          "x-goog-api-key",
          "content-type",
          "origin",
          "referer",
        ].includes(normalized)) {
          return [];
        }
        if (normalized === "authorization" || normalized === "x-goog-api-key") {
          return [[normalized, value ? "<present>" : "<missing>"]];
        }
        return [[normalized, value]];
      }),
    );

  const onRequest = (request) => {
    const url = request.url();
    if (!isInterestingNetworkUrl(url)) {
      return;
    }
    if (
      request.method().toUpperCase() === "POST" &&
      url.includes("/StreamGenerate")
    ) {
      state.streamGenerateRequestAt = Date.now();
    }
    if (shouldCaptureProgramHandleTraffic(url, request.method(), operation)) {
      let postData = null;
      try {
        postData = request.postData();
      } catch {
        postData = null;
      }
      const rawHeaders = request.headers() || {};
      const requestCookieHeader = extractRequestCookieHeader(rawHeaders);
      const handleHints = extractProgramHandleHintsFromText(`${url}\n${postData || ""}`);
      mergeProgramHandleHints(state.handleHints, handleHints);
      mergeTransportHints(state.transportHints, extractTransportHintsFromNetworkUrl(url));
      mergeActionContract(
        state.actionContract,
        extractCanvasProgramActionContractFromText(`${url}\n${postData || ""}`),
      );
      state.invokeContract = mergeInvokeContract(
        state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          state.actionContract,
          state.transportHints,
          { bodyText: postData ?? "", buttons: [] },
          null,
          state,
        ),
      );
      state.handlePairs = dedupeProgramHandlePairs([
        ...state.handlePairs,
        ...extractProgramHandlePairs(`${url}\n${postData || ""}`, {
          sourceKind: "request",
          sourceUrl: url,
          sourceRpc: readRpcIdFromRequestUrl(url),
          ...classifyHandlePairSurface(`${url}\n${postData || ""}`),
          ts: new Date().toISOString(),
        }),
      ]);
      pushEvent({
        type: "request",
        method: request.method(),
        url,
        headers: summarizeHeaders(request.headers()),
        postData:
          typeof postData === "string" && postData.length > 6000
            ? `${postData.slice(0, 6000)}...[truncated]`
            : postData,
      });
      const rpcCapture = classifyProgramRpcCapture(url, request.method());
      if (rpcCapture) {
        pushProgramRpcCapture(state, {
          type: "request",
          ...rpcCapture,
          url,
          cookieHeader: requestCookieHeader,
          headers: summarizeHeaders(request.headers()),
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
    }
  };

  const onResponse = async (response) => {
    const url = response.url();
    if (!isInterestingNetworkUrl(url)) {
      return;
    }
    const request = response.request();
    const shouldCaptureHandleTraffic = shouldCaptureProgramHandleTraffic(
      url,
      request.method(),
      operation,
    );
    if (url.includes("/StreamGenerate")) {
      state.streamGenerateResponseAt = Date.now();
    }

    const contentType = response.headers()["content-type"] ?? "";
    if (shouldCaptureHandleTraffic) {
      let text = null;
      try {
        text = await response.text();
      } catch (error) {
        text = `[[response text unavailable: ${error instanceof Error ? error.message : String(error)}]]`;
      }
      const handleHints = extractProgramHandleHintsFromText(`${url}\n${text || ""}`);
      mergeProgramHandleHints(state.handleHints, handleHints);
      mergeTransportHints(state.transportHints, extractTransportHintsFromNetworkUrl(url));
      mergeActionContract(
        state.actionContract,
        extractCanvasProgramActionContractFromText(`${url}\n${text || ""}`),
      );
      state.invokeContract = mergeInvokeContract(
        state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          state.actionContract,
          state.transportHints,
          { bodyText: text ?? "", buttons: [] },
          null,
          state,
        ),
      );
      state.handlePairs = dedupeProgramHandlePairs([
        ...state.handlePairs,
        ...extractProgramHandlePairs(`${url}\n${text || ""}`, {
          sourceKind: "response",
          sourceUrl: url,
          sourceRpc: readRpcIdFromRequestUrl(url),
          ...classifyHandlePairSurface(`${url}\n${text || ""}`),
          ts: new Date().toISOString(),
        }),
      ]);
      pushEvent({
        type: "response",
        status: response.status(),
        url,
        requestHeaders: summarizeHeaders(request.headers()),
        responseHeaders: summarizeHeaders(response.headers()),
        text:
          typeof text === "string" && text.length > 12000
            ? `${text.slice(0, 12000)}...[truncated]`
            : text,
      });
      const rpcCapture = classifyProgramRpcCapture(url, request.method());
      if (rpcCapture) {
        pushProgramRpcCapture(state, {
          type: "response",
          ...rpcCapture,
          url,
          status: response.status(),
          requestHeaders: summarizeHeaders(request.headers()),
          responseHeaders: summarizeHeaders(response.headers()),
          bodyText: trimProgramRpcCaptureText(text),
        });
      }
    }

    if (operation === "image") {
      if (
        (/image\//i.test(contentType) || /gg-dl|rd-gg-dl|googleusercontent/i.test(url)) &&
        !isLikelyAvatarUrl(url)
      ) {
        let bodyBase64 = null;
        if (/image\//i.test(contentType)) {
          try {
            const bytes = await response.body();
            if (bytes?.length) {
              bodyBase64 = Buffer.from(bytes).toString("base64");
            }
          } catch {
            bodyBase64 = null;
          }
        }
        pushUniqueMediaUrl(state.imageUrls, {
          url,
          mimeType: inferMimeTypeFromUrl(url, contentType.split(";")[0] || "image/png"),
          bodyBase64,
        });
      }
      return;
    }

    if (operation === "tts" || operation === "music") {
      if (isAudioLikeMimeType(contentType) || isAudioLikeUrl(url)) {
        let bodyBase64 = null;
        if (isAudioLikeMimeType(contentType)) {
          try {
            const bytes = await response.body();
            if (bytes?.length) {
              bodyBase64 = Buffer.from(bytes).toString("base64");
            }
          } catch {
            bodyBase64 = null;
          }
        }
        pushUniqueMediaUrl(state.audioUrls, {
          url,
          mimeType: inferMimeTypeFromUrl(url, contentType.split(";")[0] || "audio/wav"),
          bodyBase64,
        });
      }
    }

    if (operation === "video" || operation === "music") {
      if (
        /video\//i.test(contentType) ||
        /contribution\.usercontent\.google\.com|googlevideo\.com|gvt1\.com|\.mp4(\?|$)|\.webm(\?|$)/i.test(url)
      ) {
        pushUniqueMediaUrl(state.videoUrls, {
          url,
          mimeType: inferMimeTypeFromUrl(url, contentType.split(";")[0] || "video/mp4"),
        });
      }
    }
  };

  const onWebSocket = (websocket) => {
    const url = websocket.url();
    mergeTransportHints(state.transportHints, extractTransportHintsFromNetworkUrl(url));
    pushEvent({
      type: "websocket",
      url,
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

async function collectProgramHandleSnapshot(page) {
  const snapshot = await page.evaluate(() => {
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
      url: location.href,
      title: document.title,
      bodyText: (bodyText || "").slice(0, 12000),
      historyState: typeof historyState === "string" ? historyState.slice(0, 4000) : null,
      buttons,
      anchors,
      mediaNodes,
      textboxes,
    };
  });

  const handleHints = extractProgramHandleHintsFromText(
    [
      snapshot.url,
      snapshot.historyState,
      snapshot.bodyText,
      ...(snapshot.anchors || []).flatMap((anchor) => [
        anchor.href,
        anchor.text,
        anchor.ariaLabel,
        anchor.title,
      ]),
    ]
      .filter(Boolean)
      .join("\n"),
  );

  return {
    ...snapshot,
    handleHints,
  };
}

function selectStableProgramPair(handlePairs, requestedAppPath = null) {
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
  const requestedMatch =
    requestedAppPath &&
    reversed.find(
      (pair) =>
        pair.appPath === requestedAppPath &&
        /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
    );
  if (requestedMatch) {
    return requestedMatch;
  }
  const sharePair = reversed.find((pair) =>
    /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
  );
  if (sharePair) {
    return sharePair;
  }
  return reversed.find((pair) => pair.appPath && pair.conversationId && pair.responseId) ?? null;
}

function concreteAppPathFromUrl(url) {
  const matched = extractProgramAppPaths(url || "")[0] ?? null;
  return matched && /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(matched) ? matched : null;
}

function selectCanonicalProgramPair(handlePairs, pageUrl, requestedAppPath = null) {
  const stableProgramPair = selectStableProgramPair(handlePairs, requestedAppPath);
  const latestResponsePair =
    [...(handlePairs || [])].reverse().find((pair) => pair.conversationId && pair.responseId) ?? null;
  const finalAppPath = concreteAppPathFromUrl(pageUrl);
  if (
    finalAppPath &&
    latestResponsePair?.appPath === finalAppPath &&
    latestResponsePair?.conversationId &&
    latestResponsePair?.responseId
  ) {
    return latestResponsePair;
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
  return stableProgramPair ?? latestResponsePair;
}

function hasCanvasProxyProgramCandidate(handlePairs, invokeContract = null) {
  if (invokeContract?.transportKind === "canvas_program_ws_candidate") {
    return true;
  }
  return [...(handlePairs || [])].some((pair) => pair?.sourceSurface === "canvas_proxy_client");
}

function buildProgramHandleState(baseUrl, args, pageUrl, captureState) {
  const candidatePairs = dedupeProgramHandlePairs(captureState?.handlePairs || []);
  const requestedProgramUrl = resolveProgramPageUrl(baseUrl, args);
  const requestedAppPath =
    extractProgramAppPaths(requestedProgramUrl || "")[0] ||
    normalizeString(args?.appPath) ||
    null;
  const handleHints = captureState?.handleHints || {
    appPaths: [],
    conversationIds: [],
    responseIds: [],
    sharePaths: [],
  };
  const transportHints = captureState?.transportHints || {
    invokeBaseUrls: [],
    musicWsUrls: [],
    videoInvokePaths: [],
  };
  const stableProgramPair = selectStableProgramPair(candidatePairs, requestedAppPath);
  const latestResponsePair =
    [...candidatePairs].reverse().find((pair) => pair.conversationId && pair.responseId) ?? null;
  const canonicalProgramPair =
    selectCanonicalProgramPair(candidatePairs, pageUrl, requestedAppPath) ?? stableProgramPair;
  const finalAppPath = concreteAppPathFromUrl(pageUrl);
  const hintedAppPath =
    [...(handleHints.appPaths || [])]
      .reverse()
      .find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ?? null;
  const effectiveAppPath =
    finalAppPath ?? canonicalProgramPair?.appPath ?? requestedAppPath ?? hintedAppPath ?? null;
  return {
    canvasProgramUrl:
      canonicalProgramPair?.programUrl ??
      (effectiveAppPath ? `${baseUrl.replace(/\/+$/, "")}${effectiveAppPath}` : null) ??
      (isConcreteProgramUrlCandidate(requestedProgramUrl) ? requestedProgramUrl : null),
    pageUrl: pageUrl || null,
    appPath: effectiveAppPath,
    conversationId:
      canonicalProgramPair?.conversationId ??
      normalizeString(args?.conversationId) ??
      deriveConversationIdForAppPath(effectiveAppPath, handleHints.conversationIds) ??
      null,
    responseId: canonicalProgramPair?.responseId ?? [...(handleHints.responseIds || [])].reverse()[0] ?? null,
    lastSeenConversationId:
      latestResponsePair?.conversationId ?? [...(handleHints.conversationIds || [])].reverse()[0] ?? null,
    lastSeenResponseId:
      latestResponsePair?.responseId ?? [...(handleHints.responseIds || [])].reverse()[0] ?? null,
    invokeBaseUrl: [...(transportHints.invokeBaseUrls || [])].reverse()[0] ?? null,
    musicWsUrl: [...(transportHints.musicWsUrls || [])].reverse()[0] ?? null,
    videoInvokePath: [...(transportHints.videoInvokePaths || [])].reverse()[0] ?? null,
    candidatePairs,
    stableProgramPair,
    latestResponsePair,
    capturedAt: new Date().toISOString(),
    lastValidatedAt: new Date().toISOString(),
  };
}

function normalizeBootstrapOperation(value) {
  const normalized = normalizeString(value)?.toLowerCase() ?? null;
  switch (normalized) {
    case "text":
    case "tts":
      return "text";
    case "image":
    case "music":
    case "video":
      return normalized;
    default:
      return "image";
  }
}

function defaultBootstrapPrompt(operation) {
  const suffix = Date.now();
  switch (operation) {
    case "text":
      return `CANVAS_PROGRAM_BOOTSTRAP_TEXT_${suffix} 请只回复 ok`;
    case "music":
      return `CANVAS_PROGRAM_BOOTSTRAP_MUSIC_${suffix} 一段简短电子提示音，节奏清晰。`;
    case "video":
      return `CANVAS_PROGRAM_BOOTSTRAP_VIDEO_${suffix} 3秒发光立方体旋转动画，干净背景。`;
    case "image":
    default:
      return `CANVAS_PROGRAM_BOOTSTRAP_IMAGE_${suffix} 一枚发光霓虹徽章，中央写着 CANVAS PROGRAM，科技感，高清细节，Requested aspect ratio: 1:1.`;
  }
}

function hasConcreteProgramHandleState(state) {
  return Boolean(
    normalizeString(state?.canvasProgramUrl) &&
      normalizeString(state?.appPath) &&
      normalizeString(state?.conversationId),
  );
}

async function clickOperationMode(page, operation, timeoutMs) {
  const config = operationConfig(operation);
  if (!config.buttonName) {
    return false;
  }
  try {
    await dismissGeminiAppInterstitials(page, timeoutMs);
    if (await operationModeAppearsSelected(page, config, Math.min(timeoutMs, 8_000))) {
      return true;
    }
  } catch {
    // keep trying explicit mode buttons below
  }
  const candidates = [
    page.getByRole("button", { name: config.buttonName }).first(),
    page.getByRole("link", { name: config.buttonName }).first(),
    page.getByRole("menuitem", { name: config.buttonName }).first(),
    page.locator(`[aria-label*="${operation}"], [title*="${operation}"]`).first(),
    page.getByText(config.buttonName).first(),
  ];

  for (const candidate of candidates) {
    try {
      await dismissGeminiAppInterstitials(page, timeoutMs);
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 8_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 15_000), force: true });
      await page.waitForTimeout(1200);
      if (await operationModeAppearsSelected(page, config, timeoutMs)) {
        return true;
      }
      if (["image", "music", "video"].includes(operation)) {
        return true;
      }
    } catch {
      // keep trying other selectors
    }
  }

  if (await openOperationModeSelector(page, timeoutMs)) {
    const selectorCandidates = [
      page.getByRole("menuitem", { name: config.buttonName }).first(),
      page.getByRole("button", { name: config.buttonName }).first(),
      page.getByRole("link", { name: config.buttonName }).first(),
      page.locator('button,[role="button"],a,[role="menuitem"],div,span').filter({ hasText: config.buttonName }).first(),
      page.getByText(config.buttonName).first(),
    ];
    for (const candidate of selectorCandidates) {
      try {
        await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 8_000) });
        await candidate.click({ timeout: Math.min(timeoutMs, 15_000), force: true });
        await page.waitForTimeout(1200);
        if (await operationModeAppearsSelected(page, config, timeoutMs)) {
          return true;
        }
        if (["image", "music", "video"].includes(operation)) {
          return true;
        }
      } catch {
        // try next menu candidate
      }
    }
    try {
      const fallbackClicked = await page.evaluate(({ pattern, flags }) => {
        const matcher = new RegExp(pattern, flags);
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
          return (
            node.matches('button,[role="button"],a,[role="menuitem"],[tabindex]') ||
            typeof node.onclick === "function" ||
            window.getComputedStyle(node).cursor === "pointer"
          );
        };
        const clickNode = (node) => {
          node.scrollIntoView({ block: "center", inline: "center" });
          node.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true }));
          node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
          node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
          node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
        };
        const nodes = Array.from(
          document.querySelectorAll('button,[role="button"],a,[role="menuitem"],div,span'),
        );
        for (const node of nodes) {
          const text = (node.innerText || "").trim();
          if (!text || !matcher.test(text) || !isVisible(node)) {
            continue;
          }
          let clickableNode = node;
          let depth = 0;
          while (clickableNode && depth < 8 && !isClickable(clickableNode)) {
            clickableNode = clickableNode.parentElement;
            depth += 1;
          }
          if (!clickableNode || !isVisible(clickableNode)) {
            continue;
          }
          clickNode(clickableNode);
          return {
            clicked: true,
            text,
            targetTag: clickableNode.tagName || null,
            targetRole: clickableNode.getAttribute?.("role") || null,
            targetAria: clickableNode.getAttribute?.("aria-label") || null,
          };
        }
        return { clicked: false };
      }, { pattern: config.buttonName.source, flags: config.buttonName.flags });
      if (fallbackClicked?.clicked) {
        log("media operation mode dom fallback clicked", JSON.stringify({ operation, fallbackClicked }));
        await page.waitForTimeout(1200);
        if (await operationModeAppearsSelected(page, config, timeoutMs)) {
          return true;
        }
        if (["image", "music", "video"].includes(operation)) {
          return true;
        }
      }
    } catch {
      // fall through to final false below
    }
  }

  return false;
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
      // try next candidate
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

async function openOperationModeSelector(page, timeoutMs) {
  const selectorCandidates = [
    page.getByRole("button", { name: /工具|Tools/i }).first(),
    page.locator('button,[role="button"],a,[role="tab"]').filter({ hasText: /工具|Tools/i }).first(),
    page.getByRole("button", { name: /打开模式选择器|Open mode selector/i }).first(),
    page.getByRole("button", { name: /快速|Fast/i }).first(),
    page.getByRole("button", { name: /模式选择器|Mode selector/i }).first(),
  ];
  for (const candidate of selectorCandidates) {
    try {
      await dismissGeminiAppInterstitials(page, timeoutMs);
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 5_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000), force: true });
      await page.waitForTimeout(800);
      return true;
    } catch {
      // try next selector
    }
  }
  return false;
}

async function operationModeAppearsSelected(page, config, timeoutMs) {
  const selectedButtonName = config?.selectedButtonName ?? null;
  const modeIndicator = config?.modeIndicator ?? null;
  if (!selectedButtonName && !modeIndicator) {
    return true;
  }
  const deadline = Date.now() + Math.min(timeoutMs, 8_000);
  while (Date.now() < deadline) {
    if (selectedButtonName) {
      const selectedCandidates = [
        page.getByRole("button", { name: selectedButtonName }).first(),
        page.getByRole("link", { name: selectedButtonName }).first(),
      ];
      for (const candidate of selectedCandidates) {
        try {
          if (await candidate.isVisible({ timeout: 300 })) {
            return true;
          }
        } catch {
          // ignore and continue
        }
      }
    }
    try {
      const bodyText = await page.evaluate(() => document.body?.innerText ?? "");
      if (modeIndicator.test(String(bodyText || ""))) {
        return true;
      }
    } catch {
      // ignore and retry
    }
    await page.waitForTimeout(350);
  }
  return false;
}

async function submitPrompt(page, prompt, timeoutMs) {
  const textbox = page
    .locator(
      '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
    )
    .first();
  await textbox.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 30_000) });
  await textbox.focus();
  let populated = false;
  try {
    await textbox.evaluate((node, value) => {
      const textValue = String(value ?? "");
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
        node.innerHTML = "";
        node.textContent = textValue;
        fireInput();
        return true;
      }
      return false;
    }, prompt);
    populated = true;
  } catch {
    populated = false;
  }

  if (!populated) {
    await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
    await page.keyboard.press("Backspace").catch(() => undefined);
    await page.keyboard.type(prompt, { delay: 14 });
  }
  await page.waitForTimeout(300);

  const sendCandidates = [
    page.getByRole("button", { name: /发送|Send/i }).first(),
    page.locator('button[aria-label*="Send"], button[aria-label*="发送"], button[title*="Send"], button[title*="发送"]').first(),
    page.locator('button:has(svg), button:has(i)').last(),
  ];

  let clicked = false;
  for (const candidate of sendCandidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 4_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000), force: true });
      clicked = true;
      break;
    } catch {
      // try the next candidate
    }
  }

  if (!clicked) {
    const modifier = process.platform === "win32" ? "Control" : "Meta";
    await page.keyboard.press(`${modifier}+Enter`).catch(() => undefined);
    await page.waitForTimeout(200);
    await page.keyboard.press("Enter").catch(() => undefined);
  }
}

async function runBootstrapProgramOperation(entry, args) {
  const baseUrl = normalizeString(args.baseUrl) ?? "https://gemini.google.com";
  const shareId = normalizeString(args.shareId);
  const directProgramPageUrl = resolveProgramPageUrl(baseUrl, args);
  const preferExistingProgramPage =
    parseBoolean(String(args.preferExistingProgramPage ?? ""), false) ||
    (!shareId && Boolean(directProgramPageUrl));
  const discoveryOnly = parseBoolean(String(args.discoveryOnly ?? ""), true);
  const launchCanvasProxyPreview =
    discoveryOnly && parseBoolean(String(args.launchCanvasProxyPreview ?? ""), false);
  if (!shareId && !directProgramPageUrl) {
    throw Object.assign(
      new Error("Gemini Canvas program bootstrap requires either shareId or a concrete program page."),
      {
        status: 400,
        code: "gemini_canvas_missing_share_id",
      },
    );
  }

  const bootstrapOperation = normalizeBootstrapOperation(
    args.bootstrapOperation ?? args.targetOperation ?? args.operation,
  );
  const timeoutMs = Math.max(
    Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
    operationConfig(bootstrapOperation).resultTimeoutMs,
  );
  const shareUrl = shareId ? `${baseUrl.replace(/\/+$/, "")}/share/${shareId}` : null;
  const bootstrapPrompt =
    normalizeString(args.bootstrapPrompt) ??
    defaultBootstrapPrompt(bootstrapOperation);
  const aggregateHints = {
    appPaths: [],
    conversationIds: [],
    responseIds: [],
    sharePaths: [],
  };
  let shareMaterializationRetry = null;

  let activePage = entry.page;
  let capture = startNetworkCapture(activePage, bootstrapOperation);
  const adoptActivePage = async (nextPage) => {
    if (!nextPage || nextPage === activePage) {
      return;
    }
    capture.stop();
    capture = startNetworkCapture(nextPage, bootstrapOperation, capture.state);
    const previousPage = activePage;
    activePage = nextPage;
    entry.page = nextPage;
    if (previousPage && previousPage !== nextPage && !previousPage.isClosed()) {
      await previousPage.close().catch(() => undefined);
    }
  };

  try {
    let shareFollow = {
      kind: preferExistingProgramPage ? "direct_program_page" : "unattempted",
      buttonText: null,
      samePage: false,
      page: null,
    };

    if (preferExistingProgramPage) {
      await ensureProgramPage(entry, baseUrl, directProgramPageUrl, timeoutMs);
      activePage = entry.page;
      await activePage.waitForTimeout(2000);
    } else {
      await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
      await waitForShareSurface(activePage, Math.min(timeoutMs, 20_000));
      await activePage.waitForTimeout(1500);
      const shareBefore = await collectProgramHandleSnapshot(activePage);
      mergeProgramHandleHints(aggregateHints, shareBefore.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(shareBefore.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          bootstrapOperation,
          capture.state.actionContract,
          capture.state.transportHints,
          shareBefore,
          bootstrapPrompt,
          capture.state,
        ),
      );

      shareFollow = await tryFollowShareEntryPoint(activePage);
      if (shareFollow.page) {
        await adoptActivePage(shareFollow.page);
      }
      await activePage.waitForTimeout(3000);
      const shareAfter = await collectProgramHandleSnapshot(activePage);
      mergeProgramHandleHints(aggregateHints, shareAfter.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(shareAfter.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          bootstrapOperation,
          capture.state.actionContract,
          capture.state.transportHints,
          shareAfter,
          bootstrapPrompt,
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

      if (
        !discoveryOnly &&
        !(await hasPromptTextbox(activePage)) &&
        !strongestProgramHandleHint(aggregateHints)
      ) {
        entry.page = activePage;
        await ensureAppPage(entry, baseUrl, timeoutMs);
        activePage = entry.page;
        await activePage.waitForTimeout(2000);
      }

      if (activePage.url().includes("/share/") && !(await hasPromptTextbox(activePage))) {
        throw Object.assign(
          new Error("Gemini Canvas share page did not materialize an editable program context."),
          {
            status: 502,
            code: "gemini_canvas_program_bootstrap_share_follow_failed",
          },
        );
      }
    }

    const before = await collectProgramHandleSnapshot(activePage);
    mergeProgramHandleHints(aggregateHints, before.handleHints);
    mergeActionContract(
      capture.state.actionContract,
      extractCanvasProgramActionContractFromText(before.bodyText),
    );
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        bootstrapOperation,
        capture.state.actionContract,
        capture.state.transportHints,
        before,
        bootstrapPrompt,
        capture.state,
      ),
    );

    if (!discoveryOnly && !(await hasPromptTextbox(activePage))) {
      await clickOperationMode(activePage, bootstrapOperation, timeoutMs).catch(() => false);
      await activePage.waitForTimeout(2000);
    }

    const shouldStayOnProxyDiscoverySurface =
      discoveryOnly &&
      hasCanvasProxyProgramCandidate(capture.state.handlePairs, capture.state.invokeContract);
    if (
      shouldStayOnProxyDiscoverySurface &&
      !/Browser API Proxy Client/i.test(String(before.bodyText || ""))
    ) {
      shareMaterializationRetry = {
        attempted: true,
        initialPageUrl: activePage.url(),
        initialBodyPreview: String(before.bodyText || "").slice(0, 400),
      };
      await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
      await waitForShareSurface(activePage, Math.min(timeoutMs, 20_000));
      await activePage.waitForTimeout(1500);
      const retryFollow = await tryFollowShareEntryPoint(activePage);
      if (retryFollow.page) {
        await adoptActivePage(retryFollow.page);
      }
      await activePage.waitForTimeout(3000);
      const retrySnapshot = await collectProgramHandleSnapshot(activePage);
      mergeProgramHandleHints(aggregateHints, retrySnapshot.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(retrySnapshot.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          bootstrapOperation,
          capture.state.actionContract,
          capture.state.transportHints,
          retrySnapshot,
          bootstrapPrompt,
          capture.state,
        ),
      );
      shareMaterializationRetry.shareFollowKind = retryFollow.kind ?? "unknown";
      shareMaterializationRetry.retryPageUrl = retrySnapshot.url;
      shareMaterializationRetry.retryBodyPreview = String(retrySnapshot.bodyText || "").slice(0, 400);
      shareMaterializationRetry.proxyVisibleAfterRetry = /Browser API Proxy Client/i.test(
        String(retrySnapshot.bodyText || ""),
      );
      log(
        "canvas proxy share materialization retry",
        JSON.stringify(shareMaterializationRetry),
      );
    }
    let canvasProxyPreview = null;
    let previewSnapshot = null;
    if (launchCanvasProxyPreview && shouldStayOnProxyDiscoverySurface) {
      canvasProxyPreview = await tryOpenCanvasProxyPreview(activePage, timeoutMs).catch((error) => ({
        clicked: false,
        reason: "preview_open_error",
        errorMessage: error instanceof Error ? error.message : String(error),
      }));
      const afterPreview = await collectProgramHandleSnapshot(activePage);
      mergeProgramHandleHints(aggregateHints, afterPreview.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(afterPreview.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          bootstrapOperation,
          capture.state.actionContract,
          capture.state.transportHints,
          afterPreview,
          bootstrapPrompt,
          capture.state,
        ),
      );
      previewSnapshot = afterPreview;
      if (Number(canvasProxyPreview?.bridge?.eventCount ?? 0) <= 1) {
        const stampedFrames = await stampCanvasProxyPreviewFrames(
          activePage,
          inferGoogleAuthUser(activePage.url()),
        ).catch(() => []);
        canvasProxyPreview = {
          ...(canvasProxyPreview || {}),
          stampedFrames,
        };
        await activePage.waitForTimeout(2500);
        const afterStamp = await collectProgramHandleSnapshot(activePage);
        mergeProgramHandleHints(aggregateHints, afterStamp.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(afterStamp.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            bootstrapOperation,
            capture.state.actionContract,
            capture.state.transportHints,
            afterStamp,
            bootstrapPrompt,
            capture.state,
          ),
        );
        previewSnapshot = afterStamp;
      }
      const hasCapturedCanvasProxyHtml = Boolean(
        extractCanvasProxyClientHtmlFromTexts(
          collectCanvasProxyContractTexts(previewSnapshot ?? afterPreview, capture.state),
        ),
      );
      const previewBridgeReportsAuthIndexFailure =
        Array.isArray(canvasProxyPreview?.bridgeEvents) &&
        canvasProxyPreview.bridgeEvents.some(
          (event) =>
            event?.type === "error" &&
            /authIndex postMessage timeout/i.test(String(event?.errorMessage ?? event?.messagePreview ?? "")),
        );
      const shouldAttemptDirectCanvasProxyLaunch =
        hasCapturedCanvasProxyHtml &&
        (
          previewBridgeReportsAuthIndexFailure ||
          (
            Number(canvasProxyPreview?.bridge?.eventCount ?? 0) <= 1 &&
            !(Array.isArray(canvasProxyPreview?.bridgeEvents) && canvasProxyPreview.bridgeEvents.length > 1)
          )
        ) &&
        !/System Logs Output|Connecting\.\.\.|Connected|Disconnected/i.test(
          String(afterPreview.bodyText || ""),
        );
      if (shouldAttemptDirectCanvasProxyLaunch) {
        const directLaunch = await tryLaunchCanvasProxyClientFromCapturedHtml(
          entry.context,
          activePage,
          afterPreview,
          capture.state,
          timeoutMs,
        );
        const adoptedLaunchPage = directLaunch?.page ?? null;
        canvasProxyPreview = {
          ...(canvasProxyPreview || {}),
          directLaunch: directLaunch
            ? Object.fromEntries(
                Object.entries(directLaunch).filter(([key]) => key !== "page"),
              )
            : null,
        };
        if (adoptedLaunchPage) {
          await adoptActivePage(adoptedLaunchPage);
          previewSnapshot = await collectProgramHandleSnapshot(activePage);
        }
      }
      log(
        "canvas proxy preview probe",
        JSON.stringify({
          operation: bootstrapOperation,
          preview: canvasProxyPreview,
          pageUrl: previewSnapshot?.url ?? afterPreview.url,
          bodyPreview: String((previewSnapshot?.bodyText ?? afterPreview.bodyText) || "").slice(0, 400),
        }),
      );
      if (discoveryOnly && canvasProxyPreview?.directLaunch?.launched) {
        const previewHandleState = buildProgramHandleState(
          baseUrl,
          args,
          previewSnapshot?.url ?? afterPreview.url,
          capture.state,
        );
        const previewAppPath =
          previewHandleState.appPath ??
          [...aggregateHints.appPaths]
            .reverse()
            .find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ??
          null;
        const previewCanvasProgramUrl =
          previewHandleState.canvasProgramUrl ??
          (previewAppPath ? `${baseUrl.replace(/\/+$/, "")}${previewAppPath}` : null);
        const previewConversationId =
          previewHandleState.conversationId ??
          deriveConversationIdForAppPath(previewAppPath, aggregateHints.conversationIds);
        const previewResponseId =
          previewHandleState.responseId ??
          [...aggregateHints.responseIds].reverse()[0] ??
          null;
        const previewActionContract = {
          canvasProgramAction:
            capture.state.actionContract.canvasProgramAction ??
            extractCanvasProgramActionContractFromText(previewSnapshot?.bodyText)?.canvasProgramAction ??
            null,
          canvasProgramActionInput:
            capture.state.actionContract.canvasProgramActionInput ??
            extractCanvasProgramActionContractFromText(previewSnapshot?.bodyText)?.canvasProgramActionInput ??
            null,
        };
        const previewInvokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            bootstrapOperation,
            previewActionContract,
            capture.state.transportHints,
            previewSnapshot ?? afterPreview,
            bootstrapPrompt,
            capture.state,
          ),
        );
        if (!previewCanvasProgramUrl || !previewAppPath || !previewConversationId) {
          throw Object.assign(
            new Error("Gemini Canvas program direct launch completed without a concrete program handle."),
            {
              status: 504,
              code: "gemini_canvas_program_bootstrap_handle_missing",
              bodyText: previewSnapshot?.bodyText ?? afterPreview.bodyText ?? null,
            },
          );
        }
        return {
          operation: "bootstrap_program",
          runtimeStateObjectKey: normalizeString(args.runtimeStateObjectKey),
          shareUrl,
          shareId,
          shareFollowKind: shareFollow.kind,
          bootstrapOperation,
          bootstrapPrompt,
          discoveryOnly,
          beforeUrl: before.url,
          finalUrl: previewSnapshot?.url ?? afterPreview.url,
          pageUrl: previewSnapshot?.url ?? afterPreview.url,
          bodyText: previewSnapshot?.bodyText ?? afterPreview.bodyText ?? null,
          canvasProxyPreview,
          shareMaterializationRetry,
          newChatClicked: false,
          modeSelected: false,
          canvasProgramUrl: previewCanvasProgramUrl,
          appPath: previewAppPath,
          conversationId: previewConversationId,
          responseId: previewResponseId,
          lastSeenConversationId:
            previewHandleState.lastSeenConversationId ??
            [...aggregateHints.conversationIds].reverse()[0] ??
            null,
          lastSeenResponseId:
            previewHandleState.lastSeenResponseId ??
            [...aggregateHints.responseIds].reverse()[0] ??
            null,
          candidatePairs: previewHandleState.candidatePairs,
          stableProgramPair: previewHandleState.stableProgramPair,
          latestResponsePair: previewHandleState.latestResponsePair,
          aggregateHints,
          transportHints: capture.state.transportHints,
          invokeBaseUrl: previewHandleState.invokeBaseUrl,
          musicWsUrl: previewHandleState.musicWsUrl,
          videoInvokePath: previewHandleState.videoInvokePath,
          canvasProgramAction: previewActionContract.canvasProgramAction,
          canvasProgramActionInput: previewActionContract.canvasProgramActionInput,
          canvasProgramInvokeContract: previewInvokeContract,
          networkEvents: capture.state.events,
        };
      }
    }
    const newChatClicked = shouldStayOnProxyDiscoverySurface ? false : await clickNewChat(activePage);
    const modeSelected = shouldStayOnProxyDiscoverySurface
      ? false
      : bootstrapOperation === "text"
        ? true
        : await clickOperationMode(activePage, bootstrapOperation, timeoutMs);
    const afterNewChat = shouldStayOnProxyDiscoverySurface
      ? before
      : await collectProgramHandleSnapshot(activePage);
    mergeProgramHandleHints(aggregateHints, afterNewChat.handleHints);
    mergeActionContract(
      capture.state.actionContract,
      extractCanvasProgramActionContractFromText(afterNewChat.bodyText),
    );
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        bootstrapOperation,
        capture.state.actionContract,
        capture.state.transportHints,
        afterNewChat,
        bootstrapPrompt,
        capture.state,
      ),
    );

    if (
      !discoveryOnly &&
      bootstrapOperation === "music" &&
      /选择要混合制作的曲目/i.test(afterNewChat.bodyText)
    ) {
      const styleSelection = await trySelectMusicStyleCard(activePage, timeoutMs).catch(() => null);
      if (styleSelection?.clicked) {
        const afterStyleSelection = await collectProgramHandleSnapshot(activePage);
        mergeProgramHandleHints(aggregateHints, afterStyleSelection.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(afterStyleSelection.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            bootstrapOperation,
            capture.state.actionContract,
            capture.state.transportHints,
            afterStyleSelection,
            bootstrapPrompt,
            capture.state,
          ),
        );
      }
    }

    if (!discoveryOnly) {
      await submitPrompt(activePage, bootstrapPrompt, timeoutMs);
    }

    const deadline = Date.now() + timeoutMs;
    let lastSnapshot = previewSnapshot ?? afterNewChat;
    let concreteHandleReadyAt = null;
    let acceptedProgressReadyAt = null;
    while (Date.now() < deadline) {
      await activePage.waitForTimeout(1800);
      lastSnapshot = await collectProgramHandleSnapshot(activePage);
      mergeProgramHandleHints(aggregateHints, lastSnapshot.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          bootstrapOperation,
          capture.state.actionContract,
          capture.state.transportHints,
          lastSnapshot,
          bootstrapPrompt,
          capture.state,
        ),
      );
      const currentHandleState = buildProgramHandleState(
        baseUrl,
        args,
        lastSnapshot.url,
        capture.state,
      );
      if (hasConcreteProgramHandleState(currentHandleState)) {
        if (!concreteHandleReadyAt) {
          concreteHandleReadyAt = Date.now();
        }
        const invokeReady =
          invokeContractIndicatesConcreteProgress(
            bootstrapOperation,
            capture.state.invokeContract,
            lastSnapshot,
          );
        if (invokeReady && !acceptedProgressReadyAt) {
          acceptedProgressReadyAt = Date.now();
        }
        if (
          discoveryOnly &&
          (hasTransportHints(capture.state.transportHints) ||
            Boolean(capture.state.invokeContract?.transportKind) ||
            Boolean(capture.state.invokeContract?.actionName) ||
            Boolean(capture.state.invokeContract?.uiState))
        ) {
          break;
        }
        if (
          !discoveryOnly &&
          (
            bootstrapOperation === "text" ||
            bootstrapOperation === "image" ||
            hasTransportHints(capture.state.transportHints) ||
            (
              invokeReady &&
              (
                bootstrapOperation !== "music" ||
                capture.state.invokeContract?.uiState === "music_player_ready" ||
                Date.now() - acceptedProgressReadyAt >= 12_000
              )
            ) ||
            Date.now() - concreteHandleReadyAt >= 15_000
          )
        ) {
          break;
        }
      }
    }

    mergeProgramHandleHints(aggregateHints, capture.state.handleHints);
    const programHandleState = buildProgramHandleState(baseUrl, args, lastSnapshot.url, capture.state);
    const appPath =
      programHandleState.appPath ??
      [...aggregateHints.appPaths]
        .reverse()
        .find((value) => /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value))) ??
      null;
    const canvasProgramUrl =
      programHandleState.canvasProgramUrl ??
      (appPath ? `${baseUrl.replace(/\/+$/, "")}${appPath}` : null);
    const conversationId =
      programHandleState.conversationId ??
      deriveConversationIdForAppPath(appPath, aggregateHints.conversationIds);
    const responseId =
      programHandleState.responseId ??
      [...aggregateHints.responseIds].reverse()[0] ??
      null;
    const lastSnapshotActionContract =
      extractCanvasProgramActionContractFromText(lastSnapshot.bodyText);
    const actionContract = {
      canvasProgramAction:
        capture.state.actionContract.canvasProgramAction ??
        lastSnapshotActionContract.canvasProgramAction,
      canvasProgramActionInput:
        capture.state.actionContract.canvasProgramActionInput ??
        lastSnapshotActionContract.canvasProgramActionInput,
    };
    let invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        bootstrapOperation,
        actionContract,
        capture.state.transportHints,
        lastSnapshot,
        bootstrapPrompt,
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
      const playClicked = await clickMediaActionButton(
        activePage,
        bootstrapOperation,
        "play",
        timeoutMs,
      );
      if (playClicked) {
        await activePage.waitForTimeout(2000);
        lastSnapshot = await collectProgramHandleSnapshot(activePage);
        mergeProgramHandleHints(aggregateHints, lastSnapshot.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            bootstrapOperation,
            capture.state.actionContract,
            capture.state.transportHints,
            lastSnapshot,
            bootstrapPrompt,
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
      const downloadClicked = await clickMediaActionButton(
        activePage,
        bootstrapOperation,
        "download",
        timeoutMs,
      );
      if (downloadClicked) {
        await activePage.waitForTimeout(2500);
        lastSnapshot = await collectProgramHandleSnapshot(activePage);
        mergeProgramHandleHints(aggregateHints, lastSnapshot.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            bootstrapOperation,
            capture.state.actionContract,
            capture.state.transportHints,
            lastSnapshot,
            bootstrapPrompt,
            capture.state,
          ),
        );
        invokeContract = capture.state.invokeContract;
      }
    }

    if (!canvasProgramUrl || !appPath || !conversationId) {
      throw Object.assign(
        new Error("Gemini Canvas program bootstrap completed without a concrete program handle."),
        {
          status: 504,
          code: "gemini_canvas_program_bootstrap_handle_missing",
          bodyText: lastSnapshot.bodyText ?? null,
        },
      );
    }

    return {
      operation: "bootstrap_program",
      runtimeStateObjectKey: normalizeString(args.runtimeStateObjectKey),
      shareUrl,
      shareId,
      shareFollowKind: shareFollow.kind,
      bootstrapOperation,
      bootstrapPrompt,
      discoveryOnly,
      beforeUrl: before.url,
      finalUrl: lastSnapshot.url,
      pageUrl: lastSnapshot.url,
      bodyText: lastSnapshot.bodyText ?? null,
      canvasProxyPreview,
      shareMaterializationRetry,
      newChatClicked,
      modeSelected,
      canvasProgramUrl,
      appPath,
      conversationId,
      responseId,
      lastSeenConversationId:
        programHandleState.lastSeenConversationId ??
        [...aggregateHints.conversationIds].reverse()[0] ??
        null,
      lastSeenResponseId:
        programHandleState.lastSeenResponseId ??
        [...aggregateHints.responseIds].reverse()[0] ??
        null,
      candidatePairs: programHandleState.candidatePairs,
      stableProgramPair: programHandleState.stableProgramPair,
      latestResponsePair: programHandleState.latestResponsePair,
      aggregateHints,
      transportHints: capture.state.transportHints,
      invokeBaseUrl: programHandleState.invokeBaseUrl,
      musicWsUrl: programHandleState.musicWsUrl,
      videoInvokePath: programHandleState.videoInvokePath,
      canvasProgramAction: actionContract.canvasProgramAction,
      canvasProgramActionInput: actionContract.canvasProgramActionInput,
      canvasProgramInvokeContract: invokeContract,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
      capturedAt: programHandleState.capturedAt,
      lastValidatedAt: programHandleState.lastValidatedAt,
    };
  } finally {
    capture.stop();
  }
}

function normalizeAssistantText(text) {
  const lines = String(text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .filter(
      (line) =>
        !/^(显示思路|Show thinking|Gemini 说|Gemini says?|复制提示|Copy prompt|修改|Modify|重做|Redo|听回答|Listen|答得好|答得不好|Assessing Prompt Clarity|Resolving Instruction Conflict|立即回答)$/i.test(
          line,
        ),
    );
  return lines.join("\n").trim();
}

function bodyIndicatesGenerationInProgress(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  return /(Gemini 正在输入|Gemini is typing|正在输入|is typing|正在思考|Thinking|正在生成|Generating)/i.test(
    text,
  );
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

function bodyIndicatesMusicPendingOrBusy(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  const normalized = text.toLowerCase();
  return (
    bodyIndicatesMusicAcceptedProgress(text)
    || normalized.includes("i've hit a bit of a snag")
    || normalized.includes("i’ve hit a bit of a snag")
    || normalized.includes("please try again later")
    || normalized.includes("getting a lot of requests right now")
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
  if (invokeContract.target && !invokeContractIsProxyOnlyCandidate(invokeContract)) {
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

function extractExactAnswerDirective(prompt) {
  const text = String(prompt || "").trim();
  if (!text) {
    return null;
  }
  const lowered = text.toLowerCase();
  const markers = [
    "reply with exactly:",
    "return exactly:",
    "output exactly:",
    "respond with exactly:",
  ];
  for (const marker of markers) {
    const index = lowered.indexOf(marker);
    if (index === -1) {
      continue;
    }
    const remainder = text.slice(index + marker.length).trim();
    if (!remainder) {
      continue;
    }
    const firstLine = remainder
      .split(/\r?\n/)
      .map((line) => line.trim().replace(/^["`]+|["`]+$/g, "").trim())
      .find(Boolean);
    if (firstLine) {
      return firstLine;
    }
  }
  return null;
}

function augmentToolHistoryPrompt(prompt) {
  const text = String(prompt || "");
  if (!text) {
    return text;
  }
  const hasToolHistory =
    /Caller-provided result for\s+`[^`]+`:/i.test(text) ||
    /The caller has already executed every required external tool\./i.test(text);
  if (!hasToolHistory || /Required exact final answer:/i.test(text)) {
    return text;
  }
  const exactAnswer = extractExactAnswerDirective(text);
  if (!exactAnswer) {
    return text;
  }
  return `${text}\n\nRequired exact final answer:\n${exactAnswer}\n\nReturn exactly that text and nothing else.`;
}

async function collectTextSnapshot(page) {
  return page.evaluate(() => {
    const collectTexts = (selector) =>
      Array.from(document.querySelectorAll(selector))
        .map((node) => (node.innerText || "").trim())
        .filter(Boolean);

    const primaryTexts = collectTexts("message-content");
    const fallbackTexts = collectTexts(".markdown.markdown-main-panel, model-response");
    const sendButton = Array.from(document.querySelectorAll("button")).find((node) =>
      /(发送|Send)/i.test((node.innerText || node.getAttribute("aria-label") || "").trim()),
    );

    return {
      url: location.href,
      bodyText: (document.body?.innerText ?? "").slice(0, 12000),
      primaryTexts,
      fallbackTexts,
      sendDisabled: Boolean(sendButton?.disabled),
    };
  });
}

async function runTextOperation(entry, args) {
  const prompt = normalizeString(args.prompt);
  if (!prompt) {
    throw Object.assign(
      new Error("Gemini Canvas text invocation requires a non-empty prompt."),
      {
        status: 400,
        code: "gemini_canvas_invalid_text_prompt",
      },
    );
  }

  const timeoutMs = Math.max(
    Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
    operationConfig("text").resultTimeoutMs,
  );
  const effectivePrompt = augmentToolHistoryPrompt(prompt);
  const page = entry.page;
  const preferredProgramPageUrl = resolveProgramPageUrl(
    normalizeString(args.baseUrl) ?? "https://gemini.google.com",
    args,
  );
  const capture = startNetworkCapture(page, "text");
  log("text operation starting", effectivePrompt.slice(0, 160));
  try {
    await resetConversation(
      page,
      normalizeString(args.baseUrl) ?? "https://gemini.google.com",
      timeoutMs,
      preferredProgramPageUrl,
    );
    log("text conversation reset complete", page.url());
    const baseline = await collectTextSnapshot(page);
    const baselineTexts =
      baseline.primaryTexts.length > 0 ? baseline.primaryTexts : baseline.fallbackTexts;
    const baselineLastText = normalizeAssistantText(baselineTexts.at(-1) || "");
    const selectLatestCandidate = (snapshot) => {
      const candidateTexts =
        snapshot.primaryTexts.length > 0 ? snapshot.primaryTexts : snapshot.fallbackTexts;
      const rawCandidate = candidateTexts.at(-1) || "";
      const candidate = normalizeAssistantText(rawCandidate);
      const isNewResponse =
        Boolean(candidate) &&
        (candidateTexts.length > baselineTexts.length || candidate !== baselineLastText);
      return { candidate, candidateTexts, isNewResponse };
    };

    log("text submitting prompt");
    await submitPrompt(page, effectivePrompt, timeoutMs);
    log("text prompt submitted");

    const deadline = Date.now() + timeoutMs;
    let lastStableText = null;
    let stableHits = 0;
    let lastSnapshot = baseline;
    let pollCount = 0;
    while (Date.now() < deadline) {
      pollCount += 1;
      lastSnapshot = await collectTextSnapshot(page);
      const { candidate, isNewResponse } = selectLatestCandidate(lastSnapshot);
      const responseInProgress = bodyIndicatesGenerationInProgress(lastSnapshot.bodyText);

      if (pollCount <= 3 || pollCount % 10 === 0) {
        log(
          "text poll snapshot",
          JSON.stringify({
            pollCount,
            primaryCount: lastSnapshot.primaryTexts.length,
            fallbackCount: lastSnapshot.fallbackTexts.length,
            sendDisabled: lastSnapshot.sendDisabled,
            responseInProgress,
            bodyPreview: String(lastSnapshot.bodyText || "").slice(0, 240),
          }),
        );
      }

      if (isNewResponse) {
        if (candidate === lastStableText) {
          stableHits += 1;
        } else {
          lastStableText = candidate;
          stableHits = 1;
        }
        const looksLikeToolPayload = /<tool_calls>|<function_calls>|<invoke\b/i.test(candidate);
        const looksSubstantive = candidate.length >= 24 || looksLikeToolPayload;
        if (pollCount <= 3 || pollCount % 10 === 0) {
          log(
            "text candidate observed",
            JSON.stringify({
              pollCount,
              stableHits,
              sendDisabled: lastSnapshot.sendDisabled,
              responseInProgress,
              looksSubstantive,
              candidatePreview: candidate.slice(0, 120),
            }),
          );
        }
        if (
          stableHits >= 2 &&
          (lastSnapshot.sendDisabled || looksSubstantive || !responseInProgress)
        ) {
          return {
            operation: "text",
            pageUrl: lastSnapshot.url,
            bodyText: lastSnapshot.bodyText,
            ...buildProgramHandleState(
              normalizeString(args.baseUrl) ?? "https://gemini.google.com",
              args,
              lastSnapshot.url,
              capture.state,
            ),
            text: candidate,
            media: [],
            networkEvents: capture.state.events,
            rpcCaptures: capture.state.rpcCaptures,
          };
        }
      }

      await page.waitForTimeout(1500);
    }

    const fallback = selectLatestCandidate(lastSnapshot);
    if (fallback.isNewResponse && fallback.candidate) {
      log("text operation returning fallback candidate", fallback.candidate.slice(0, 160));
      return {
        operation: "text",
        pageUrl: lastSnapshot.url,
        bodyText: lastSnapshot.bodyText,
        ...buildProgramHandleState(
          normalizeString(args.baseUrl) ?? "https://gemini.google.com",
          args,
          lastSnapshot.url,
          capture.state,
        ),
        text: fallback.candidate,
        media: [],
        networkEvents: capture.state.events,
        rpcCaptures: capture.state.rpcCaptures,
      };
    }

    log(
      "text operation timed out",
      JSON.stringify({
        pageUrl: lastSnapshot?.url ?? null,
        bodyPreview: String(lastSnapshot?.bodyText ?? "").slice(0, 240),
      }),
    );
    throw Object.assign(new Error("Timed out waiting for Gemini Canvas text response."), {
      status: 504,
      code: "gemini_canvas_text_timeout",
      bodyText: lastSnapshot?.bodyText ?? null,
      captureState: capture.state,
    });
  } finally {
    capture.stop();
  }
}

function selectAudioAsset(snapshot, captureState) {
  const audioNode = snapshot.mediaNodes.find((node) => {
    const url = node.currentSrc || node.src;
    return (
      node.kind === "audio" &&
      typeof url === "string" &&
      (/blob:/i.test(url) || isAudioLikeUrl(url))
    );
  });

  const networkCandidate = [...captureState.audioUrls]
    .reverse()
    .find(
      (candidate) =>
        candidate.url &&
        (isAudioLikeMimeType(candidate.mimeType) || isAudioLikeUrl(candidate.url)),
    );
  if (networkCandidate) {
    return {
      url: networkCandidate.url,
      mimeType: networkCandidate.mimeType || "audio/wav",
      bodyBase64: networkCandidate.bodyBase64 || null,
      durationSeconds: audioNode?.duration ?? null,
    };
  }

  const anchorCandidate = [...(snapshot.anchorNodes || [])].reverse().find((entry) => {
    const href = String(entry?.href || "");
    const label = `${entry?.text || ""}\n${entry?.ariaLabel || ""}\n${entry?.title || ""}`;
    return (
      href &&
      (isAudioLikeUrl(href) ||
        isBlobLikeUrl(href) ||
        /下载音乐作品|Download music|播放|Play/i.test(label))
    );
  });
  if (anchorCandidate?.href) {
    return {
      url: anchorCandidate.href,
      mimeType: inferMimeTypeFromUrl(anchorCandidate.href, "audio/wav"),
      durationSeconds: audioNode?.duration ?? null,
    };
  }

  const streamGenerateCandidate = [...(captureState?.rpcCaptures || [])]
    .reverse()
    .find(
      (capture) =>
        capture?.type === "response"
        && (
          capture?.label === "StreamGenerate"
          || /\/StreamGenerate/i.test(String(capture?.url || ""))
        )
        && normalizeString(capture?.bodyText),
    );
  if (streamGenerateCandidate?.bodyText) {
    const rawMatches =
      String(streamGenerateCandidate.bodyText).match(
        /https:\/\/contribution\.usercontent\.google\.com\/download\?[^"]+/g,
      ) || [];
    for (const rawMatch of rawMatches) {
      const decodedUrl = String(rawMatch)
        .replace(/\\u003d/g, "=")
        .replace(/\\u0026/g, "&");
      if (!isAudioLikeUrl(decodedUrl)) {
        continue;
      }
      return {
        url: decodedUrl,
        mimeType: inferMimeTypeFromUrl(decodedUrl, "audio/mpeg"),
        durationSeconds: audioNode?.duration ?? null,
      };
    }
  }

  if (!audioNode) {
    return null;
  }
  const url = audioNode.currentSrc || audioNode.src;
  return {
    url,
    mimeType: inferMimeTypeFromUrl(url, "audio/wav"),
    durationSeconds:
      typeof audioNode.duration === "number" && Number.isFinite(audioNode.duration)
        ? audioNode.duration
        : null,
  };
}

function selectAudioAssetFromInvokeContract(invokeContract) {
  const candidates = [];
  const directTarget = normalizeString(invokeContract?.target);
  if (directTarget && (isAudioLikeUrl(directTarget) || isAudioLikeMimeType(invokeContract?.targetMimeType))) {
    candidates.push({
      url: directTarget,
      mimeType: normalizeString(invokeContract?.targetMimeType) || inferMimeTypeFromUrl(directTarget, "audio/mpeg"),
      source: normalizeString(invokeContract?.targetSource) || "invoke_contract_target",
      kind: "audio",
      score: scorePlayerReadyTargetCandidate("music", {
        url: directTarget,
        mimeType: normalizeString(invokeContract?.targetMimeType) || null,
        source: normalizeString(invokeContract?.targetSource) || "invoke_contract_target",
        kind: "audio",
        download: /download/i.test(String(invokeContract?.targetSource || "")),
      }),
    });
  }
  for (const candidate of invokeContract?.targetCandidates || []) {
    const url = normalizeString(candidate?.url);
    const mimeType = normalizeString(candidate?.mimeType) || inferMimeTypeFromUrl(url, null);
    if (!url || (!isAudioLikeUrl(url) && !isAudioLikeMimeType(mimeType))) {
      continue;
    }
    candidates.push({
      url,
      mimeType: mimeType || "audio/mpeg",
      source: normalizeString(candidate?.source) || "invoke_contract_target_candidate",
      kind: "audio",
      score:
        Number.isFinite(candidate?.score)
          ? Number(candidate.score)
          : scorePlayerReadyTargetCandidate("music", {
            url,
            mimeType,
            source: normalizeString(candidate?.source) || "invoke_contract_target_candidate",
            kind: "audio",
            download: Boolean(candidate?.download),
          }),
    });
  }
  const best = candidates
    .sort((left, right) => (Number(right?.score || 0) - Number(left?.score || 0)))[0];
  if (!best?.url) {
    return null;
  }
  return {
    url: best.url,
    mimeType: best.mimeType || "audio/mpeg",
    durationSeconds: null,
  };
}

async function extractAudioBytes(page, asset) {
  if (asset?.bodyBase64) {
    return {
      mimeType: asset.mimeType || "audio/wav",
      bodyBase64: asset.bodyBase64,
    };
  }

  const normalizedUrl = normalizeGeminiBrowserAssetUrl(asset?.url) ?? asset?.url;
  let result;
  try {
    result = await page.evaluate(async ({ url }) => {
      const response = await fetch(url, { credentials: "include" });
      const arrayBuffer = await response.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      let binary = "";
      const chunkSize = 0x8000;
      for (let index = 0; index < bytes.length; index += chunkSize) {
        const chunk = bytes.subarray(index, index + chunkSize);
        binary += String.fromCharCode(...chunk);
      }
      return {
        status: response.status,
        ok: response.ok,
        contentType: response.headers.get("content-type"),
        bodyBase64: btoa(binary),
      };
    }, { url: normalizedUrl });
  } catch (error) {
    throw Object.assign(
      new Error("Failed to download Gemini Canvas TTS audio payload from the browser context."),
      {
        status: Number(error?.status ?? 500),
        code: "gemini_canvas_tts_audio_fetch_failed",
        url: normalizedUrl ?? null,
        cause: error instanceof Error ? error.message : String(error),
      },
    );
  }

  if (!result?.ok || !result.bodyBase64) {
    throw Object.assign(new Error("Failed to download Gemini Canvas TTS audio payload from the browser context."), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_tts_audio_fetch_failed",
      url: normalizedUrl ?? null,
    });
  }

  const resolvedMimeType = result.contentType || asset.mimeType || "audio/wav";
  if (!isAudioLikeMimeType(resolvedMimeType) && !isAudioLikeUrl(normalizedUrl)) {
    throw Object.assign(new Error("Gemini Canvas TTS candidate asset was not an audio resource."), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_tts_non_audio_asset",
      mimeType: resolvedMimeType,
      url: normalizedUrl,
    });
  }

  return {
    mimeType: resolvedMimeType,
    bodyBase64: result.bodyBase64,
  };
}

async function extractImageBytes(page, asset) {
  if (asset?.bodyBase64) {
    return {
      mimeType: asset.mimeType || "image/png",
      bodyBase64: asset.bodyBase64,
    };
  }

  const normalizedUrl = normalizeGeminiBrowserAssetUrl(asset?.url) ?? asset?.url;
  let result;
  try {
    result = await page.evaluate(async ({ url }) => {
      const response = await fetch(url, { credentials: "include" });
      const arrayBuffer = await response.arrayBuffer();
      const bytes = new Uint8Array(arrayBuffer);
      let binary = "";
      const chunkSize = 0x8000;
      for (let index = 0; index < bytes.length; index += chunkSize) {
        const chunk = bytes.subarray(index, index + chunkSize);
        binary += String.fromCharCode(...chunk);
      }
      return {
        status: response.status,
        ok: response.ok,
        contentType: response.headers.get("content-type"),
        bodyBase64: btoa(binary),
      };
    }, { url: normalizedUrl });
  } catch (error) {
    throw Object.assign(
      new Error("Failed to download Gemini Canvas image payload from the browser context."),
      {
        status: Number(error?.status ?? 500),
        code: "gemini_canvas_image_fetch_failed",
        url: normalizedUrl ?? null,
        cause: error instanceof Error ? error.message : String(error),
      },
    );
  }

  if (!result?.ok || !result.bodyBase64) {
    throw Object.assign(new Error("Failed to download Gemini Canvas image payload from the browser context."), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_image_fetch_failed",
      url: normalizedUrl ?? null,
    });
  }

  const resolvedMimeType = result.contentType || asset.mimeType || "image/png";
  if (!/^image\//i.test(resolvedMimeType)) {
    throw Object.assign(new Error(`Gemini Canvas image candidate asset was not an image resource. url=${normalizedUrl} contentType=${resolvedMimeType}`), {
      status: Number(result?.status ?? 500),
      code: "gemini_canvas_non_image_asset",
      mimeType: resolvedMimeType,
      url: normalizedUrl,
    });
  }

  return {
    mimeType: resolvedMimeType,
    bodyBase64: result.bodyBase64,
  };
}

async function downloadBinaryViaNavigation(entry, url, timeoutMs) {
  const page = await entry.context.newPage();
  try {
    const response = await page.goto(url, {
      waitUntil: "commit",
      timeout: timeoutMs,
    });
    if (!response) {
      throw Object.assign(
        new Error("Gemini Canvas browser navigation download returned no response."),
        {
          status: 599,
          code: "gemini_canvas_navigation_fetch_missing_response",
        },
      );
    }
    const bodyBuffer = Buffer.from(await response.body());
    const responseHeaders = response.headers();
    const contentType = responseHeaders["content-type"] ?? null;
    const bodyText =
      contentType && /(json|text|javascript|xml|html)/i.test(contentType)
        ? bodyBuffer.toString("utf8")
        : null;
    return {
      status: response.status(),
      ok: response.ok(),
      finalUrl: response.url(),
      contentType,
      headers: responseHeaders,
      bodyText,
      bodyBase64: bodyBuffer.toString("base64"),
    };
  } finally {
    await page.close().catch(() => undefined);
  }
}

async function collectButtonSnapshot(page) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll('button,[role="button"],a[role="button"]'))
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 300),
  );
}

function bodyTextSuggestsVideoTemplateSelection(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  return (
    /挑选一个模板/.test(text) ||
    /开始制作你的视频/.test(text) ||
    /choose a template/i.test(text) ||
    /start creating your video/i.test(text)
  );
}

async function tryClickSendButton(page, timeoutMs) {
  const sendCandidates = [
    page.getByRole("button", { name: /发送|Send/i }).first(),
    page.locator(
      'button[aria-label*="Send"], button[aria-label*="发送"], button[title*="Send"], button[title*="发送"]',
    ).first(),
    page.locator('button:has(svg), button:has(i)').last(),
  ];

  for (const candidate of sendCandidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 1_500) });
      await candidate.click({ timeout: Math.min(timeoutMs, 5_000), force: true });
      return true;
    } catch {
      // try next candidate
    }
  }
  return false;
}

async function hasVideoCreateAction(page, timeoutMs) {
  const candidates = [
    page
      .locator('button,[role="button"],a[role="button"],div[role="button"],span[role="button"]')
      .filter({ hasText: /创作视频|制作视频|Create video/i })
      .last(),
  ];

  for (const candidate of candidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 800) });
      return true;
    } catch {
      // try next candidate
    }
  }

  try {
    return await page.evaluate(() => {
      const matcher = /创作视频|制作视频|Create video/i;
      return Array.from(
        document.querySelectorAll(
          'button,[role="button"],a[role="button"],div[role="button"],span[role="button"]',
        ),
      ).some((node) => {
        const text = `${node.textContent || ""}\n${node.getAttribute?.("aria-label") || ""}\n${node.getAttribute?.("title") || ""}`;
        if (!matcher.test(text)) {
          return false;
        }
        const style = window.getComputedStyle(node);
        const rect = node.getBoundingClientRect();
        return (
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          rect.width > 0 &&
          rect.height > 0
        );
      });
    });
  } catch {
    return false;
  }
}

async function tryClickVideoCreateAction(page, timeoutMs) {
  const candidates = [
    page
      .locator('button,[role="button"],a[role="button"],div[role="button"],span[role="button"]')
      .filter({ hasText: /创作视频|制作视频|Create video/i })
      .last(),
  ];

  for (const candidate of candidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 1_500) });
      const ariaLabel = await candidate.getAttribute("aria-label").catch(() => null);
      const title = await candidate.getAttribute("title").catch(() => null);
      const combined = `${ariaLabel || ""}\n${title || ""}`;
      if (/取消选择|deselect/i.test(combined)) {
        continue;
      }
      await candidate.click({ timeout: Math.min(timeoutMs, 5_000), force: true });
      return true;
    } catch {
      // try next candidate
    }
  }

  try {
    const domClicked = await page.evaluate(() => {
      const matcher = /创作视频|制作视频|Create video/i;
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
      const dispatchClick = (node) => {
        node.scrollIntoView({ block: "center", inline: "center" });
        for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
          node.dispatchEvent(
            new MouseEvent(type, {
              bubbles: true,
              cancelable: true,
              composed: true,
              view: window,
            }),
          );
        }
        if (typeof node.click === "function") {
          node.click();
        }
      };

      const nodes = Array.from(
        document.querySelectorAll(
          'button,[role="button"],a[role="button"],div[role="button"],span[role="button"]',
        ),
      );
      for (const node of nodes) {
        const combined = `${node.textContent || ""}\n${node.getAttribute?.("aria-label") || ""}\n${node.getAttribute?.("title") || ""}`;
        if (!matcher.test(combined) || /取消选择|deselect/i.test(combined) || !isVisible(node)) {
          continue;
        }
        let clickableNode = node;
        let depth = 0;
        while (clickableNode && depth < 8) {
          if (isClickable(clickableNode) && isVisible(clickableNode)) {
            break;
          }
          clickableNode = clickableNode.parentElement;
          depth += 1;
        }
        dispatchClick(clickableNode instanceof HTMLElement ? clickableNode : node);
        return true;
      }
      return false;
    });
    if (domClicked) {
      return true;
    }
  } catch {
    // fall through
  }
  return false;
}

async function trySelectVideoTemplateCard(page, timeoutMs, attemptIndex = 0) {
  const selection = await page.evaluate((requestedIndex) => {
    const templateLabels = [
      "slime",
      "Civilization",
      "Metallic",
      "Memo",
      "Glam",
      "Crochet",
      "Cyberpunk",
      "Video Game",
      "Cosmos",
      "Action Hero",
      "Stardust",
      "Jellytoon",
      "Racetrack",
      "ASMR Apple",
      "Red Carpet",
      "Popcorn",
    ];
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

    const templateImages = Array.from(document.querySelectorAll("img"))
      .filter((node) => /视频生成模板图片|video generation template/i.test(node.alt || ""))
      .filter((node) => isVisible(node))
      .map((node) => ({
        source: "image",
        node,
        label: node.alt || null,
      }));
    const templateTextNodes = Array.from(document.querySelectorAll("button,[role=\"button\"],a,div,span"))
      .map((node) => ({
        node,
        text: (node.innerText || "").trim(),
      }))
      .filter((entry) => entry.text && templateLabels.includes(entry.text))
      .filter((entry) => isVisible(entry.node))
      .map((entry) => ({
        source: "text",
        node: entry.node,
        label: entry.text,
      }));
    const templateCandidates = [...templateImages, ...templateTextNodes];
    if (templateCandidates.length === 0) {
      return {
        clicked: false,
        reason: "template_candidate_missing",
        templateCount: 0,
      };
    }

    const selectedIndex = Math.min(
      Math.max(Number(requestedIndex || 0), 0),
      templateCandidates.length - 1,
    );
    const selectedCandidate = templateCandidates[selectedIndex];
    const imageNode = selectedCandidate.node;
    let clickableNode = findClickableAncestor(imageNode);
    let depth = 0;
    while (clickableNode && depth < 8) {
      if (isClickable(clickableNode) && isVisible(clickableNode)) {
        break;
      }
      clickableNode = clickableNode.parentElement;
      depth += 1;
    }

    if (!(clickableNode instanceof HTMLElement) || !isVisible(clickableNode)) {
      clickableNode = imageNode;
    }

    clickNode(clickableNode);

    return {
      clicked: true,
      reason: "template_clicked",
      templateCount: templateCandidates.length,
      selectedIndex,
      selectedSource: selectedCandidate.source,
      selectedLabel: selectedCandidate.label,
      selectedSrc: imageNode.currentSrc || imageNode.src || null,
      selectedAlt: imageNode.alt || null,
      clickedTag: clickableNode.tagName || null,
      clickedRole: clickableNode.getAttribute?.("role") || null,
      clickedAria: clickableNode.getAttribute?.("aria-label") || null,
      clickedText: (clickableNode.innerText || "").trim().slice(0, 200),
    };
  }, attemptIndex);

  await page.waitForTimeout(1_500);
  const videoCreateVisible = await hasVideoCreateAction(page, timeoutMs).catch(() => false);
  const videoCreateClicked = await tryClickVideoCreateAction(page, timeoutMs).catch(
    () => false,
  );
  const sendClicked = !videoCreateClicked && !videoCreateVisible
    ? await tryClickSendButton(page, timeoutMs).catch(() => false)
    : false;
  return {
    ...selection,
    videoCreateVisible,
    videoCreateClicked,
    sendClicked,
  };
}

function buildTtsDiagnostics({
  captureState,
  lastSnapshot,
  lastButtonSnapshot,
  clicked = false,
  pollCount = null,
  asset = null,
  audio = null,
}) {
  return {
    clicked,
    pollCount,
    pageUrl: lastSnapshot?.pageState?.url ?? null,
    title: lastSnapshot?.pageState?.title ?? null,
    bodyText: lastSnapshot?.pageState?.bodyText ?? null,
    buttons: Array.isArray(lastButtonSnapshot) ? lastButtonSnapshot.slice(0, 120) : [],
    mediaNodes: Array.isArray(lastSnapshot?.mediaNodes) ? lastSnapshot.mediaNodes.slice(0, 80) : [],
    anchorNodes: Array.isArray(lastSnapshot?.anchorNodes) ? lastSnapshot.anchorNodes.slice(0, 40) : [],
    streamGenerateRequestAt: captureState?.streamGenerateRequestAt ?? null,
    streamGenerateResponseAt: captureState?.streamGenerateResponseAt ?? null,
    audioUrls: Array.isArray(captureState?.audioUrls) ? captureState.audioUrls.slice(-12) : [],
    events: Array.isArray(captureState?.events) ? captureState.events.slice(-120) : [],
    selectedAsset: asset
      ? {
        url: asset.url ?? null,
        mimeType: asset.mimeType ?? null,
        durationSeconds: asset.durationSeconds ?? null,
      }
      : null,
    audioResult: audio
      ? {
        mimeType: audio.mimeType ?? null,
        bodyBase64Length: typeof audio.bodyBase64 === "string" ? audio.bodyBase64.length : 0,
      }
      : null,
  };
}

function listenControlCandidates(page) {
  return [
    page.getByRole("button", { name: /听回答|收听回答|朗读|播放语音|播放回答|Listen|Play response|Read aloud|Listen to response/i }).last(),
    page.locator(
      'button[aria-label*="Listen"], button[aria-label*="听"], button[aria-label*="朗读"], button[aria-label*="播放"], button[title*="Listen"], button[title*="听"], button[title*="朗读"], button[title*="播放"]',
    ).last(),
    page.locator(
      'button:has-text("听"), button:has-text("朗读"), button:has-text("播放"), button:has-text("Listen"), button:has-text("Read aloud"), button:has-text("Play")',
    ).last(),
  ];
}

async function clickListenControlWithFallback(page, candidate, timeoutMs) {
  try {
    await candidate.waitFor({ state: "visible", timeout: 1200 });
  } catch {
    return false;
  }

  try {
    await candidate.scrollIntoViewIfNeeded().catch(() => undefined);
    await candidate.click({ timeout: Math.min(timeoutMs, 10_000) });
    return true;
  } catch {
    // Fall back to a DOM-dispatched click when Gemini's presented-response layer
    // intercepts pointer events above the button.
  }

  try {
    const elementHandle = await candidate.elementHandle();
    if (!elementHandle) {
      return false;
    }
    await elementHandle.evaluate((node) => {
      node.scrollIntoView({ block: "center", inline: "nearest" });
      for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
        node.dispatchEvent(
          new MouseEvent(type, {
            bubbles: true,
            cancelable: true,
            composed: true,
            view: window,
          }),
        );
      }
      node.click();
    });
    return true;
  } catch {
    return false;
  }
}

async function runTtsOperation(entry, args) {
  const prompt = normalizeString(args.prompt);
  if (!prompt) {
    throw Object.assign(
      new Error("Gemini Canvas TTS invocation requires a non-empty prompt."),
      {
        status: 400,
        code: "gemini_canvas_invalid_tts_prompt",
      },
    );
  }

  const timeoutMs = Math.max(
    Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
    operationConfig("tts").resultTimeoutMs,
  );
  const fixtureBaseUrl = normalizeString(args.baseUrl);
  if (args.requireAppPage === false && isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
    const ttsBaseUrl = fixtureBaseUrl.replace(/\/+$/, "");
    const fixtureAudio = Buffer.from(
      (`OggSgemini canvas fixture tts::${prompt}`).padEnd(128, "~"),
      "utf8",
    );
    return {
      operation: "tts",
      pageUrl: `${ttsBaseUrl}/gemini-canvas-fixture`,
      bodyText: "gemini canvas fixture tts ok",
      bodyBase64: fixtureAudio.toString("base64"),
      mimeType: "audio/ogg",
      media: [],
    };
  }
  const includeDiagnostics = parseBoolean(args.includeDiagnostics, false);
  const page = entry.page;
  const preferredProgramPageUrl = resolveProgramPageUrl(
    normalizeString(args.baseUrl) ?? "https://gemini.google.com",
    args,
  );
  await resetConversation(
    page,
    normalizeString(args.baseUrl) ?? "https://gemini.google.com",
    timeoutMs,
    preferredProgramPageUrl,
    {
      skipInitialNavigationWhenAppSurfaceReady: entry.attachedCdp === true,
    },
  );
  const capture = startNetworkCapture(page, "tts");

  try {
    await submitPrompt(page, prompt, timeoutMs);
    log("tts prompt submitted", prompt.slice(0, 120));

    const deadline = Date.now() + timeoutMs;
    let lastSnapshot = null;
    let clicked = false;
    let lastButtonSnapshot = [];
    let pollCount = 0;
    while (Date.now() < deadline && !clicked) {
      pollCount += 1;
      lastSnapshot = await collectPageSnapshot(page);
      lastButtonSnapshot = await collectButtonSnapshot(page).catch(() => []);
      if (pollCount <= 3 || pollCount % 10 === 0) {
        log(
          "tts awaiting listen control",
          JSON.stringify({
            pollCount,
            buttonCount: lastButtonSnapshot.length,
            bodyPreview: (lastSnapshot?.pageState?.bodyText ?? "").slice(0, 240),
          }),
        );
      }
      for (const candidate of listenControlCandidates(page)) {
        try {
          if (await clickListenControlWithFallback(page, candidate, timeoutMs)) {
            clicked = true;
            log("tts listen control clicked", `poll=${pollCount}`);
            break;
          }
        } catch {
          // keep trying other selectors / later polling iterations
        }
      }
      if (!clicked) {
        try {
          const domClicked = await page.evaluate(() => {
            const selectors = [
              'button[aria-label*="听回答"]',
              'button[aria-label*="收听回答"]',
              'button[aria-label*="朗读"]',
              'button[aria-label*="播放语音"]',
              'button[aria-label*="播放回答"]',
              'button[aria-label*="Listen"]',
              'button[aria-label*="Play response"]',
              'button[aria-label*="Read aloud"]',
              'button[aria-label*="Listen to response"]',
              'button[title*="听回答"]',
              'button[title*="朗读"]',
              'button[title*="播放"]',
              'button[title*="Listen"]',
              'button[title*="Read aloud"]',
              'button[title*="Play"]',
            ];
            for (const selector of selectors) {
              const matches = Array.from(document.querySelectorAll(selector));
              const target = matches.at(-1);
              if (!target) {
                continue;
              }
              target.scrollIntoView({ block: "center", inline: "nearest" });
              for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
                target.dispatchEvent(
                  new MouseEvent(type, {
                    bubbles: true,
                    cancelable: true,
                    composed: true,
                    view: window,
                  }),
                );
              }
              target.click();
              return true;
            }
            return false;
          });
          if (domClicked) {
            clicked = true;
            break;
          }
        } catch {
          // keep trying later polling iterations
        }
      }
      if (!clicked) {
        await page.waitForTimeout(1500);
      }
    }
    if (!clicked) {
      throw Object.assign(
        new Error(
          "Gemini Canvas did not expose a visible TTS/Listen control for the current response.",
        ),
        {
          status: 500,
          code: "gemini_canvas_tts_control_missing",
          bodyText: JSON.stringify(
            buildTtsDiagnostics({
              captureState: capture.state,
              lastSnapshot,
              lastButtonSnapshot,
              clicked,
              pollCount,
            }),
            null,
            2,
          ),
        },
      );
    }

    while (Date.now() < deadline) {
      lastSnapshot = await collectPageSnapshot(page);
      const asset = selectAudioAsset(lastSnapshot, capture.state);
      if (!asset && (pollCount <= 3 || pollCount % 10 === 0)) {
        log(
          "tts awaiting audio asset",
          JSON.stringify({
            pollCount,
            audioUrls: capture.state.audioUrls,
            streamGenerateRequestAt: capture.state.streamGenerateRequestAt,
            streamGenerateResponseAt: capture.state.streamGenerateResponseAt,
          }),
        );
      }
      if (asset) {
        let audio = null;
        try {
          audio = await extractAudioBytes(page, asset);
        } catch (error) {
          if (error instanceof Error && error.code === "gemini_canvas_tts_non_audio_asset") {
            log(
              "tts skipping non-audio asset",
              JSON.stringify({
                url: asset.url,
                mimeType: error.mimeType ?? null,
              }),
            );
            await page.waitForTimeout(1200);
            continue;
          }
          throw error;
        }
        const result = {
          operation: "tts",
          pageUrl: lastSnapshot.pageState?.url ?? page.url(),
          bodyText: lastSnapshot.pageState?.bodyText ?? null,
          mimeType: audio.mimeType,
          bodyBase64: audio.bodyBase64,
          ...buildProgramHandleState(
            normalizeString(args.baseUrl) ?? "https://gemini.google.com",
            args,
            lastSnapshot.pageState?.url ?? page.url(),
            capture.state,
          ),
          media: [
            {
              kind: "audio",
              url: asset.url,
              mimeType: audio.mimeType,
              durationSeconds: asset.durationSeconds,
            },
          ],
        };
        if (includeDiagnostics) {
          result.diagnostics = buildTtsDiagnostics({
            captureState: capture.state,
            lastSnapshot,
            lastButtonSnapshot,
            clicked,
            pollCount,
            asset,
            audio,
          });
        }
        return result;
      }
      await page.waitForTimeout(2000);
    }

    throw Object.assign(new Error("Timed out waiting for Gemini Canvas TTS audio output."), {
      status: 504,
      code: "gemini_canvas_tts_timeout",
      bodyText: JSON.stringify(
        buildTtsDiagnostics({
          captureState: capture.state,
          lastSnapshot,
          lastButtonSnapshot,
          clicked,
          pollCount,
        }),
        null,
        2,
      ),
    });
  } finally {
    capture.stop();
  }
}

async function runDebugOperation(entry, args = {}) {
  const { page } = entry;
  const baseUrl = normalizeString(args.baseUrl) ?? "https://gemini.google.com";
  const timeoutMs = Math.max(
    Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
    DEFAULT_TIMEOUT_MS,
  );
  const clickOperation = normalizeString(args.clickOperation);
  const captureNetwork = parseBoolean(args.captureNetwork, false);
  const networkWaitMs = Math.max(Number(args.networkWaitMs || 5000), 0);
  const preferredProgramPageUrl = resolveProgramPageUrl(baseUrl, args);

  if (args.resetConversation !== false) {
    await resetConversation(page, baseUrl, timeoutMs, preferredProgramPageUrl);
  }
  let capture = null;
  if (captureNetwork) {
    capture = startNetworkCapture(page, clickOperation || "text");
  }
  if (clickOperation) {
    const selected = await clickOperationMode(page, clickOperation, timeoutMs);
    log("debug click operation result", JSON.stringify({ clickOperation, selected }));
    await page.waitForTimeout(captureNetwork ? Math.max(networkWaitMs, 1200) : 1200);
  }

  const snapshot = await collectPageSnapshot(page);
  const buttons = await collectButtonSnapshot(page).catch(() => []);
  const matchingButtonHtml = clickOperation
    ? await page
        .evaluate((operation) => {
          const candidates = Array.from(document.querySelectorAll("button, [role=\"button\"], a"));
          const matcher =
            operation === "image"
              ? /制作图片|Create image|Create images|Make image/i
              : operation === "music"
                ? /创作音乐|制作音乐|Create music/i
                : operation === "video"
                  ? /创作视频|制作视频|Create video/i
                  : null;
          if (!matcher) {
            return [];
          }
          return candidates
            .filter((node) => matcher.test(node.textContent || "") || matcher.test(node.getAttribute?.("aria-label") || ""))
            .slice(0, 6)
            .map((node, index) => ({
              index,
              outerHTML: node.outerHTML.slice(0, 4000),
              text: (node.textContent || "").trim().slice(0, 300),
              ariaLabel: node.getAttribute?.("aria-label") || null,
            }));
        }, clickOperation)
        .catch(() => [])
    : [];
  const textboxes = await page
    .evaluate(() =>
      Array.from(
        document.querySelectorAll(
          '[role="textbox"], textarea, input[type="text"], [contenteditable="true"]',
        ),
      )
        .map((node, index) => ({
          index,
          tagName: node.tagName,
          ariaLabel: node.getAttribute?.("aria-label") || null,
          placeholder: node.getAttribute?.("placeholder") || null,
          text: (node.innerText || node.textContent || node.value || "").trim().slice(0, 300),
        }))
        .slice(0, 80),
    )
    .catch(() => []);
  const pageDiagnostics = await page.evaluate(() => {
    const html = document.documentElement?.outerHTML || "";
    const scripts = Array.from(document.scripts || []).map((node) => node.textContent || "").join("\n");
    const blob = `${html}\n${scripts}`;
    const apiKeys = Array.from(new Set(blob.match(/AIza[0-9A-Za-z\-_]{20,}/g) || []));
    const debugTerms = [
      "generativelanguage.googleapis.com",
      "clients6.google.com",
      "tts",
      "speech",
      "voiceConfig",
      "responseModalities",
    ];
    const snippets = {};
    for (const term of debugTerms) {
      const loweredBlob = blob.toLowerCase();
      const loweredTerm = term.toLowerCase();
      const index = loweredBlob.indexOf(loweredTerm);
      if (index >= 0) {
        const start = Math.max(0, index - 240);
        const end = Math.min(blob.length, index + loweredTerm.length + 240);
        snippets[term] = blob.slice(start, end);
      }
    }
    const interesting = {};
    for (const key of Object.keys(window)) {
      if (!/config|bootstrap|data|init|api|key/i.test(key)) {
        continue;
      }
      try {
        const value = window[key];
        const serialized = typeof value === "string" ? value : JSON.stringify(value);
        if (serialized && serialized.includes("AIza")) {
          interesting[key] = serialized.slice(0, 4000);
        }
      } catch {
        // ignore serialization failures for opaque browser-owned objects
      }
    }
    return {
      url: location.href,
      title: document.title,
      apiKeys,
      snippets,
      interestingKeys: interesting,
      bodyText: document.body?.innerText ?? "",
    };
    });

  const networkEvents = capture?.state?.events ?? [];
  const programHandleState = buildProgramHandleState(
    baseUrl,
    args,
    pageDiagnostics.url,
    capture?.state,
  );
  capture?.stop?.();

  return {
    operation: "debug",
    pageUrl: pageDiagnostics.url,
    ...programHandleState,
    title: pageDiagnostics.title,
    bodyText: pageDiagnostics.bodyText,
    buttons,
    textboxes,
    matchingButtonHtml,
    media: snapshot.mediaNodes || [],
    apiKeys: pageDiagnostics.apiKeys || [],
    snippets: pageDiagnostics.snippets || {},
    interestingKeys: pageDiagnostics.interestingKeys || {},
    networkEvents,
    rpcCaptures: capture?.state?.rpcCaptures ?? [],
  };
}

async function collectPageSnapshot(page) {
  return page.evaluate(() => {
    const buttons = Array.from(
      document.querySelectorAll('button,[role="button"],a[role="button"]'),
    )
      .map((node, i) => ({
        i,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 200);
    const mediaNodes = [
      ...Array.from(document.querySelectorAll("audio")).map((node, i) => ({
        kind: "audio",
        i,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
      })),
      ...Array.from(document.querySelectorAll("video")).map((node, i) => ({
        kind: "video",
        i,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        width: node.videoWidth,
        height: node.videoHeight,
        duration: Number.isFinite(node.duration) ? node.duration : null,
        poster: node.getAttribute("poster"),
      })),
      ...Array.from(document.querySelectorAll("img")).map((node, i) => ({
        kind: "image",
        i,
        alt: node.getAttribute("alt"),
        src: node.getAttribute("src"),
        width: node.naturalWidth,
        height: node.naturalHeight,
      })),
    ].filter((node) => node.src || node.currentSrc);

    const anchorNodes = Array.from(document.querySelectorAll("a"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        href: node.href,
        rawHref: node.getAttribute("href"),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
        target: node.getAttribute("target"),
        download: node.getAttribute("download"),
      }))
      .filter((node) => node.href || node.text);

    return {
      buttons,
      mediaNodes,
      anchorNodes,
      pageState: {
        url: location.href,
        title: document.title,
        bodyText: (document.body?.innerText ?? "").slice(0, 12000),
      },
    };
  });
}

function isInterestingImageNode(node) {
  const src = `${node.currentSrc ?? ""} ${node.src ?? ""}`;
  return (
    node.kind === "image" &&
    (
      /AI 生成/i.test(node.alt ?? "") ||
      (
        /blob:|googleusercontent|googlevideo|gvt1|gg-dl|rd-gg-dl/i.test(src) &&
        !isLikelyAvatarUrl(src) &&
        !isLikelyNoiseMediaUrl(src) &&
        ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256)
      )
    )
  );
}

function isLikelyGeneratedImageUrl(url) {
  return /blob:|googleusercontent|contribution\.usercontent\.google\.com|gg-dl|rd-gg-dl|googlevideo|gvt1/i.test(
    String(url || ""),
  );
}

function selectImageAssets(snapshot, captureState) {
  const assets = [];
  const imageNode = snapshot.mediaNodes.find((node) => isInterestingImageNode(node)) ?? null;

  const networkCandidate = [...captureState.imageUrls]
    .reverse()
    .find((candidate) => candidate.url && isLikelyGeneratedImageUrl(candidate.url));
  if (networkCandidate) {
    assets.push({
      kind: "image",
      url: normalizeGeminiBrowserAssetUrl(networkCandidate.url) ?? networkCandidate.url,
      mimeType: inferMimeTypeFromUrl(
        normalizeGeminiBrowserAssetUrl(networkCandidate.url) ?? networkCandidate.url,
        networkCandidate.mimeType || "image/png",
      ),
      bodyBase64: networkCandidate.bodyBase64 || null,
      alt: imageNode?.alt ?? null,
      width: imageNode?.width ?? null,
      height: imageNode?.height ?? null,
      durationSeconds: null,
    });
    return assets;
  }

  if (imageNode && imageNode.src) {
    const normalizedImageUrl = normalizeGeminiBrowserAssetUrl(imageNode.src) ?? imageNode.src;
    assets.push({
      kind: "image",
      url: normalizedImageUrl,
      mimeType: inferMimeTypeFromUrl(normalizedImageUrl, "image/png"),
      bodyBase64: null,
      alt: imageNode.alt ?? null,
      width: imageNode.width ?? null,
      height: imageNode.height ?? null,
      durationSeconds: null,
    });
  }
  return assets;
}

function selectVideoAsset(snapshot) {
  const anchorCandidate = [...(snapshot.anchorNodes || [])].reverse().find((entry) => {
    const href = String(entry?.href || "");
    const label = `${entry?.text || ""}\n${entry?.ariaLabel || ""}\n${entry?.title || ""}`;
    return (
      href &&
      (isVideoLikeUrl(href) ||
        isBlobLikeUrl(href) ||
        /下载视频|Download video|播放视频|Play video/i.test(label))
    );
  });
  if (anchorCandidate?.href) {
    return [
      {
        kind: "video",
        url: anchorCandidate.href,
        mimeType: inferMimeTypeFromUrl(anchorCandidate.href, "video/mp4"),
        alt: null,
        width: null,
        height: null,
        durationSeconds: null,
      },
    ];
  }
  const videoNode = snapshot.mediaNodes.find((node) => {
    const url = node.currentSrc || node.src;
    return (
      node.kind === "video" &&
      typeof url === "string" &&
      /contribution\.usercontent\.google\.com|googlevideo\.com|gvt1\.com|\.mp4(\?|$)|\.webm(\?|$)/i.test(
        url,
      )
    );
  });
  if (!videoNode) {
    return [];
  }
  const url = videoNode.currentSrc || videoNode.src;
  return [
    {
      kind: "video",
      url,
      mimeType: inferMimeTypeFromUrl(url, "video/mp4"),
      alt: null,
      width: Number.isFinite(videoNode.width) ? videoNode.width : null,
      height: Number.isFinite(videoNode.height) ? videoNode.height : null,
      durationSeconds:
        typeof videoNode.duration === "number" && Number.isFinite(videoNode.duration)
          ? videoNode.duration
          : null,
    },
  ];
}

function selectMediaAssetsForOperation(operation, snapshot, captureState) {
  if (operation === "image") {
    return selectImageAssets(snapshot, captureState);
  }

  if (operation === "music") {
    const audioAsset = selectAudioAsset(snapshot, captureState);
    if (audioAsset) {
      return [
        {
          kind: "audio",
          url: audioAsset.url,
          mimeType: audioAsset.mimeType,
          alt: null,
          width: null,
          height: null,
          durationSeconds: audioAsset.durationSeconds ?? null,
        },
      ];
    }
    const contractAudioAsset = selectAudioAssetFromInvokeContract(captureState?.invokeContract);
    if (contractAudioAsset) {
      return [
        {
          kind: "audio",
          url: contractAudioAsset.url,
          mimeType: contractAudioAsset.mimeType,
          alt: null,
          width: null,
          height: null,
          durationSeconds: contractAudioAsset.durationSeconds ?? null,
        },
      ];
    }
    const networkVideo = [...captureState.videoUrls].reverse().find((candidate) => candidate.url);
    if (networkVideo) {
      return [
        {
          kind: "video",
          url: networkVideo.url,
          mimeType: networkVideo.mimeType || "video/mp4",
          alt: null,
          width: null,
          height: null,
          durationSeconds: null,
        },
      ];
    }
  }

  const domVideo = selectVideoAsset(snapshot);
  if (domVideo.length > 0) {
    return domVideo;
  }

  const networkVideo = [...captureState.videoUrls].reverse().find((candidate) => candidate.url);
  if (networkVideo) {
    return [
      {
        kind: "video",
        url: networkVideo.url,
        mimeType: networkVideo.mimeType || "video/mp4",
        alt: null,
        width: null,
        height: null,
        durationSeconds: null,
      },
    ];
  }

  return [];
}

function detectMediaProviderGate(operation, bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return null;
  }
  if (
    operation === "image" &&
    (
      text.includes("Image Creation Not Available") ||
      /can't create it right now/i.test(text) ||
      /can't seem to create any/i.test(text) ||
      /can't create any for you/i.test(text) ||
      (/search for images/i.test(text) && /can't create/i.test(text)) ||
      /image creation isn't available/i.test(text) ||
      text.includes("您登录了吗") ||
      text.includes("似乎无法为您创建任何图片") ||
      text.includes("所在的地区尚未开通图片创建功能")
    )
  ) {
    return {
      status: 503,
      code: "gemini_canvas_image_generation_unavailable",
      message: "Gemini Canvas image generation is unavailable for the current browser session or location.",
    };
  }
  if (
    operation === "video" &&
    (
      /出了点问题\s*\(13\)/.test(text) ||
      /出了点问题\s*\(1099\)/.test(text) ||
      /something went wrong\s*\(13\)/i.test(text)
    )
  ) {
    return {
      status: 409,
      code: "gemini_canvas_video_mode_unavailable",
      message: "Gemini Canvas video mode could not be activated.",
    };
  }
  if (
    operation === "video" &&
    (
      text.includes("已达到视频生成数量上限") ||
      text.includes("视频生成数量上限") ||
      /出了点问题\s*\(1053\)/.test(text) ||
      text.includes("1053")
    )
  ) {
    return {
      status: 429,
      code: "gemini_canvas_video_quota_reached",
      message: "Gemini Canvas video generation quota is currently exhausted for this account.",
    };
  }
  return null;
}

function recentMediaProviderGateText(captureState) {
  if (!captureState || !Array.isArray(captureState.events)) {
    return "";
  }
  const texts = [];
  for (const event of captureState.events.slice(-8)) {
    if (event?.type !== "response" || typeof event.text !== "string" || !event.text.trim()) {
      continue;
    }
    texts.push(event.text);
  }
  return texts.join("\n");
}

async function runMediaOperation(entry, args) {
  const operation = normalizeString(args.operation);
  const prompt = normalizeString(args.prompt);
  if (!operation || !prompt) {
    throw Object.assign(
      new Error("Gemini Canvas browser invocation requires operation and prompt."),
      {
        status: 400,
        code: "gemini_canvas_invalid_request",
      },
    );
  }

  const { resultTimeoutMs } = operationConfig(operation);
  const timeoutMs = Math.max(
    Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
    resultTimeoutMs,
  );
  const fixtureBaseUrl = normalizeString(args.baseUrl);
  if (args.requireAppPage === false && isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
    const mediaBaseUrl = fixtureBaseUrl.replace(/\/+$/, "");
    if (operation === "image") {
      return {
        operation,
        pageUrl: `${mediaBaseUrl}/gemini-canvas-fixture`,
        bodyText: "gemini canvas fixture image ok",
        media: [
          {
            kind: "image",
            url: `${mediaBaseUrl}/fixtures/gemini-canvas/image.png`,
            mimeType: "image/png",
            alt: "Gemini Canvas fixture image",
            width: 1024,
            height: 1024,
            durationSeconds: null,
          },
        ],
      };
    }
    if (operation === "music") {
      return {
        operation,
        pageUrl: `${mediaBaseUrl}/gemini-canvas-fixture`,
        bodyText: "gemini canvas fixture music ok",
        media: [
          {
            kind: "audio",
            url: `${mediaBaseUrl}/fixtures/gemini-canvas/music.wav`,
            mimeType: "audio/wav",
            alt: "Gemini Canvas fixture music",
            width: null,
            height: null,
            durationSeconds: 3.2,
          },
        ],
      };
    }
    if (operation === "video") {
      return {
        operation,
        pageUrl: `${mediaBaseUrl}/gemini-canvas-fixture`,
        bodyText: "gemini canvas fixture video ok",
        media: [
          {
            kind: "video",
            url: `${mediaBaseUrl}/fixtures/gemini-canvas/video.mp4`,
            mimeType: "video/mp4",
            alt: "Gemini Canvas fixture video",
            width: 1280,
            height: 720,
            durationSeconds: 5.4,
          },
        ],
      };
    }
  }
  const pageLease = await acquireMediaOperationPage(
    entry,
    normalizeString(args.baseUrl) ?? "https://gemini.google.com",
  );
  const page = pageLease.page;
  const preferredProgramPageUrl = resolveProgramPageUrl(
    normalizeString(args.baseUrl) ?? "https://gemini.google.com",
    args,
  );
  await resetConversation(
    page,
    normalizeString(args.baseUrl) ?? "https://gemini.google.com",
    timeoutMs,
    preferredProgramPageUrl,
  );
  const capture = startNetworkCapture(page, operation);

  try {
    const modeSelected = await clickOperationMode(page, operation, timeoutMs);
    if (!modeSelected) {
      const modeUnavailableSnapshot = await collectPageSnapshot(page).catch(() => null);
      const modeUnavailableButtons = await collectButtonSnapshot(page).catch(() => []);
      log(
        "media operation mode unavailable",
        JSON.stringify({
          operation,
          pageUrl: page.url(),
          bodyPreview: String(modeUnavailableSnapshot?.pageState?.bodyText ?? "").slice(0, 1200),
          buttons: modeUnavailableButtons.slice(0, 120),
        }),
      );
      throw Object.assign(
        new Error(`Gemini Canvas ${operation} mode could not be activated.`),
        {
          status: 409,
          code: `gemini_canvas_${operation}_mode_unavailable`,
          bodyText: JSON.stringify(
            {
              operation,
              pageUrl: page.url(),
              bodyText: modeUnavailableSnapshot?.pageState?.bodyText ?? null,
              buttons: modeUnavailableButtons.slice(0, 120),
              mediaNodes: modeUnavailableSnapshot?.mediaNodes?.slice(0, 40) ?? [],
              anchorNodes: modeUnavailableSnapshot?.anchorNodes?.slice(0, 40) ?? [],
              networkEvents: capture.state.events.slice(-80),
              rpcCaptures: capture.state.rpcCaptures.slice(-40),
            },
            null,
            2,
          ),
        },
      );
    }
    log("media operation mode selected", operation);
    await submitPrompt(page, prompt, timeoutMs);
    log("media prompt submitted", `${operation}: ${String(prompt).slice(0, 180)}`);

  const deadline = Date.now() + timeoutMs;
  let lastSnapshot = null;
  let pollCount = 0;
  let videoTemplateSelectionAttempts = 0;
  let videoTemplateSelectedAt = null;
  let videoCreateClickAttempts = 0;
  let videoCreateClickedAt = null;
  let playerReadyPlayAttempted = false;
  let playerReadyDownloadAttempted = false;
  while (Date.now() < deadline) {
    pollCount += 1;
    lastSnapshot = await collectPageSnapshot(page);
    mergeActionContract(
      capture.state.actionContract,
      extractCanvasProgramActionContractFromText(lastSnapshot?.pageState?.bodyText ?? ""),
    );
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        capture.state.actionContract,
        capture.state.transportHints,
        {
          bodyText: lastSnapshot?.pageState?.bodyText ?? "",
          buttons: lastSnapshot?.buttons ?? [],
          anchorNodes: lastSnapshot?.anchorNodes ?? [],
          mediaNodes: lastSnapshot?.mediaNodes ?? [],
        },
        prompt,
        capture.state,
      ),
    );
    const media = selectMediaAssetsForOperation(operation, lastSnapshot, capture.state);

      let quotaGateText = null;
      if (operation === "video") {
        const quotaCandidates = [
          page.getByText(/已达到视频生成数量上限/i).first(),
          page.getByText(/方可继续生成视频/i).first(),
          page.getByText(/出了点问题\s*\(1053\)/i).first(),
        ];
        for (const candidate of quotaCandidates) {
          try {
            if (await candidate.isVisible({ timeout: 50 })) {
              quotaGateText = (await candidate.textContent()) || "Gemini Canvas video quota gate";
              break;
            }
          } catch {
            // try next candidate
          }
        }
      }

      if (pollCount <= 3 || pollCount % 10 === 0) {
        log(
          "media poll snapshot",
          JSON.stringify({
            operation,
            pollCount,
            imageUrls: capture.state.imageUrls.map((entry) => entry.url).slice(-3),
            audioUrls: capture.state.audioUrls.map((entry) => entry.url).slice(-3),
            videoUrls: capture.state.videoUrls.map((entry) => entry.url).slice(-3),
            mediaNodeKinds: (lastSnapshot?.mediaNodes || []).map((node) => node.kind).slice(0, 12),
            bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
            events: operation === "image" ? capture.state.events.slice(-4) : undefined,
          }),
        );
      }

      if (
        operation === "video" &&
        media.length === 0 &&
        bodyTextSuggestsVideoTemplateSelection(lastSnapshot?.pageState?.bodyText) &&
        videoTemplateSelectionAttempts < 3
      ) {
        const templateAttemptIndex = videoTemplateSelectionAttempts;
        videoTemplateSelectionAttempts += 1;
        const templateSelection = await trySelectVideoTemplateCard(
          page,
          timeoutMs,
          templateAttemptIndex,
        ).catch((error) => ({
          clicked: false,
          reason: "template_selection_error",
          errorMessage: error instanceof Error ? error.message : String(error),
        }));
        log(
          "video template selection attempt",
          JSON.stringify({
            operation,
            pollCount,
            attempt: videoTemplateSelectionAttempts,
            result: templateSelection,
            bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
          }),
        );
        if (templateSelection?.clicked) {
          videoTemplateSelectedAt = Date.now();
          if (templateSelection?.videoCreateClicked) {
            videoCreateClickedAt = Date.now();
            videoCreateClickAttempts += 1;
          }
          continue;
        }
      }

      if (
        operation === "video" &&
        media.length === 0 &&
        videoTemplateSelectedAt &&
        Date.now() - videoTemplateSelectedAt < 20_000 &&
        /创作视频|制作视频|Create video/i.test(String(lastSnapshot?.pageState?.bodyText || "")) &&
        videoCreateClickAttempts < 3 &&
        (!videoCreateClickedAt || Date.now() - videoCreateClickedAt > 3_000)
      ) {
        const videoCreateClicked = await tryClickVideoCreateAction(page, timeoutMs).catch(
          () => false,
        );
        videoCreateClickAttempts += 1;
        if (videoCreateClicked) {
          videoCreateClickedAt = Date.now();
        }
        log(
          "video create action attempt",
          JSON.stringify({
            operation,
            pollCount,
            attempt: videoCreateClickAttempts,
            clicked: videoCreateClicked,
            bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
          }),
        );
        if (videoCreateClicked) {
          await page.waitForTimeout(1_500);
          continue;
        }
      }

      const providerGate =
        detectMediaProviderGate(operation, quotaGateText)
        || detectMediaProviderGate(operation, lastSnapshot?.pageState?.bodyText)
        || detectMediaProviderGate(operation, recentMediaProviderGateText(capture.state));
      if (providerGate) {
        if (
          operation === "video" &&
          videoTemplateSelectedAt &&
          Date.now() - videoTemplateSelectedAt < 20_000
        ) {
          log(
            "video provider gate deferred after template selection",
            JSON.stringify({
              operation,
              pollCount,
              code: providerGate.code,
              msSinceTemplateSelection: Date.now() - videoTemplateSelectedAt,
              bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
            }),
          );
          await page.waitForTimeout(2_000);
          continue;
        }
        log(
          "media provider gate detected",
          JSON.stringify({
            operation,
            pollCount,
            code: providerGate.code,
            bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
          }),
        );
        throw Object.assign(new Error(providerGate.message), {
          status: providerGate.status,
          code: providerGate.code,
          bodyText: lastSnapshot?.pageState?.bodyText ?? null,
          captureState: capture.state,
        });
      }

      const needsPlayerReadyTargetBoost =
        media.length === 0 &&
        (operation === "music" || operation === "video") &&
        /_player_ready$/i.test(String(capture.state.invokeContract?.uiState || "")) &&
        !normalizeString(capture.state.invokeContract?.targetSource);
      if (needsPlayerReadyTargetBoost && !playerReadyPlayAttempted) {
        playerReadyPlayAttempted = true;
        const playClicked = await clickMediaActionButton(page, operation, "play", timeoutMs).catch(
          () => false,
        );
        log(
          "player-ready play target probe",
          JSON.stringify({
            operation,
            pollCount,
            clicked: playClicked,
          }),
        );
        if (playClicked) {
          await page.waitForTimeout(1800);
          continue;
        }
      }
      if (needsPlayerReadyTargetBoost && !playerReadyDownloadAttempted) {
        playerReadyDownloadAttempted = true;
        const downloadClicked = await clickMediaActionButton(
          page,
          operation,
          "download",
          timeoutMs,
        ).catch(() => false);
        log(
          "player-ready download target probe",
          JSON.stringify({
            operation,
            pollCount,
            clicked: downloadClicked,
          }),
        );
        if (downloadClicked) {
          await page.waitForTimeout(2200);
          continue;
        }
      }

      if (media.length > 0) {
        const finalInvokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            operation,
            capture.state.actionContract,
            capture.state.transportHints,
            {
              bodyText: lastSnapshot?.pageState?.bodyText ?? "",
              buttons: lastSnapshot?.buttons ?? [],
              anchorNodes: lastSnapshot?.anchorNodes ?? [],
              mediaNodes: lastSnapshot?.mediaNodes ?? [],
            },
            prompt,
            capture.state,
          ),
        );
        capture.state.invokeContract = finalInvokeContract;
        const refreshedMedia = selectMediaAssetsForOperation(operation, lastSnapshot, capture.state);
        let resolvedMedia = refreshedMedia.length > 0 ? refreshedMedia : media;
        if (operation === "image" || operation === "music") {
          resolvedMedia = [];
          for (const asset of (refreshedMedia.length > 0 ? refreshedMedia : media)) {
            if (operation === "image" && asset.kind !== "image") {
              resolvedMedia.push(asset);
              continue;
            }
            if (operation === "music" && asset.kind !== "audio") {
              resolvedMedia.push(asset);
              continue;
            }
            try {
              if (operation === "image") {
                const image = await extractImageBytes(page, asset);
                resolvedMedia.push({
                  ...asset,
                  mimeType: image.mimeType,
                  bodyBase64: image.bodyBase64,
                });
              } else {
                const audio = await extractAudioBytes(page, asset);
                resolvedMedia.push({
                  ...asset,
                  mimeType: audio.mimeType,
                  bodyBase64: audio.bodyBase64,
                });
              }
            } catch (error) {
              const imageDeferred =
                operation === "image"
                && error instanceof Error
                && (
                  error.code === "gemini_canvas_non_image_asset"
                  || error.code === "gemini_canvas_image_fetch_failed"
                );
              const musicDeferred =
                operation === "music"
                && error instanceof Error
                && (
                  error.code === "gemini_canvas_tts_non_audio_asset"
                  || error.code === "gemini_canvas_tts_audio_fetch_failed"
                );
              if (imageDeferred || musicDeferred) {
                log(
                  `${operation} asset extraction deferred to gateway`,
                  JSON.stringify({
                    url: asset.url,
                    code: error.code ?? null,
                    mimeType: error.mimeType ?? null,
                  }),
                );
                resolvedMedia.push(asset);
                continue;
              }
              throw error;
            }
          }
        }
        if (operation === "music") {
          const hasInlineAudio = resolvedMedia.some(
            (asset) =>
              asset.kind === "audio"
              && typeof asset.bodyBase64 === "string"
              && asset.bodyBase64.length > 0,
          );
          const hasAudioTargetCandidate = (finalInvokeContract?.targetCandidates || []).some(
            (candidate) =>
              (normalizeString(candidate?.kind) === "audio")
              || isAudioLikeUrl(candidate?.url)
              || isAudioLikeMimeType(candidate?.mimeType),
          );
          const musicPendingOrBusy = bodyIndicatesMusicPendingOrBusy(
            lastSnapshot?.pageState?.bodyText ?? "",
          );
          if (
            !hasInlineAudio
            && !hasAudioTargetCandidate
            && !musicPendingOrBusy
            && Date.now() + 1600 < deadline
          ) {
            log(
              "music media awaiting streamgenerate settlement",
              JSON.stringify({
                pollCount,
                media: resolvedMedia,
              }),
            );
            await page.waitForTimeout(1600);
            continue;
          }
        }
        log(
          "media operation resolved",
          JSON.stringify({
            operation,
            pollCount,
            media: resolvedMedia,
          }),
        );
        return {
          operation,
          pageUrl: lastSnapshot.pageState?.url ?? page.url(),
          bodyText: lastSnapshot.pageState?.bodyText ?? null,
          ...buildProgramHandleState(
            normalizeString(args.baseUrl) ?? "https://gemini.google.com",
            args,
            lastSnapshot.pageState?.url ?? page.url(),
            capture.state,
          ),
          canvasProgramInvokeContract: finalInvokeContract,
          media: resolvedMedia,
          networkEvents: capture.state.events,
          rpcCaptures: capture.state.rpcCaptures,
        };
      }

      await page.waitForTimeout(2000);
    }

    throw Object.assign(
      new Error(`Timed out waiting for Gemini Canvas ${operation} result.`),
      {
        status: 504,
        code: "gemini_canvas_media_timeout",
        bodyText: lastSnapshot?.pageState?.bodyText ?? null,
        captureState: capture.state,
      },
    );
  } finally {
    log("runMediaOperation finally start", JSON.stringify({ operation }));
    capture.stop();
    if (pageLease.closeWhenDone) {
      await closePageSafely(page, `runMediaOperation:${operation}`);
    }
    log("runMediaOperation finally done", JSON.stringify({ operation }));
  }
}

async function runFetchOperation(entry, args) {
  const fetchRequest = normalizeObject(args.fetchRequest);
  const url = normalizeString(fetchRequest.url);
  if (!url) {
    throw Object.assign(
      new Error("Gemini Canvas browser fetch requires fetchRequest.url."),
      {
        status: 400,
        code: "gemini_canvas_invalid_fetch_request",
      },
    );
  }

  const method = normalizeString(fetchRequest.method)?.toUpperCase() ?? "GET";
  const useCanvasProxyMode = normalizeString(args.googleFetchMode) === "canvas_proxy";
  const useCanvasPreviewNoKeyMode =
    normalizeString(args.googleFetchMode) === "canvas_preview_no_key";
  const useCanvasPreviewMusicNoKeyMode =
    normalizeString(args.googleFetchMode) === "canvas_preview_music_no_key";
  const useCanvasPageNoKeyMode =
    normalizeString(args.googleFetchMode) === "canvas_page_no_key";
  const useCanvasPageMusicNoKeyMode =
    normalizeString(args.googleFetchMode) === "canvas_page_music_no_key";
  const requestedHeaders = useCanvasProxyMode
    ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
    : useCanvasPreviewNoKeyMode
    ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
    : useCanvasPreviewMusicNoKeyMode
    ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
    : useCanvasPageNoKeyMode
    ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
    : useCanvasPageMusicNoKeyMode
    ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
    : sanitizeBrowserFetchHeaders({
        ...normalizeObject(fetchRequest.headers),
        ...(await buildGoogleFetchAuthHeaders(
          entry,
          normalizeString(args.baseUrl) ?? "https://gemini.google.com",
          url,
        )),
      });
  const timeoutMs = Math.max(
    Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
    DEFAULT_TIMEOUT_MS,
  );
  const fixtureBaseUrl = normalizeString(args.baseUrl);
  const requireSharePage = args.requireSharePage === true;
  const preferredProgramPageUrl = resolveProgramPageUrl(
    fixtureBaseUrl ?? "https://gemini.google.com",
    args,
  );
  const attachedAppPage = await findAttachedGeminiAppPage(
    entry,
    fixtureBaseUrl ?? "https://gemini.google.com",
  );
  const originalPage = entry.page;
  if (attachedAppPage && attachedAppPage !== entry.page) {
    await attachedAppPage.bringToFront().catch(() => undefined);
    entry.page = attachedAppPage;
    log(
      "reusing attached Gemini app page for fetch operation",
      JSON.stringify({
        pageUrl: normalizeString(attachedAppPage.url()) ?? "",
        runtimeStatePath: entry.runtimeStatePath,
      }),
    );
  }
  const capture = startNetworkCapture(entry.page, "text");
  let connectedClientBootstrapError = null;
  try {
  const requestBodyText =
    typeof fetchRequest.bodyText === "string"
      ? fetchRequest.bodyText
      : fetchRequest.jsonBody !== undefined
        ? JSON.stringify(fetchRequest.jsonBody)
        : null;
  const requestReferrer =
    preferredProgramPageUrl ||
    (typeof fetchRequest.referrer === "string" && fetchRequest.referrer.trim()
      ? fetchRequest.referrer.trim()
      : null);
  const requestReferrerPolicy =
    typeof fetchRequest.referrerPolicy === "string" && fetchRequest.referrerPolicy.trim()
      ? fetchRequest.referrerPolicy.trim()
      : null;
  const connectedRequestSpec = {
    method,
    url,
    headers: requestedHeaders,
    body: requestBodyText,
    referrer: requestReferrer,
    referrerPolicy: requestReferrerPolicy,
  };
  if (useCanvasPreviewNoKeyMode || useCanvasPreviewMusicNoKeyMode) {
    const previewFetchRequest = {
      url,
      method,
      headers: requestedHeaders,
      bodyText: requestBodyText,
    };
    const preview = await ensureCanvasProxyPreviewFrame(
      entry,
      fixtureBaseUrl ?? "https://gemini.google.com",
      normalizeString(args.shareId),
      timeoutMs,
      useCanvasPreviewMusicNoKeyMode ? null : previewFetchRequest,
    );
    const probeFetchResult =
      !useCanvasPreviewMusicNoKeyMode
        ? preview.stampedFrames.find((entry) => entry?.stamped && entry?.probeFetchResult)
            ?.probeFetchResult
        : null
      ?? null;
    const result = probeFetchResult
      ? {
          status: probeFetchResult.status ?? 599,
          ok: probeFetchResult.ok === true,
          finalUrl: normalizeString(probeFetchResult.url) ?? previewFetchRequest.url,
          contentType: normalizeString(probeFetchResult.contentType),
          headers: {},
          bodyText:
            typeof probeFetchResult.bodyText === "string"
              ? probeFetchResult.bodyText
              : normalizeString(probeFetchResult.bodyPreview) ?? null,
          bodyBase64:
            typeof probeFetchResult.bodyText === "string"
              ? Buffer.from(probeFetchResult.bodyText, "utf8").toString("base64")
              : typeof probeFetchResult.bodyPreview === "string"
                ? Buffer.from(probeFetchResult.bodyPreview, "utf8").toString("base64")
              : null,
          errorMessage: probeFetchResult.ok ? null : normalizeString(probeFetchResult.errorMessage),
          errorName: probeFetchResult.ok ? null : "PreviewProbeFetchError",
        }
      : useCanvasPreviewMusicNoKeyMode
      ? await executeCanvasProxyPreviewNoKeyMusic(
          preview.frame,
          {
            url: previewFetchRequest.url,
            jsonBody:
              fetchRequest.jsonBody !== undefined
                ? fetchRequest.jsonBody
                : typeof previewFetchRequest.bodyText === "string"
                ? (() => {
                    try {
                      return JSON.parse(previewFetchRequest.bodyText);
                    } catch {
                      return null;
                    }
                  })()
                : null,
          },
          timeoutMs,
        )
      : await executeCanvasProxyPreviewNoKeyFetch(
          preview.frame,
          previewFetchRequest,
          timeoutMs,
        );
    if (!result.ok && result.errorMessage) {
      const diagnostics = {
        previewFrameUrl: preview.frame?.url?.() ?? null,
        probeResult: result,
        networkEvents: capture.state.events.slice(-40),
        rpcCaptures: capture.state.rpcCaptures.slice(-20),
      };
      throw Object.assign(
        new Error(`Gemini Canvas preview-frame no-key fetch failed: ${result.errorName || "Error"} ${result.errorMessage}`),
        {
          status: result.status || 599,
          code: useCanvasPreviewMusicNoKeyMode
            ? "gemini_canvas_preview_music_no_key_fetch_failed"
            : "gemini_canvas_preview_no_key_fetch_failed",
          bodyText:
            typeof result.bodyText === "string" && result.bodyText.trim()
              ? result.bodyText
              : JSON.stringify(diagnostics),
        },
      );
    }
    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        preview.page.url(),
        capture.state,
      ),
      ...result,
      previewFrameUrl: preview.frame.url(),
      previewProbe: preview.stampedFrames,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  if (preferredProgramPageUrl && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
    await ensureProgramPage(
      entry,
      fixtureBaseUrl ?? "https://gemini.google.com",
      preferredProgramPageUrl,
      timeoutMs,
    );
  }

  if (useCanvasPageMusicNoKeyMode) {
    const result = await executeCanvasProgramPageNoKeyMusic(
      entry.page,
      {
        url,
        jsonBody:
          fetchRequest.jsonBody !== undefined
            ? fetchRequest.jsonBody
            : typeof fetchRequest.bodyText === "string"
            ? (() => {
                try {
                  return JSON.parse(fetchRequest.bodyText);
                } catch {
                  return null;
                }
              })()
            : null,
      },
      timeoutMs,
    );
    if (!result.ok && result.errorMessage) {
      throw Object.assign(
        new Error(`Gemini Canvas page-context no-key music websocket failed: ${result.errorName || "Error"} ${result.errorMessage}`),
        {
          status: result.status || 599,
          code: "gemini_canvas_page_music_no_key_fetch_failed",
          bodyText: result.bodyText,
        },
      );
    }
    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        entry.page.url(),
        capture.state,
      ),
      ...result,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  if ((useCanvasProxyMode || requireSharePage) && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
    log(
      useCanvasProxyMode
        ? "fetch operation using canvas proxy mode"
        : "fetch operation preparing Gemini share-page context",
      url,
    );
    await ensureSharePage(
      entry,
      fixtureBaseUrl ?? "https://gemini.google.com",
      normalizeString(args.shareId),
      timeoutMs,
    );
  }

  if (preferredProgramPageUrl && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
    await ensureProgramPage(
      entry,
      fixtureBaseUrl ?? "https://gemini.google.com",
      preferredProgramPageUrl,
      timeoutMs,
    );
  }

  if (useCanvasProxyMode) {
    try {
      await ensureLoopbackConnectedClient(entry);
    } catch (error) {
      connectedClientBootstrapError = error;
      log(
        "failed to bootstrap Gemini Canvas loopback connected client",
        error instanceof Error ? error.message : String(error),
      );
    }
  }

  if (useCanvasProxyMode && listConnectedClients().length > 0) {
    try {
      const connectedResult = await dispatchConnectedProxyRequest({
        method,
        url,
        headers: requestedHeaders,
        ...connectedRequestSpec,
      });
      return {
        operation: "fetch",
        ...buildProgramHandleState(
          fixtureBaseUrl ?? "https://gemini.google.com",
          args,
          entry.page.url(),
          capture.state,
        ),
        status: connectedResult.status,
        ok: connectedResult.status >= 200 && connectedResult.status < 300,
        finalUrl: url,
        contentType:
          connectedResult.headers["content-type"] ??
          connectedResult.headers["Content-Type"] ??
          null,
        headers: connectedResult.headers,
        bodyText: connectedResult.bodyText,
        bodyBase64: Buffer.from(connectedResult.bodyText ?? "", "utf8").toString("base64"),
        networkEvents: capture.state.events,
        rpcCaptures: capture.state.rpcCaptures,
      };
    } catch (error) {
      connectedClientBootstrapError = error;
      log(
        "Gemini Canvas connected client fetch failed; trying browser navigation fallback",
        error instanceof Error ? error.message : String(error),
      );
    }
  }

  if (useCanvasProxyMode && method === "GET" && isBrowserDownloadAssetUrl(url)) {
    const navigationResult = await downloadBinaryViaNavigation(entry, url, timeoutMs);
    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        navigationResult.finalUrl ?? entry.page.url(),
        capture.state,
      ),
      ...navigationResult,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  if (useCanvasProxyMode) {
    throw Object.assign(
      new Error(
        connectedClientBootstrapError instanceof Error
          ? `Gemini Canvas program websocket path is unavailable: ${connectedClientBootstrapError.message}`
          : "Gemini Canvas program websocket path is unavailable because no connected client is attached.",
      ),
      {
        status: 503,
        code:
          connectedClientBootstrapError instanceof Error
            ? "gemini_canvas_program_ws_bootstrap_failed"
            : "gemini_canvas_program_ws_client_unavailable",
        bodyText:
          connectedClientBootstrapError instanceof Error
            ? connectedClientBootstrapError.message
            : null,
      },
    );
  }

  if (args.requireAppPage === false && isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
    const response = await fetch(url, {
      method,
      headers: requestedHeaders,
          body: requestBodyText ?? undefined,
      referrer: useCanvasProxyMode
        ? undefined
        : requestReferrer
          ? requestReferrer
          : undefined,
      referrerPolicy: useCanvasProxyMode
        ? undefined
        : requestReferrerPolicy
          ? requestReferrerPolicy
          : undefined,
    });
    const bodyBuffer = Buffer.from(await response.arrayBuffer());
    const responseHeaders = {};
    response.headers.forEach((value, key) => {
      responseHeaders[key] = value;
    });
    const contentType = response.headers.get("content-type");
    const bodyTextResult =
      contentType && /(json|text|javascript|xml|html)/i.test(contentType)
        ? bodyBuffer.toString("utf8")
        : null;
    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        response.url,
        capture.state,
      ),
      status: response.status,
      ok: response.ok,
      finalUrl: response.url,
      contentType,
      headers: responseHeaders,
      bodyText: bodyTextResult,
      bodyBase64: bodyBuffer.toString("base64"),
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  let result;
  try {
    result = await entry.page.evaluate(
    async ({ url, method, headers, bodyText, timeoutMs, referrer, referrerPolicy, useCanvasProxyMode }) => {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), timeoutMs);
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      const toBase64 = (bytes) => {
        let binary = "";
        const chunkSize = 0x8000;
        for (let index = 0; index < bytes.length; index += chunkSize) {
          const chunk = bytes.subarray(index, index + chunkSize);
          binary += String.fromCharCode(...chunk);
        }
        return btoa(binary);
      };

      try {
        console.debug?.("[gemini-canvas-fetch] start", url);
        const response = await fetch(url, {
          method,
          headers,
          body: typeof bodyText === "string" ? encoder.encode(bodyText) : undefined,
          credentials: useCanvasProxyMode ? "same-origin" : "include",
          mode: "cors",
          referrer:
            useCanvasProxyMode || !(typeof referrer === "string" && referrer)
              ? undefined
              : referrer,
          referrerPolicy:
            useCanvasProxyMode || !(typeof referrerPolicy === "string" && referrerPolicy)
              ? undefined
              : referrerPolicy,
          signal: controller.signal,
        });
        const arrayBuffer = await response.arrayBuffer();
        const bodyBytes = new Uint8Array(arrayBuffer);
        const responseHeaders = {};
        response.headers.forEach((value, key) => {
          responseHeaders[key] = value;
        });
        const contentType = response.headers.get("content-type");
        const bodyTextResult =
          contentType && /(json|text|javascript|xml|html)/i.test(contentType)
            ? decoder.decode(bodyBytes)
            : null;
        console.debug?.("[gemini-canvas-fetch] done", response.status, response.url);

        return {
          status: response.status,
          ok: response.ok,
          finalUrl: response.url,
          contentType,
          headers: responseHeaders,
          bodyText: bodyTextResult,
          bodyBase64: toBase64(bodyBytes),
        };
      } finally {
        clearTimeout(timeout);
      }
    },
    {
      url,
      method,
      headers: requestedHeaders,
      bodyText: requestBodyText,
      referrer: requestReferrer,
      referrerPolicy: requestReferrerPolicy,
      timeoutMs,
      useCanvasProxyMode,
    },
  );
  } catch (error) {
    if (shouldAttemptConnectedClientFetchFallback(error, { method, useCanvasProxyMode })) {
      try {
        if (listConnectedClients().length === 0) {
          await ensureLoopbackConnectedClient(entry);
        }
      } catch (connectedClientError) {
        connectedClientBootstrapError = connectedClientError;
        log(
          "failed to bootstrap Gemini Canvas loopback connected client after page.evaluate fetch error",
          connectedClientError instanceof Error ? connectedClientError.message : String(connectedClientError),
        );
      }
      if (listConnectedClients().length > 0) {
        log(
          "page.evaluate fetch failed; retrying Gemini Canvas fetch through connected client fallback",
          url,
        );
        const connectedResult = await dispatchConnectedProxyRequest(connectedRequestSpec);
        return {
          operation: "fetch",
          ...buildProgramHandleState(
            fixtureBaseUrl ?? "https://gemini.google.com",
            args,
            entry.page.url(),
            capture.state,
          ),
          status: connectedResult.status,
          ok: connectedResult.status >= 200 && connectedResult.status < 300,
          finalUrl: url,
          contentType:
            connectedResult.headers["content-type"] ??
            connectedResult.headers["Content-Type"] ??
            null,
          headers: connectedResult.headers,
          bodyText: connectedResult.bodyText,
          bodyBase64: Buffer.from(connectedResult.bodyText ?? "", "utf8").toString("base64"),
          networkEvents: capture.state.events,
          rpcCaptures: capture.state.rpcCaptures,
        };
      }
    }
    throw error;
  }

  return {
    operation: "fetch",
    ...buildProgramHandleState(
      fixtureBaseUrl ?? "https://gemini.google.com",
      args,
      result.finalUrl ?? entry.page.url(),
      capture.state,
    ),
    ...result,
    networkEvents: capture.state.events,
    rpcCaptures: capture.state.rpcCaptures,
  };
  } finally {
    entry.page = originalPage;
    capture.stop();
  }
}

async function invokeGeminiCanvas(args) {
  const runtimeStateObjectKey = normalizeString(args.runtimeStateObjectKey);
  const hasFetchRequest = Boolean(args?.fetchRequest);
  log(
    "invokeGeminiCanvas start",
    JSON.stringify({
      runtimeStateObjectKey,
      operation: normalizeString(args?.operation),
      requireAppPage: args?.requireAppPage !== false,
      hasFetchRequest,
    }),
  );
  if (!runtimeStateObjectKey) {
    return {
      ok: false,
      error: {
        code: "gemini_canvas_invalid_request",
        message: "runtimeStateObjectKey is required.",
        status: 400,
      },
    };
  }

  let entry = null;
  try {
    await evictIfOverCapacity();
    entry = await ensureContext({
      runtimeStateObjectKey,
      baseUrl: args.baseUrl,
      locale: args.locale,
      browserExecutablePath: args.browserExecutablePath,
      browserCdpUrl: args.browserCdpUrl,
      timeoutMs: args.timeoutMs,
    });
    if (entry.busy) {
      return {
        ok: false,
        error: {
          code: "gemini_canvas_context_busy",
          message: "Gemini Canvas browser context is busy.",
          status: 429,
        },
      };
    }

    entry.busy = true;
    entry.lastUsedAt = Date.now();
    if (!entry.page || entry.page.isClosed()) {
      entry.page = await entry.context.newPage();
      await entry.page.bringToFront().catch(() => undefined);
      log("recreated closed Gemini Canvas shared page", runtimeStateObjectKey);
    }
    const cookieSyncBaseUrl = normalizeString(args.baseUrl) ?? "https://gemini.google.com";
    const runtimeHasAuthCookies = await contextHasGeminiAuthCookies(entry.context, cookieSyncBaseUrl);
    const defaultCookieSyncEnabled =
      entry.runtimeStateMode === "storage_state_file" ||
      entry.launchClonedProfile === true ||
      runtimeHasAuthCookies === false;
    const cookieSyncEnabled = parseBoolean(
      args.forceCookieSync ?? process.env.GEMINI_CANVAS_BROWSER_FORCE_COOKIE_SYNC,
      defaultCookieSyncEnabled,
    );
    const runtimeAlreadyHasAuthCookies =
      cookieSyncEnabled &&
      entry.runtimeStateMode !== "storage_state_file" &&
      entry.launchClonedProfile !== true
        ? runtimeHasAuthCookies
        : false;
    const synchronizedCookieCount =
      cookieSyncEnabled && !runtimeAlreadyHasAuthCookies
        ? await syncCookieHeaderIntoContext(entry.context, args.cookieHeader, cookieSyncBaseUrl).catch(
            (error) => {
              log(
                "cookie sync failed",
                runtimeStateObjectKey,
                error instanceof Error ? error.message : String(error),
              );
              return 0;
            },
          )
        : 0;
    if (!cookieSyncEnabled) {
      log("skipped cookie sync because runtime mirroring is authoritative", runtimeStateObjectKey);
    } else if (runtimeAlreadyHasAuthCookies) {
      log("skipped cookie sync because runtime already has Gemini auth cookies", runtimeStateObjectKey);
    }
    if (synchronizedCookieCount > 0) {
      log("synced Gemini auth cookies into browser context", runtimeStateObjectKey, synchronizedCookieCount);
    }
    const operation = normalizeString(args.operation);
    if (operation === "bootstrap_program") {
      const result = await runBootstrapProgramOperation(entry, args);
      return {
        ok: true,
        result,
      };
    }
    const preferredProgramPageUrl = resolveProgramPageUrl(
      normalizeString(args.baseUrl) ?? "https://gemini.google.com",
      args,
    );
    if (args.enforceProgramOwner === true && !preferredProgramPageUrl) {
      return {
        ok: false,
        error: {
          code: "gemini_canvas_program_handle_required",
          message:
            "Gemini Canvas program-owned relay requires a concrete canvasProgramUrl/appPath/conversationId handle.",
          status: 400,
        },
      };
    }
    const shouldDeferAppPageEnsureToFetchOperation =
      args.requireAppPage !== false && hasFetchRequest && entry.attachedCdp === true;
    if (shouldDeferAppPageEnsureToFetchOperation) {
      log(
        "deferring outer ensureAppPage to fetch operation for attached CDP request",
        JSON.stringify({
          runtimeStateObjectKey,
          browserCdpUrl: normalizeString(args.browserCdpUrl) ?? null,
        }),
      );
    } else if (args.requireAppPage !== false) {
      if (preferredProgramPageUrl) {
        await ensureProgramPage(
          entry,
          normalizeString(args.baseUrl) ?? "https://gemini.google.com",
          preferredProgramPageUrl,
          Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
        );
      } else {
        await ensureAppPage(
          entry,
          normalizeString(args.baseUrl) ?? "https://gemini.google.com",
          Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
          {
            cookieHeader: args.cookieHeader,
            skipInitialNavigationWhenAppSurfaceReady: entry.attachedCdp === true,
          },
        );
      }
    }
    const result = hasFetchRequest
      ? await runFetchOperation(entry, args)
      : operation === "text"
        ? await runTextOperation(entry, args)
        : operation === "tts"
          ? await runTtsOperation(entry, args)
          : operation === "debug"
            ? await runDebugOperation(entry, args)
          : await runMediaOperation(entry, args);
    return {
      ok: true,
      result,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const status = Number(error?.status ?? 500);
    if ([401, 403].includes(status)) {
      await closeContext(runtimeStateObjectKey);
    }
    log(
      "invokeGeminiCanvas returning error",
      JSON.stringify({
        runtimeStateObjectKey,
        operation: normalizeString(args.operation),
        status,
        code: error?.code ?? "gemini_canvas_browser_worker_failed",
        message,
      }),
    );
    return {
      ok: false,
      error: {
        code: error?.code ?? "gemini_canvas_browser_worker_failed",
        message,
        status,
        body: error?.bodyText ?? null,
      },
    };
  } finally {
    if (entry) {
      entry.busy = false;
      entry.lastUsedAt = Date.now();
    }
    log(
      "invokeGeminiCanvas finally release",
      JSON.stringify({
        runtimeStateObjectKey,
        operation: normalizeString(args.operation),
        hadEntry: Boolean(entry),
      }),
    );
  }
}

function sendJson(res, status, body) {
  const json = Buffer.from(JSON.stringify(body));
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": String(json.length),
    "cache-control": "no-store",
  });
  res.end(json);
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
    req.on("end", () => {
      try {
        const raw = Buffer.concat(chunks).toString("utf8");
        resolve(raw ? JSON.parse(raw) : {});
      } catch (error) {
        reject(error);
      }
    });
    req.on("error", reject);
  });
}

async function main() {
  const host = normalizeString(process.env.GEMINI_CANVAS_BROWSER_HOST) ?? DEFAULT_HOST;
  const port = Number(process.env.GEMINI_CANVAS_BROWSER_POOL_PORT || DEFAULT_PORT);
  const tlsPort = Number(process.env.GEMINI_CANVAS_BROWSER_POOL_TLS_PORT || DEFAULT_TLS_PORT);
  await mkdir(getStorageRoot(), { recursive: true }).catch(() => undefined);
  const wsServer = new WebSocketServer({ noServer: true });

  setInterval(() => {
    evictIdleContexts().catch((error) => log("idle eviction failed", error.message));
  }, 5 * 60 * 1000).unref();

  const requestHandler = async (req, res) => {
    try {
      if (req.method === "GET" && req.url === "/health") {
        return sendJson(res, 200, {
          ok: true,
          contexts: contexts.size,
          endpoints: {
            httpBaseUrl: `http://${host}:${port}`,
            httpsBaseUrl: `https://${host}:${tlsPort}`,
            wsEndpoint: `ws://${host}:${port}/ws`,
            wssEndpoint: `wss://${host}:${tlsPort}/ws`,
          },
          connectedClients: listConnectedClients().map((entry) => ({
            clientLabel: entry.clientLabel,
            connectedAt: entry.connectedAt,
          })),
        });
      }

      if (req.method === "POST" && req.url === "/invoke") {
        const body = await readJsonBody(req);
        const result = await invokeGeminiCanvas(body);
        return sendJson(res, result.ok ? 200 : Number(result.error?.status ?? 500), result);
      }

      if (req.method === "POST" && req.url === "/fetch") {
        const body = await readJsonBody(req);
        const result = await invokeGeminiCanvas({
          ...body,
          fetchRequest: body.fetchRequest ?? body,
        });
        return sendJson(res, result.ok ? 200 : Number(result.error?.status ?? 500), result);
      }

      return sendJson(res, 404, {
        ok: false,
        error: {
          code: "not_found",
          message: "Unknown browser pool endpoint.",
          status: 404,
        },
      });
    } catch (error) {
      return sendJson(res, 500, {
        ok: false,
        error: {
          code: "gemini_canvas_browser_pool_server_error",
          message: error instanceof Error ? error.message : String(error),
          status: 500,
        },
      });
    }
  };

  function attachUpgradeHandler(server, scheme, listenPort) {
    server.on("upgrade", (req, socket, head) => {
      try {
        const requestUrl = new URL(req.url || "/", `${scheme}://${req.headers.host || `${host}:${listenPort}`}`);
        if (requestUrl.pathname !== "/ws") {
          socket.destroy();
          return;
        }
        wsServer.handleUpgrade(req, socket, head, (ws) => {
          const connectionId = nextConnectedRequestId();
          const entry = {
            connectionId,
            ws,
            authenticated: false,
            clientLabel: null,
            connectedAt: new Date().toISOString(),
            scheme,
          };
          connectedClients.set(connectionId, entry);
          ws.on("message", (message) => handleConnectedClientMessage(connectionId, message));
          ws.on("close", () => cleanupConnectedClient(connectionId));
          ws.on("error", () => cleanupConnectedClient(connectionId));
        });
      } catch {
        socket.destroy();
      }
    });
  }

  const httpServer = createServer(requestHandler);
  attachUpgradeHandler(httpServer, "http", port);
  httpServer.listen(port, host, () => {
    log(`listening on http://${host}:${port}`);
  });

  const tlsBundle = loadOrCreateTlsCertificate(host);
  const httpsServer = createHttpsServer(
    {
      key: tlsBundle.key,
      cert: tlsBundle.cert,
    },
    requestHandler,
  );
  attachUpgradeHandler(httpsServer, "https", tlsPort);
  httpsServer.listen(tlsPort, host, () => {
    log(`listening on https://${host}:${tlsPort}`);
    log(
      `${tlsBundle.generated ? "generated" : "loaded"} TLS certificate`,
      tlsBundle.certPath,
      tlsBundle.keyPath,
    );
  });
}

if (process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN !== "1") {
  main().catch((error) => {
    console.error("[gemini-canvas-pool] fatal:", error);
    process.exit(1);
  });
}
