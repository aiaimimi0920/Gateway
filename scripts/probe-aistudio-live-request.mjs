/**
 * Interactive page-owned AI Studio live probe.
 *
 * Purpose:
 * - Open a real AI Studio page from an existing browser-state/profile runtime
 * - Keep the browser visible so a human can log in or submit a prompt
 * - Capture the first real page-owned request contract instead of guessing that
 *   a raw `generateContent` fetch is sufficient
 * - Emit `target-rpc-summary.json` plus numbered pair files when the target
 *   MakerSuite text RPC contracts are captured
 *
 * Input JSON via stdin:
 * {
 *   "runtimeStateObjectKey": "credential-runtime/.../storage-state.json",
 *   "appUrl": "https://ai.studio/apps/...",
 *   "timeoutMs": 300000,
 *   "settleMs": 15000,
 *   "autoPrompt": "Reply with exactly OK.",
 *   "failUnlessTargetRpcCaptured": true,
 *   "responseBodyLimit": 65536,
 *   "browserExecutablePath": "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
 *   "authIndex": 0,
 *   "localWsPort": 9998,
 *   "requestUrlIncludes": ["ai.studio", "generativelanguage.googleapis.com"],
 *   "captureDir": ".runtime/aistudio-live-probe/manual"
 * }
 */

import { chromium } from "playwright-core";
import { existsSync, lstatSync, mkdirSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import net from "node:net";
import crypto from "node:crypto";
import { S3Client, GetObjectCommand, PutObjectCommand } from "@aws-sdk/client-s3";

const DEFAULT_TIMEOUT_MS = 300_000;
const DEFAULT_SETTLE_MS = 15_000;
const DEFAULT_RESPONSE_BODY_LIMIT = 65_536;
const DEFAULT_LOCALE = "zh-CN";
const DEFAULT_APP_URL = "https://ai.studio/apps/fa9cb8e6-4d92-4fb6-a2b1-b947405c22ae";
const DEFAULT_LOCAL_PROXY_MODEL = "gemini-3-flash-preview";
const DEFAULT_LOCAL_PROXY_STREAMING_MODE = "fake";
const DEFAULT_LOCAL_PROXY_DELAY_MS = 1_200;
const DEFAULT_LOCAL_PROXY_PROMPT = "Reply with exactly OK.";
const DEFAULT_URL_PATTERNS = [
  "ai.studio",
  "aistudio.google.com",
  "makersuite.google.com",
  "clients6.google.com",
  "alkalimakersuite-pa.clients6.google.com",
  "generativelanguage.googleapis.com",
  "alkalimakersuite-pa.googleapis.com",
];
const AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
const AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
const AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME = "aistudio-target-rpc-contract.json";

const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
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

let objectStorageClient = null;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeStringArray(value, fallback) {
  if (!Array.isArray(value)) {
    return fallback;
  }
  const normalized = value
    .map((entry) => normalizeString(entry))
    .filter(Boolean);
  return normalized.length ? normalized : fallback;
}

function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
  if (!normalized) return fallback;
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  return fallback;
}

function stripUtf8Bom(text) {
  return typeof text === "string" && text.charCodeAt(0) === 0xfeff
    ? text.slice(1)
    : text;
}

function tryParseJson(text) {
  try {
    return JSON.parse(text);
  } catch (_) {
    return null;
  }
}

function createRequestId(prefix = "req") {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

function createRequestAttemptId(requestId, attemptNumber = 1) {
  return `${requestId}_attempt_${attemptNumber}_${Math.random().toString(36).slice(2, 8)}`;
}

function buildDefaultLocalProxyRequest(input) {
  const explicit = input?.localProxyRequest;
  if (explicit === false || input?.disableLocalProxyRequest === true) {
    return null;
  }

  const requestSpec =
    explicit && typeof explicit === "object" && !Array.isArray(explicit) ? explicit : {};
  const requestId = normalizeString(requestSpec.request_id) ?? createRequestId("req");
  const requestAttemptId =
    normalizeString(requestSpec.request_attempt_id) ?? createRequestAttemptId(requestId, 1);
  const streamingMode =
    normalizeString(requestSpec.streaming_mode) ??
    normalizeString(input?.localProxyStreamingMode) ??
    DEFAULT_LOCAL_PROXY_STREAMING_MODE;
  const method = (normalizeString(requestSpec.method) ?? "POST").toUpperCase();
  // Keep GET/HEAD probes truly bodyless; otherwise we accidentally introduce
  // preflight/CORS behavior that does not exist in the real browser-owned lane.
  const expectsRequestBody = ["POST", "PUT", "PATCH"].includes(method);
  const model =
    normalizeString(requestSpec.model) ??
    normalizeString(input?.localProxyModel) ??
    DEFAULT_LOCAL_PROXY_MODEL;
  const prompt =
    normalizeString(requestSpec.prompt) ??
    normalizeString(input?.localProxyPrompt) ??
    normalizeString(input?.autoPrompt) ??
    DEFAULT_LOCAL_PROXY_PROMPT;
  const pathValue =
    normalizeString(requestSpec.path) ??
    `/v1beta/models/${model}:${streamingMode === "real" ? "streamGenerateContent" : "generateContent"}`;
  const queryParams =
    requestSpec.query_params &&
    typeof requestSpec.query_params === "object" &&
    !Array.isArray(requestSpec.query_params)
      ? requestSpec.query_params
      : streamingMode === "real"
        ? { alt: "sse" }
        : {};
  const headers = normalizeHeadersObject(requestSpec.headers);
  const hasContentType = Object.keys(headers).some(
    (key) => key.toLowerCase() === "content-type",
  );
  if (expectsRequestBody && !hasContentType) {
    headers["content-type"] = "application/json";
  }

  let body = requestSpec.body;
  if (body && typeof body !== "string") {
    body = JSON.stringify(body);
  }
  if ((body === undefined || body === null || body === "") && expectsRequestBody) {
    body = JSON.stringify({
      contents: [
        {
          role: "user",
          parts: [{ text: prompt }],
        },
      ],
    });
  }

  return {
    event_type: "proxy_request",
    request_id: requestId,
    request_attempt_id: requestAttemptId,
    method,
    path: pathValue,
    query_params: queryParams,
    headers,
    body,
    streaming_mode: streamingMode,
    is_generative:
      typeof requestSpec.is_generative === "boolean" ? requestSpec.is_generative : true,
  };
}

async function sendActiveTrigger(page, capture) {
  try {
    await page.evaluate(async () => {
      try {
        await fetch("https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger", {
          method: "GET",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
        });
      } catch (error) {
        console.log("[AIStudioProbe] Active trigger sent");
      }
    });
    capture.autoActions.push({ action: "active-trigger", ok: true });
  } catch (error) {
    capture.autoActions.push({
      action: "active-trigger",
      ok: false,
      error: String(error),
    });
  }
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
      (process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
        process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
        "")
        .trim()
        .toLowerCase(),
    ),
  };
}

function resolveBrowserProxySettings(input) {
  const rawProxy =
    normalizeString(input?.browserProxyUrl) ??
    normalizeString(input?.browser_proxy_url) ??
    normalizeString(input?.browserProxyServer) ??
    normalizeString(input?.browser_proxy_server) ??
    normalizeString(process.env.AISTUDIO_PROBE_BROWSER_PROXY_URL);

  if (!rawProxy || /^none$/i.test(rawProxy)) {
    return null;
  }
  if (/^(direct|no-proxy|no_proxy)$/i.test(rawProxy)) {
    return {
      mode: "direct",
      rawProxy,
      launchProxy: null,
      launchArgs: ["--no-proxy-server"],
    };
  }

  const parsed = new URL(rawProxy);
  const launchProxy = {
    server: `${parsed.protocol}//${parsed.hostname}${parsed.port ? `:${parsed.port}` : ""}`,
  };
  const username = parsed.username ? decodeURIComponent(parsed.username) : null;
  const password = parsed.password ? decodeURIComponent(parsed.password) : null;
  const bypass =
    normalizeString(input?.browserProxyBypass) ??
    normalizeString(input?.browser_proxy_bypass) ??
    normalizeString(input?.browserNoProxy) ??
    normalizeString(input?.browser_no_proxy) ??
    normalizeString(process.env.AISTUDIO_PROBE_BROWSER_PROXY_BYPASS);
  if (username) {
    launchProxy.username = username;
  }
  if (password) {
    launchProxy.password = password;
  }
  if (bypass) {
    launchProxy.bypass = bypass;
  }

  return {
    mode: "proxy",
    rawProxy,
    launchProxy,
    launchArgs: [],
  };
}

function buildBrowserProxyLaunchOptions(browserProxySettings) {
  const options = {
    args: browserProxySettings?.launchArgs ?? [],
  };
  if (browserProxySettings?.launchProxy) {
    options.proxy = browserProxySettings.launchProxy;
  }
  return options;
}

function normalizeProxyHostForReachability(host) {
  const normalized = String(host ?? "").trim().toLowerCase();
  if (normalized.startsWith("[") && normalized.endsWith("]")) {
    return normalized.slice(1, -1);
  }
  return normalized;
}

function isLocalProxyHost(host) {
  return ["127.0.0.1", "localhost", "::1"].includes(
    normalizeProxyHostForReachability(host),
  );
}

function defaultProxyPortForProtocol(protocol) {
  switch (String(protocol ?? "").toLowerCase()) {
    case "http:":
    case "ws:":
      return 80;
    case "https:":
    case "wss:":
      return 443;
    case "socks:":
    case "socks4:":
    case "socks5:":
      return 1080;
    default:
      return null;
  }
}

function buildBrowserProxyUnreachableError({
  host,
  port,
  rawProxy,
  server,
  cause,
}) {
  const causeMessage = cause?.message ? `; cause=${cause.message}` : "";
  const error = new Error(
    `Browser proxy endpoint is not reachable: host=${host}, port=${port}, server=${server}, rawProxy=${rawProxy}${causeMessage}`,
    cause ? { cause } : undefined,
  );
  error.code = "aistudio_browser_proxy_unreachable";
  error.status = 503;
  error.host = host;
  error.port = port;
  error.rawProxy = rawProxy;
  error.server = server;
  return error;
}

async function assertBrowserProxyReachable(
  browserProxySettings,
  timeoutMs = 1500,
) {
  if (
    browserProxySettings?.mode !== "proxy" ||
    !browserProxySettings?.launchProxy?.server
  ) {
    return { ok: true, skipped: true, reason: "browser-proxy-not-configured" };
  }

  const server = browserProxySettings.launchProxy.server;
  const parsed = new URL(server);
  const host = normalizeProxyHostForReachability(parsed.hostname);
  const defaultPort = defaultProxyPortForProtocol(parsed.protocol);
  const port = Number(parsed.port || defaultPort);

  if (!isLocalProxyHost(host)) {
    return { ok: true, skipped: true, reason: "non-local-browser-proxy", host };
  }

  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw buildBrowserProxyUnreachableError({
      host,
      port: parsed.port || null,
      rawProxy: browserProxySettings.rawProxy,
      server,
      cause: new Error("missing or invalid proxy port"),
    });
  }

  const timeout = Math.max(1, Number(timeoutMs) || 1500);
  return await new Promise((resolve, reject) => {
    let settled = false;
    let timer = null;
    const socket = net.createConnection({ host, port });

    const finish = (error) => {
      if (settled) {
        return;
      }
      settled = true;
      if (timer) {
        clearTimeout(timer);
      }
      socket.destroy();
      if (error) {
        reject(
          buildBrowserProxyUnreachableError({
            host,
            port,
            rawProxy: browserProxySettings.rawProxy,
            server,
            cause: error,
          }),
        );
        return;
      }
      resolve({ ok: true, host, port });
    };

    timer = setTimeout(() => {
      finish(new Error(`connect timeout after ${timeout}ms`));
    }, timeout);

    socket.once("connect", () => finish(null));
    socket.once("error", finish);
  });
}

function getStorageRoot() {
  const config = resolveObjectStorageConfig();
  return path.resolve(process.cwd(), config.localDir);
}

async function toBuffer(stream) {
  if (!stream) return Buffer.alloc(0);
  if (Buffer.isBuffer(stream)) return stream;
  if (typeof stream === "object" && typeof stream.transformToByteArray === "function") {
    return Buffer.from(await stream.transformToByteArray());
  }
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("AI Studio live probe object storage is not fully configured.");
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
  const bytes = await toBuffer(response.Body);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, bytes);
  return absolutePath;
}

function buildTargetRpcContractObjectKey(runtimeStateObjectKey) {
  const normalized = normalizeString(runtimeStateObjectKey);
  if (!normalized) {
    return null;
  }
  if (normalized.endsWith("/storage-state.json")) {
    return `${normalized.slice(0, -"/storage-state.json".length)}/${AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}`;
  }
  if (normalized.endsWith("\\storage-state.json")) {
    return `${normalized.slice(0, -"\\storage-state.json".length)}/${AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}`;
  }
  return `${normalized}.${AISTUDIO_TARGET_RPC_CONTRACT_FILE_NAME}`;
}

async function persistJsonObjectMirror(objectKey, payload) {
  const normalizedKey = normalizeString(objectKey);
  if (!normalizedKey) {
    return null;
  }
  const config = resolveObjectStorageConfig();
  const serialized = Buffer.from(`${JSON.stringify(payload, null, 2)}\n`, "utf8");
  const absolutePath = path.join(getStorageRoot(), ...normalizedKey.split("/"));
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, serialized);
  if (config.driver === "s3-compatible") {
    const client = getS3Client(config);
    await client.send(
      new PutObjectCommand({
        Bucket: config.bucket,
        Key: normalizedKey,
        ContentType: "application/json",
        Body: serialized,
      }),
    );
  }
  return absolutePath;
}

async function resolveRuntimeStateSource(runtimeStateObjectKey) {
  const config = resolveObjectStorageConfig();
  const absolutePath = path.join(getStorageRoot(), ...runtimeStateObjectKey.split("/"));

  if (!existsSync(absolutePath) && config.driver !== "local") {
    await mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath);
  }

  if (!existsSync(absolutePath)) {
    throw Object.assign(
      new Error(
        `AI Studio runtimeStateObjectKey '${runtimeStateObjectKey}' could not be resolved to a local file or directory.`,
      ),
      {
        status: 500,
        code: "aistudio_runtime_state_unavailable",
      },
    );
  }

  const stat = lstatSync(absolutePath);
  if (stat.isDirectory()) {
    return { mode: "profile_dir", absolutePath };
  }
  if (stat.isFile() && absolutePath.toLowerCase().endsWith(".json")) {
    return { mode: "storage_state_file", absolutePath };
  }

  throw Object.assign(
    new Error(
      `AI Studio runtimeStateObjectKey must point to a browser profile directory or Playwright storageState JSON file, but '${absolutePath}' is neither.`,
    ),
    {
      status: 400,
      code: "aistudio_runtime_state_invalid_path",
    },
  );
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return stripUtf8Bom(Buffer.concat(chunks).toString("utf8"));
}

function printJsonAndSetExitCode(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exitCode = exitCode;
}

function validateInput(input) {
  if (!normalizeString(input?.runtimeStateObjectKey)) {
    throw new Error("runtimeStateObjectKey is required.");
  }
}

function ensureCaptureDir(customDir) {
  const resolved =
    normalizeString(customDir) ??
    path.join(
      process.cwd(),
      ".runtime",
      "aistudio-live-probe",
      new Date().toISOString().replace(/[:.]/g, "-"),
    );
  mkdirSync(resolved, { recursive: true });
  return resolved;
}

function requestMatches(url, patterns) {
  return patterns.some((pattern) => url.includes(pattern));
}

function previewText(value, limit = 4096) {
  return String(value ?? "").slice(0, limit);
}

function requestMethodAndUrlKey(method, url) {
  const normalizedMethod = normalizeString(method);
  const normalizedUrl = normalizeString(url);
  return normalizedMethod && normalizedUrl ? `${normalizedMethod} ${normalizedUrl}` : null;
}

function getRequestMethod(request) {
  if (!request) {
    return null;
  }
  if (typeof request.method === "function") {
    return request.method();
  }
  return request.method;
}

function getRequestUrl(request) {
  if (!request) {
    return null;
  }
  if (typeof request.url === "function") {
    return request.url();
  }
  return request.url;
}

function removePendingRequestId(pendingByKey, requestId) {
  if (!requestId) {
    return;
  }
  for (const [key, pending] of pendingByKey.entries()) {
    const index = pending.indexOf(requestId);
    if (index < 0) {
      continue;
    }
    pending.splice(index, 1);
    if (!pending.length) {
      pendingByKey.delete(key);
    }
    return;
  }
}

function createCaptureRequestIdFactory(now = () => Date.now()) {
  let sequence = 0;
  return () => {
    sequence += 1;
    return `${now()}-${sequence}`;
  };
}

function createResponseRequestAttributor() {
  const requestIdsByRequest = new WeakMap();
  const pendingByKey = new Map();

  const trackRequest = (request, requestId) => {
    if (!request || typeof request !== "object" || !requestId) {
      return;
    }
    requestIdsByRequest.set(request, requestId);
    const key = requestMethodAndUrlKey(
      getRequestMethod(request),
      getRequestUrl(request),
    );
    if (!key) {
      return;
    }
    const pending = pendingByKey.get(key) ?? [];
    pending.push(requestId);
    pendingByKey.set(key, pending);
  };

  const resolveResponse = (response) => {
    const request =
      response && typeof response.request === "function"
        ? response.request()
        : response?.request;
    if (request && typeof request === "object" && requestIdsByRequest.has(request)) {
      const requestId = requestIdsByRequest.get(request);
      removePendingRequestId(pendingByKey, requestId);
      return requestId;
    }
    const key = requestMethodAndUrlKey(
      getRequestMethod(request),
      typeof response?.url === "function" ? response.url() : response?.url,
    );
    const pending = key ? pendingByKey.get(key) : null;
    if (!pending?.length) {
      return null;
    }
    const requestId = pending.shift() ?? null;
    if (!pending.length) {
      pendingByKey.delete(key);
    }
    return requestId;
  };

  return { trackRequest, resolveResponse };
}

function isAistudioTargetRpcUrl(url) {
  const normalized = normalizeString(url) ?? "";
  return (
    normalized.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH) ||
    normalized.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH)
  );
}

function detectAistudioTargetRpcKind(url) {
  const normalized = normalizeString(url) ?? "";
  if (normalized.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH)) {
    return "code_assistant_offline";
  }
  if (normalized.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH)) {
    return "stream_code_assistant_offline_generation";
  }
  return null;
}

function hasCompleteTargetRpcPair(entries) {
  return entries.some((entry) => entry?.request && entry?.response);
}

function summarizeTargetRpcContracts(capture) {
  const requestById = new Map(
    (Array.isArray(capture?.requests) ? capture.requests : [])
      .filter((entry) => entry && entry.id)
      .map((entry) => [entry.id, entry]),
  );
  const pairs = [];
  for (const response of Array.isArray(capture?.responses) ? capture.responses : []) {
    const kind = detectAistudioTargetRpcKind(response?.url);
    if (!kind) {
      continue;
    }
    const request = response?.requestId ? requestById.get(response.requestId) ?? null : null;
    pairs.push({
      kind,
      request,
      response,
    });
  }
  for (const request of Array.isArray(capture?.requests) ? capture.requests : []) {
    const kind = detectAistudioTargetRpcKind(request?.url);
    if (!kind) {
      continue;
    }
    const alreadyPaired = pairs.some((entry) => entry.request?.id === request.id);
    if (!alreadyPaired) {
      pairs.push({
        kind,
        request,
        response: null,
      });
    }
  }

  const codeAssistantOffline = pairs.filter((entry) => entry.kind === "code_assistant_offline");
  const streamCodeAssistantOfflineGeneration = pairs.filter(
    (entry) => entry.kind === "stream_code_assistant_offline_generation",
  );

  return {
    capturedTargetRpcContract:
      hasCompleteTargetRpcPair(codeAssistantOffline) &&
      hasCompleteTargetRpcPair(streamCodeAssistantOfflineGeneration),
    targetRpcPairCount: pairs.length,
    codeAssistantOfflineCount: codeAssistantOffline.length,
    streamCodeAssistantOfflineGenerationCount:
      streamCodeAssistantOfflineGeneration.length,
    pairs,
  };
}

function redactSensitiveHeaderValue(name, value) {
  const normalized = String(name || "").toLowerCase();
  if (normalized === "authorization" || normalized === "cookie") {
    return "<redacted>";
  }
  return value;
}

function stableHeaderSubset(headers) {
  const preferred = [
    "content-type",
    "origin",
    "referer",
    "x-aistudio-g1-tier",
    "x-aistudio-visit-id",
    "x-browser-copyright",
    "x-browser-validation",
    "x-goog-api-key",
    "x-goog-authuser",
    "x-goog-ext-519733851-bin",
    "x-user-agent",
    "x-browser-channel",
    "x-browser-year",
    "sec-fetch-mode",
    "sec-fetch-site",
    "user-agent",
  ];
  const result = {};
  const source = headers && typeof headers === "object" ? headers : {};
  const sourceByLowercaseName = {};
  for (const [name, value] of Object.entries(source)) {
    const normalizedName = String(name || "").toLowerCase();
    if (
      normalizedName &&
      typeof value === "string" &&
      value.trim() &&
      typeof sourceByLowercaseName[normalizedName] !== "string"
    ) {
      sourceByLowercaseName[normalizedName] = value;
    }
  }
  for (const key of preferred) {
    const value =
      typeof source[key] === "string" && source[key].trim()
        ? source[key]
        : sourceByLowercaseName[key];
    if (typeof value === "string" && value.trim()) {
      result[key] = redactSensitiveHeaderValue(key, value);
    }
  }
  return result;
}

function tryParseJsonValue(text) {
  try {
    return JSON.parse(text);
  } catch (_) {
    return null;
  }
}

function collectStringsDeep(node, acc = []) {
  if (typeof node === "string") {
    acc.push(node);
    return acc;
  }
  if (Array.isArray(node)) {
    for (const entry of node) {
      collectStringsDeep(entry, acc);
    }
    return acc;
  }
  if (node && typeof node === "object") {
    for (const value of Object.values(node)) {
      collectStringsDeep(value, acc);
    }
  }
  return acc;
}

function extractFirstPromptCandidate(text) {
  const parsed = tryParseJsonValue(text);
  const strings = collectStringsDeep(parsed, [])
    .map((entry) => normalizeString(entry))
    .filter(Boolean);
  for (const candidate of strings) {
    if (
      candidate.startsWith("models/") ||
      /^https?:\/\//i.test(candidate) ||
      /^[0-9a-f]{8}-[0-9a-f-]{28,}$/i.test(candidate)
    ) {
      continue;
    }
    if (candidate.length >= 6 && /[A-Za-z]/.test(candidate)) {
      return candidate;
    }
  }
  return null;
}

function extractCodeAssistantAppId(text) {
  const parsed = tryParseJsonValue(text);
  if (!Array.isArray(parsed)) {
    return null;
  }
  const values = [11, 20]
    .map((index) => normalizeString(parsed[index]))
    .filter(Boolean);
  return new Set(values).size === 1 ? values[0] : null;
}

function hasConflictingCodeAssistantAppIdSlots(text) {
  const parsed = tryParseJsonValue(text);
  if (!Array.isArray(parsed)) {
    return false;
  }
  const values = [11, 20]
    .map((index) => normalizeString(parsed[index]))
    .filter(Boolean);
  return new Set(values).size > 1;
}

function extractCodeAssistantModelPath(text) {
  const parsed = tryParseJsonValue(text);
  if (!Array.isArray(parsed)) {
    return null;
  }
  return normalizeString(parsed[7]);
}

function extractStreamCodeAssistantAppId(text) {
  const parsed = tryParseJsonValue(text);
  if (!Array.isArray(parsed)) {
    return null;
  }
  return normalizeString(parsed[3]);
}

function extractStreamGenerationId(text) {
  const parsed = tryParseJsonValue(text);
  if (!Array.isArray(parsed)) {
    return null;
  }
  return normalizeString(parsed[0]);
}

function selectConsistentAppId({
  codeAssistantAppId,
  streamAppId,
  hasCodeAssistantAppIdConflict,
}) {
  if (
    hasCodeAssistantAppIdConflict ||
    (codeAssistantAppId && streamAppId && codeAssistantAppId !== streamAppId)
  ) {
    return null;
  }
  return codeAssistantAppId ?? streamAppId;
}

function selectConsistentGenerationId({
  codeAssistantGenerationId,
  streamGenerationId,
}) {
  if (
    !codeAssistantGenerationId ||
    !streamGenerationId ||
    codeAssistantGenerationId !== streamGenerationId
  ) {
    return null;
  }
  return codeAssistantGenerationId;
}

function extractGenerationIdFromCodeAssistantResponse(text) {
  const parsed = tryParseJsonValue(text);
  return Array.isArray(parsed) ? normalizeString(parsed[0]) : null;
}

function extractCodeAssistantOpaqueToken(text) {
  const parsed = tryParseJsonValue(text);
  return Array.isArray(parsed) && typeof parsed[1] === "string" ? parsed[1] : null;
}

function extractFinalTextFromStreamBodyPreview(text) {
  const normalized = String(text || "");
  const matches = Array.from(normalized.matchAll(/\[null,"([^"]+)"\]/g))
    .map((entry) => entry[1])
    .filter((entry) => typeof entry === "string" && entry.trim());
  return matches.length ? matches[matches.length - 1] : null;
}

function selectBestTargetRpcPair(pairs, kind) {
  const candidates = Array.isArray(pairs)
    ? pairs.filter((entry) => entry?.kind === kind)
    : [];
  return (
    candidates.find(
      (entry) =>
        entry?.request &&
        entry?.response &&
        isSuccessfulTargetRpcStatus(entry.response.status),
    ) ??
    candidates.find((entry) => entry?.request && entry?.response) ??
    candidates[0] ??
    null
  );
}

function isSuccessfulTargetRpcStatus(status) {
  return Number.isInteger(status) && status >= 200 && status < 300;
}

function isReplayReadyTargetRpcContract(contract) {
  return Boolean(
    contract?.capturedTargetRpcContract &&
      normalizeString(contract?.appId) &&
      normalizeString(contract?.generationId) &&
      normalizeString(contract?.codeAssistantOpaqueToken) &&
      normalizeString(contract?.codeAssistantOffline?.url) &&
      normalizeString(contract?.streamCodeAssistantOfflineGeneration?.url) &&
      isSuccessfulTargetRpcStatus(contract?.codeAssistantOffline?.responseStatus) &&
      isSuccessfulTargetRpcStatus(
        contract?.streamCodeAssistantOfflineGeneration?.responseStatus,
      ),
  );
}

function buildNormalizedTargetRpcContract(targetRpcSummary) {
  const codeAssistantPair = selectBestTargetRpcPair(
    targetRpcSummary?.pairs,
    "code_assistant_offline",
  );
  const streamPair = selectBestTargetRpcPair(
    targetRpcSummary?.pairs,
    "stream_code_assistant_offline_generation",
  );

  const codeAssistantRequestPreview = codeAssistantPair?.request?.postDataPreview ?? null;
  const codeAssistantResponsePreview = codeAssistantPair?.response?.bodyPreview ?? null;
  const streamRequestPreview = streamPair?.request?.postDataPreview ?? null;
  const streamResponsePreview = streamPair?.response?.bodyPreview ?? null;

  const codeAssistantAppId = extractCodeAssistantAppId(codeAssistantRequestPreview);
  const streamAppId = extractStreamCodeAssistantAppId(streamRequestPreview);
  const appId = selectConsistentAppId({
    codeAssistantAppId,
    streamAppId,
    hasCodeAssistantAppIdConflict:
      hasConflictingCodeAssistantAppIdSlots(codeAssistantRequestPreview),
  });
  const modelPath = extractCodeAssistantModelPath(codeAssistantRequestPreview);
  const promptText = extractFirstPromptCandidate(codeAssistantRequestPreview);
  const generationId = selectConsistentGenerationId({
    codeAssistantGenerationId: extractGenerationIdFromCodeAssistantResponse(
      codeAssistantResponsePreview,
    ),
    streamGenerationId: extractStreamGenerationId(streamRequestPreview),
  });
  const codeAssistantOpaqueToken = extractCodeAssistantOpaqueToken(
    codeAssistantRequestPreview,
  );
  const finalText = extractFinalTextFromStreamBodyPreview(streamResponsePreview);

  const contract = {
    capturedTargetRpcContract: Boolean(
      codeAssistantPair?.request &&
        codeAssistantPair?.response &&
        streamPair?.request &&
        streamPair?.response,
    ),
    appId,
    modelPath,
    promptText,
    generationId,
    codeAssistantOpaqueToken,
    finalText,
    codeAssistantOffline: {
      url: codeAssistantPair?.request?.url ?? codeAssistantPair?.response?.url ?? null,
      requestHeaders: stableHeaderSubset(codeAssistantPair?.request?.headers),
      requestBodyPreview: codeAssistantRequestPreview,
      responseStatus: codeAssistantPair?.response?.status ?? null,
      responseHeaders: stableHeaderSubset(codeAssistantPair?.response?.headers),
      responseBodyPreview: codeAssistantResponsePreview,
    },
    streamCodeAssistantOfflineGeneration: {
      url: streamPair?.request?.url ?? streamPair?.response?.url ?? null,
      requestHeaders: stableHeaderSubset(streamPair?.request?.headers),
      requestBodyPreview: streamRequestPreview,
      responseStatus: streamPair?.response?.status ?? null,
      responseHeaders: stableHeaderSubset(streamPair?.response?.headers),
      responseBodyPreview: streamResponsePreview,
    },
  };
  contract.replayReadyTargetRpcContract =
    isReplayReadyTargetRpcContract(contract);
  return contract;
}

function extractAistudioUiSignals(snapshot) {
  const bodyText = String(snapshot?.bodyText ?? "");
  const buttonText = (Array.isArray(snapshot?.buttons) ? snapshot.buttons : [])
    .flatMap((entry) => [entry?.text, entry?.ariaLabel, entry?.title])
    .filter((entry) => typeof entry === "string")
    .join("\n");
  const textboxText = (Array.isArray(snapshot?.textboxes) ? snapshot.textboxes : [])
    .flatMap((entry) => [entry?.placeholder, entry?.ariaLabel, entry?.role, entry?.tag])
    .filter((entry) => typeof entry === "string")
    .join("\n");
  const combinedText = `${bodyText}\n${buttonText}\n${textboxText}`;
  const modelLabels = Array.from(
    new Set(
      combinedText
        .split(/\r?\n/)
        .map((line) => line.match(/\bGemini\s+\d+(?:\.\d+)?(?:\s+[A-Za-z0-9.-]+){1,3}\b/)?.[0])
        .filter(Boolean),
    ),
  ).slice(0, 12);

  return {
    hasInternalError:
      /An internal error occurred|internal error|内部错误|发生内部错误/i.test(combinedText),
    hasCanceledStatus: /\bCanceled\b|已取消|已取消请求/i.test(combinedText),
    hasRetryAction: /\bRetry\b|重试/.test(combinedText),
    hasCookieBanner: /uses cookies|OK,\s*got it|Cookie|cookies from Google/i.test(combinedText),
    hasBuildPromptTextbox:
      /Enter a prompt to generate an app|Describe an app|Make changes/i.test(textboxText),
    hasBudgetControlPrompt:
      /控制\s*API\s*费用|create budget limit|budget limit|API cost|API 费用/i.test(
        combinedText,
      ),
    hasProBadge: /(^|\n)\s*PRO\s*(\n|$)/.test(bodyText),
    accountEmails: Array.from(
      new Set(combinedText.match(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi) ?? []),
    ).slice(0, 8),
    modelLabels,
  };
}

function classifyAistudioAuthRecovery(capture) {
  const finalUrl = normalizeString(capture?.finalUrl);
  if (!finalUrl) {
    return {
      isAuthRecovery: false,
      kind: null,
      finalUrlHost: null,
      accountEmails: [],
    };
  }

  let parsed = null;
  try {
    parsed = new URL(finalUrl);
  } catch (_) {
    return {
      isAuthRecovery: false,
      kind: null,
      finalUrlHost: null,
      accountEmails: [],
    };
  }

  const host = parsed.hostname.toLowerCase();
  const isGoogleAccountsHost =
    host === "accounts.google.com" || host.endsWith(".accounts.google.com");
  if (!isGoogleAccountsHost) {
    return {
      isAuthRecovery: false,
      kind: null,
      finalUrlHost: host,
      accountEmails: [],
    };
  }

  const path = parsed.pathname.toLowerCase();
  let kind = "google_auth";
  if (path.includes("accountchooser")) {
    kind = "google_account_chooser";
  } else if (path.includes("signin")) {
    kind = "google_signin";
  } else if (path.includes("challenge")) {
    kind = "google_challenge";
  }

  const accountEmails = Array.isArray(capture?.finalPage?.uiSignals?.accountEmails)
    ? capture.finalPage.uiSignals.accountEmails
    : [];

  return {
    isAuthRecovery: true,
    kind,
    finalUrlHost: host,
    accountEmails,
  };
}

function summarizeLocalProxyErrors(localConnections) {
  return localConnections
    .flatMap((connection) =>
      (Array.isArray(connection?.framesReceived)
        ? connection.framesReceived
        : []
      ).map((frame) => {
        if (typeof frame !== "string") {
          return null;
        }
        let parsed = null;
        try {
          parsed = JSON.parse(frame);
        } catch (_) {
          return null;
        }
        if (parsed?.event_type !== "error") {
          return null;
        }
        return {
          status: Number.isInteger(parsed.status) ? parsed.status : null,
          requestId: normalizeString(parsed.request_id),
          message: previewText(parsed.message ?? "local proxy error", 512),
        };
      }),
    )
    .filter(Boolean)
    .slice(0, 8);
}

function buildProbeSummary({
  capture,
  captureDir,
  executablePath,
  runtimeState,
  appUrl,
  failUnlessTargetRpcCaptured,
  targetRpcSummary,
  normalizedTargetRpcContract,
  targetRpcContractObjectKey,
  targetRpcContractMirrorPath,
  localProxyRequest,
}) {
  const requests = Array.isArray(capture?.requests) ? capture.requests : [];
  const responses = Array.isArray(capture?.responses) ? capture.responses : [];
  const websockets = Array.isArray(capture?.websockets) ? capture.websockets : [];
  const localConnections = Array.isArray(capture?.localWebSocket?.connections)
    ? capture.localWebSocket.connections
    : [];
  const hasCapturedTraffic =
    requests.length > 0 || websockets.length > 0 || localConnections.length > 0;
  const normalizedContract =
    normalizedTargetRpcContract ??
    buildNormalizedTargetRpcContract(targetRpcSummary);
  const replayReadyTargetRpcContract = isReplayReadyTargetRpcContract(
    normalizedContract,
  );
  const targetRpcResponseStatuses = {
    codeAssistantOffline:
      normalizedContract?.codeAssistantOffline?.responseStatus ?? null,
    streamCodeAssistantOfflineGeneration:
      normalizedContract?.streamCodeAssistantOfflineGeneration?.responseStatus ?? null,
  };
  const targetRpcFailure =
    [
      {
        kind: "codeAssistantOffline",
        status: targetRpcResponseStatuses.codeAssistantOffline,
        bodyPreview:
          normalizedContract?.codeAssistantOffline?.responseBodyPreview ?? null,
      },
      {
        kind: "streamCodeAssistantOfflineGeneration",
        status: targetRpcResponseStatuses.streamCodeAssistantOfflineGeneration,
        bodyPreview:
          normalizedContract?.streamCodeAssistantOfflineGeneration
            ?.responseBodyPreview ?? null,
      },
    ].find(
      (entry) =>
        Number.isInteger(entry.status) && !isSuccessfulTargetRpcStatus(entry.status),
    ) ?? null;
  if (targetRpcFailure?.bodyPreview) {
    targetRpcFailure.bodyPreview = previewText(targetRpcFailure.bodyPreview, 512);
  }
  const authRecoveryState = classifyAistudioAuthRecovery(capture);
  const authRecoveryBlocksOk =
    authRecoveryState.isAuthRecovery &&
    !targetRpcSummary.capturedTargetRpcContract;
  const localProxyErrors = summarizeLocalProxyErrors(localConnections);

  return {
    ok: failUnlessTargetRpcCaptured
      ? replayReadyTargetRpcContract
      : hasCapturedTraffic && !authRecoveryBlocksOk,
    captureDir,
    executablePath,
    runtimeStateMode: runtimeState.mode,
    runtimeStatePath: runtimeState.absolutePath,
    appUrl,
    browserProxyMode: capture?.browserProxyMode ?? null,
    browserProxyServer: capture?.browserProxyServer ?? null,
    browserProxyPreflight: capture?.browserProxyPreflight ?? null,
    finalUrl: capture?.finalUrl ?? null,
    matchedRequestCount: requests.length,
    matchedResponseCount: responses.length,
    matchedWebSocketCount: websockets.length,
    matchedLocalWebSocketConnections: localConnections.length,
    capturedTargetRpcContract: targetRpcSummary.capturedTargetRpcContract,
    replayReadyTargetRpcContract,
    matchedCodeAssistantOfflineCount:
      targetRpcSummary.codeAssistantOfflineCount,
    matchedStreamCodeAssistantOfflineGenerationCount:
      targetRpcSummary.streamCodeAssistantOfflineGenerationCount,
    targetRpcModelPath: normalizedContract?.modelPath ?? null,
    targetRpcResponseStatuses,
    targetRpcFailure,
    authRecoveryState,
    pageUiSignals: capture?.finalPage?.uiSignals ?? null,
    targetRpcSummaryPath: captureDir
      ? path.join(captureDir, "target-rpc-summary.json")
      : null,
    normalizedTargetRpcContractPath: captureDir
      ? path.join(captureDir, "normalized-target-rpc-contract.json")
      : null,
    targetRpcContractObjectKey,
    targetRpcContractMirrorPath,
    localProxyRequestEnabled: Boolean(localProxyRequest),
    localProxySentEventTypes:
      localConnections.flatMap((entry) => entry.sentEventTypes ?? []),
    localProxyReceivedEventTypes:
      localConnections.flatMap((entry) => entry.receivedEventTypes ?? []),
    localProxyErrors,
    firstMatchedUrl: requests[0]?.url ?? websockets[0]?.url ?? null,
    runAppRedirectLocation:
      responses.find((entry) =>
        typeof entry.headers?.location === "string" &&
        entry.headers.location.includes("run.app"),
      )?.headers?.location ?? null,
    note:
      replayReadyTargetRpcContract
        ? "Captured target AI Studio MakerSuite RPC contract. Inspect target-rpc-summary.json and numbered pair files."
        : authRecoveryBlocksOk
          ? "Reached Google auth recovery/account chooser before capturing target AI Studio MakerSuite RPCs. Refresh storage-state or select the account in a visible browser run."
        : targetRpcSummary.capturedTargetRpcContract
          ? "Captured target AI Studio MakerSuite RPC pairs, but normalized contract is missing replay-critical appId / opaque token / URL material or 2xx response status."
        : requests.length > 0 || websockets.length > 0
          ? "Captured AI Studio traffic, but not the target CodeAssistantOffline / StreamCodeAssistantOfflineGeneration contract yet."
          : "No matching AI Studio traffic was captured before timeout. If login or prompt submission was required, rerun the probe and perform the action in the visible browser window.",
  };
}

function normalizeHeadersObject(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(value)
      .map(([key, entry]) => [String(key), typeof entry === "string" ? entry : String(entry ?? "")])
      .filter(([key, entry]) => key.trim() && entry.trim()),
  );
}

function createWebSocketTextFrame(text) {
  const payload = Buffer.from(String(text), "utf8");
  if (payload.length < 126) {
    return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
  }
  if (payload.length < 65536) {
    const header = Buffer.alloc(4);
    header[0] = 0x81;
    header[1] = 126;
    header.writeUInt16BE(payload.length, 2);
    return Buffer.concat([header, payload]);
  }
  const header = Buffer.alloc(10);
  header[0] = 0x81;
  header[1] = 127;
  header.writeBigUInt64BE(BigInt(payload.length), 2);
  return Buffer.concat([header, payload]);
}

function parseWebSocketFrames(buffer) {
  const frames = [];
  let offset = 0;
  while (offset + 2 <= buffer.length) {
    const first = buffer[offset];
    const second = buffer[offset + 1];
    const opcode = first & 0x0f;
    const masked = Boolean(second & 0x80);
    let payloadLength = second & 0x7f;
    let headerLength = 2;

    if (payloadLength === 126) {
      if (offset + 4 > buffer.length) break;
      payloadLength = buffer.readUInt16BE(offset + 2);
      headerLength = 4;
    } else if (payloadLength === 127) {
      if (offset + 10 > buffer.length) break;
      const bigLength = buffer.readBigUInt64BE(offset + 2);
      if (bigLength > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error("WebSocket frame too large to capture safely.");
      }
      payloadLength = Number(bigLength);
      headerLength = 10;
    }

    const maskLength = masked ? 4 : 0;
    const totalLength = headerLength + maskLength + payloadLength;
    if (offset + totalLength > buffer.length) break;

    let payload = buffer.slice(offset + headerLength + maskLength, offset + totalLength);
    if (masked) {
      const mask = buffer.slice(offset + headerLength, offset + headerLength + 4);
      const unmasked = Buffer.alloc(payload.length);
      for (let i = 0; i < payload.length; i += 1) {
        unmasked[i] = payload[i] ^ mask[i % 4];
      }
      payload = unmasked;
    }

    frames.push({ opcode, payload, totalLength });
    offset += totalLength;
  }

  return {
    frames,
    rest: buffer.slice(offset),
  };
}

async function createLocalWebSocketCaptureServer(port, capture, persistCapture) {
  if (!Number.isFinite(Number(port)) || Number(port) <= 0) {
    return null;
  }

  capture.localWebSocket = {
    host: "127.0.0.1",
    port: Number(port),
    connections: [],
  };
  const liveSockets = new Set();

  const server = net.createServer((socket) => {
    const connection = {
      openedAt: new Date().toISOString(),
      remoteAddress: socket.remoteAddress,
      remotePort: socket.remotePort,
      handshakeRequest: null,
      handshakeHeaders: null,
      framesReceived: [],
      framesSent: [],
      receivedEventTypes: [],
      sentEventTypes: [],
      dispatchKeys: [],
      errors: [],
      closedAt: null,
    };
    capture.localWebSocket.connections.push(connection);
    const liveEntry = { socket, connection };
    liveSockets.add(liveEntry);

    let handshakeDone = false;
    let pending = Buffer.alloc(0);

    socket.on("data", async (chunk) => {
      try {
        pending = Buffer.concat([pending, chunk]);
        if (!handshakeDone) {
          const marker = pending.indexOf("\r\n\r\n");
          if (marker === -1) {
            return;
          }
          const head = pending.slice(0, marker).toString("utf8");
          pending = pending.slice(marker + 4);
          const lines = head.split("\r\n");
          const [requestLine, ...headerLines] = lines;
          const headers = Object.fromEntries(
            headerLines
              .map((line) => {
                const idx = line.indexOf(":");
                if (idx === -1) return null;
                return [line.slice(0, idx).trim().toLowerCase(), line.slice(idx + 1).trim()];
              })
              .filter(Boolean),
          );
          connection.handshakeRequest = requestLine;
          connection.handshakeHeaders = headers;
          const wsKey = headers["sec-websocket-key"];
          if (!wsKey) {
            throw new Error("Missing Sec-WebSocket-Key in local probe handshake.");
          }
          const accept = crypto
            .createHash("sha1")
            .update(`${wsKey}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`, "utf8")
            .digest("base64");
          const response =
            "HTTP/1.1 101 Switching Protocols\r\n" +
            "Upgrade: websocket\r\n" +
            "Connection: Upgrade\r\n" +
            `Sec-WebSocket-Accept: ${accept}\r\n` +
            "\r\n";
          socket.write(response);
          handshakeDone = true;
          connection.framesSent.push("[handshake-101]");
          await persistCapture();
        }

        if (handshakeDone && pending.length > 0) {
          const parsed = parseWebSocketFrames(pending);
          pending = parsed.rest;
          for (const frame of parsed.frames) {
            if (frame.opcode === 0x1) {
              const text = frame.payload.toString("utf8");
              const parsedFrame = tryParseJson(text);
              if (parsedFrame?.event_type) {
                connection.receivedEventTypes.push(parsedFrame.event_type);
              }
              connection.framesReceived.push(previewText(text, 12000));
            } else if (frame.opcode === 0x2) {
              connection.framesReceived.push(`[binary ${frame.payload.length} bytes]`);
            } else if (frame.opcode === 0x8) {
              connection.framesReceived.push("[close]");
            } else if (frame.opcode === 0x9) {
              connection.framesReceived.push("[ping]");
              socket.write(Buffer.from([0x8a, 0x00]));
              connection.framesSent.push("[pong]");
            } else {
              connection.framesReceived.push(
                `[opcode ${frame.opcode}] ${previewText(frame.payload.toString("utf8"), 2048)}`,
              );
            }
          }
          await persistCapture();
        }
      } catch (error) {
        connection.errors.push(String(error));
        await persistCapture();
      }
    });

    socket.on("error", async (error) => {
      connection.errors.push(String(error));
      await persistCapture();
    });

    socket.on("close", async () => {
      liveSockets.delete(liveEntry);
      connection.closedAt = new Date().toISOString();
      await persistCapture();
    });
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(Number(port), "127.0.0.1", () => {
      server.off("error", reject);
      resolve();
    });
  });

  return {
    sendMessages: async (messages, options = {}) => {
      const normalized = Array.isArray(messages) ? messages : [];
      const dispatchKey = normalizeString(options?.dispatchKey);
      const onlyUndispatched = options?.onlyUndispatched === true;
      let sentConnectionCount = 0;
      for (const entry of liveSockets) {
        if (entry.socket.destroyed) {
          continue;
        }
        if (
          onlyUndispatched &&
          dispatchKey &&
          Array.isArray(entry.connection.dispatchKeys) &&
          entry.connection.dispatchKeys.includes(dispatchKey)
        ) {
          continue;
        }
        let sentToThisConnection = false;
        for (const message of normalized) {
          const text = typeof message === "string" ? message : JSON.stringify(message ?? null);
          const parsedMessage = tryParseJson(text);
          if (parsedMessage?.event_type) {
            entry.connection.sentEventTypes.push(parsedMessage.event_type);
          }
          entry.connection.framesSent.push(previewText(text, 12000));
          entry.socket.write(createWebSocketTextFrame(text));
          sentToThisConnection = true;
        }
        if (sentToThisConnection) {
          sentConnectionCount += 1;
          if (dispatchKey) {
            entry.connection.dispatchKeys.push(dispatchKey);
          }
        }
      }
      await persistCapture();
      return {
        sentConnectionCount,
      };
    },
    close: async () =>
      new Promise((resolve, reject) => {
        for (const entry of liveSockets) {
          entry.socket.destroy();
        }
        server.close((error) => (error ? reject(error) : resolve()));
      }),
  };
}

async function collectPageSnapshot(page, label) {
  const snapshot = await page.evaluate((phase) => {
    const bodyText = document.body?.innerText ?? "";
    const buttons = Array.from(
      document.querySelectorAll("button,[role=\"button\"],a[role=\"button\"]"),
    )
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
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
      .slice(0, 60);
    const iframes = Array.from(document.querySelectorAll("iframe"))
      .map((node, index) => ({
        index,
        src: node.getAttribute("src"),
        title: node.getAttribute("title"),
        ariaLabel: node.getAttribute("aria-label"),
      }))
      .slice(0, 40);
    const hookEvents = Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)
      ? window.__AISTUDIO_LIVE_CAPTURE__.slice(-60)
      : [];
    return {
      label: phase,
      url: location.href,
      title: document.title,
      bodyText: bodyText.slice(0, 12000),
      buttons,
      textboxes,
      iframes,
      hookEvents,
    };
  }, label);
  return snapshot;
}

async function collectFrameDiagnostics(page, label) {
  const frames = [];
  for (const frame of page.frames()) {
    try {
      const data = await frame.evaluate((phase) => {
        const hookEvents = Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)
          ? window.__AISTUDIO_LIVE_CAPTURE__.slice(-120)
          : [];
        return {
          label: phase,
          url: location.href,
          title: document.title,
          name: window.name || null,
          hookEvents,
        };
      }, label);
      frames.push(data);
    } catch (error) {
      frames.push({
        label,
        url: frame.url(),
        name: frame.name() || null,
        error: String(error),
      });
    }
  }
  return frames;
}

function countOpenLocalWebSocketConnections(capture) {
  return (
    capture.localWebSocket?.connections?.filter((entry) => !entry?.closedAt).length ?? 0
  );
}

function countOpenUndispatchedLocalProxyConnections(capture) {
  return (
    capture.localWebSocket?.connections?.filter(
      (entry) =>
        !entry?.closedAt &&
        !(Array.isArray(entry?.sentEventTypes) && entry.sentEventTypes.includes("proxy_request")),
    ).length ?? 0
  );
}

function hasPendingLocalProxyResponse(capture) {
  return (
    capture.localWebSocket?.connections?.some(
      (entry) =>
        Array.isArray(entry?.sentEventTypes) &&
        entry.sentEventTypes.includes("proxy_request") &&
        !entry?.closedAt &&
        (!Array.isArray(entry?.receivedEventTypes) || entry.receivedEventTypes.length <= 0),
    ) ?? false
  );
}

async function waitForLocalWebSocketConnection(capture, timeoutMs = 15_000, options = {}) {
  const requireUndispatched = options?.requireUndispatched === true;
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const connectionCount = requireUndispatched
      ? countOpenUndispatchedLocalProxyConnections(capture)
      : countOpenLocalWebSocketConnections(capture);
    if (connectionCount > 0) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  return requireUndispatched
    ? countOpenUndispatchedLocalProxyConnections(capture) > 0
    : countOpenLocalWebSocketConnections(capture) > 0;
}

async function waitForRunAppBootstrap(page, timeoutMs = 15_000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    for (const frame of page.frames()) {
      try {
        const hasBootstrap = await frame.evaluate(() => {
          if (!Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)) {
            return false;
          }
          return window.__AISTUDIO_LIVE_CAPTURE__.some(
            (event) =>
              event?.kind === "window.message" &&
              typeof event.messagePreview === "string" &&
              event.messagePreview.includes("\"type\":\"bootstrap\""),
          );
        });
        if (hasBootstrap) {
          return {
            ok: true,
            frameUrl: frame.url(),
          };
        }
      } catch (_) {}
    }
    await page.waitForTimeout(250);
  }
  return { ok: false, frameUrl: null };
}

async function maybeDispatchLocalProxyMessages(
  page,
  capture,
  localWebSocketServer,
  localProxyMessages,
  localProxyDelayMs,
  stageLabel,
  persistCapture,
) {
  if (!localWebSocketServer || localProxyMessages.length === 0) {
    return false;
  }

  if (countOpenUndispatchedLocalProxyConnections(capture) <= 0) {
    return false;
  }

  const hasLocalConnection = await waitForLocalWebSocketConnection(capture, 20_000, {
    requireUndispatched: true,
  });
  capture.autoActions.push({
    action: `wait-local-ws-connection-${stageLabel}`,
    ok: hasLocalConnection,
    connectionCount: countOpenLocalWebSocketConnections(capture),
    undispatchedConnectionCount: countOpenUndispatchedLocalProxyConnections(capture),
  });
  const bootstrapResult = await waitForRunAppBootstrap(page, 20_000);
  capture.autoActions.push({
    action: `wait-runapp-bootstrap-${stageLabel}`,
    ok: bootstrapResult.ok,
    frameUrl: bootstrapResult.frameUrl,
  });
  if (!hasLocalConnection) {
    return false;
  }

  await page.waitForTimeout(localProxyDelayMs);
  const dispatchResult = await localWebSocketServer.sendMessages(localProxyMessages, {
    dispatchKey: "local-proxy-request",
    onlyUndispatched: true,
  });
  if ((dispatchResult?.sentConnectionCount ?? 0) <= 0) {
    return false;
  }
  capture.autoActions.push({
    action: `send-local-proxy-request-${stageLabel}`,
    ok: true,
    sentConnectionCount: dispatchResult.sentConnectionCount,
    sentEventTypes: localProxyMessages.map((entry) => entry?.event_type).filter(Boolean),
  });
  await persistCapture();
  return true;
}

async function probeRunAppFrameFetch(page, requestSpec) {
  const runAppFrame = page.frames().find((frame) => frame.url().includes("run.app"));
  if (!runAppFrame) {
    return {
      ok: false,
      error: "run_app_frame_not_found",
    };
  }

  try {
    return await runAppFrame.evaluate(async (spec) => {
      const fetchPromise = (async () => {
        try {
          const response = await fetch(spec.url, {
            method: spec.method || "GET",
            headers: spec.headers || {},
            body: typeof spec.body === "string" ? spec.body : undefined,
            credentials: spec.credentials || "omit",
          });
          const text = await response.text().catch(() => "");
          return {
            ok: response.ok,
            status: response.status,
            finalUrl: response.url,
            contentType: response.headers.get("content-type"),
            bodyPreview: text.slice(0, 2000),
          };
        } catch (error) {
          return {
            ok: false,
            error: String(error),
          };
        }
      })();

      const timeoutPromise = new Promise((resolve) => {
        setTimeout(
          () =>
            resolve({
              ok: false,
              error: "run_app_probe_timeout",
            }),
          12_000,
        );
      });

      return await Promise.race([fetchPromise, timeoutPromise]);
    }, requestSpec);
  } catch (error) {
    return {
      ok: false,
      error: String(error),
    };
  }
}

async function bestEffortContinueIntoApp(page, capture) {
  const clickIfVisible = async (locator, label) => {
    try {
      const candidate = locator.first();
      if (await candidate.isVisible({ timeout: 1200 }).catch(() => false)) {
        await candidate.click({ timeout: 5000, force: true });
        capture.autoActions.push({ action: label, ok: true });
        await page.waitForTimeout(1200);
        return true;
      }
    } catch (error) {
      capture.autoActions.push({ action: label, ok: false, error: String(error) });
    }
    return false;
  };

  try {
    const termsCheckbox = page.locator(
      'input[aria-label*="Google API 服务条款"], input[aria-label*="Gemini API"], input[aria-label*="I agree"]',
    );
    const checkbox = termsCheckbox.first();
    if (await checkbox.isVisible({ timeout: 1000 }).catch(() => false)) {
      await checkbox.check({ force: true });
      capture.autoActions.push({ action: "accept-terms-checkbox", ok: true });
      await page.waitForTimeout(400);
    }
  } catch (error) {
    capture.autoActions.push({
      action: "accept-terms-checkbox",
      ok: false,
      error: String(error),
    });
  }

  await clickIfVisible(page.getByRole("button", { name: /Continue to the app/i }), "continue-to-app");
  await clickIfVisible(page.getByRole("button", { name: /^继续$/ }), "continue-cn");
  await clickIfVisible(page.locator("text=Continue to the app"), "continue-to-app-text");
  await clickIfVisible(page.locator("text=继续"), "continue-cn-text");
  await clickIfVisible(page.getByRole("button", { name: /Dismiss/i }), "dismiss-update-banner");
  await clickIfVisible(page.locator(".cdk-overlay-backdrop"), "click-overlay-backdrop");
}

async function bestEffortApplyRemixModal(page, capture) {
  try {
    const remixHeader = page.getByText(/^Remix\b/).first();
    if (!(await remixHeader.isVisible({ timeout: 1200 }).catch(() => false))) {
      return false;
    }
    const applyButton = page.getByRole("button", { name: /^Apply$/i }).first();
    if (!(await applyButton.isVisible({ timeout: 1200 }).catch(() => false))) {
      return false;
    }
    await applyButton.click({ timeout: 5000, force: true });
    capture.autoActions.push({ action: "apply-remix-modal", ok: true });
    await page.waitForTimeout(2000);
    return true;
  } catch (error) {
    capture.autoActions.push({
      action: "apply-remix-modal",
      ok: false,
      error: String(error),
    });
    return false;
  }
}

async function bestEffortDismissAistudioOverlays(page, capture) {
  const clickIfVisible = async (locator, label) => {
    try {
      const candidate = locator.first();
      if (!(await candidate.isVisible({ timeout: 900 }).catch(() => false))) {
        return false;
      }
      await candidate.click({ timeout: 3000, force: true });
      capture.autoActions.push({ action: label, ok: true });
      await page.waitForTimeout(500);
      return true;
    } catch (error) {
      capture.autoActions.push({ action: label, ok: false, error: String(error) });
      return false;
    }
  };

  let dismissed = false;
  await page.keyboard.press("Escape").catch(() => undefined);
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(OK,\s*got it|Got it|Dismiss)$/i }),
      "dismiss-cookie-ok",
    )) || dismissed;
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(Skip|跳过|暂不|Not now)$/i }),
      "dismiss-onboarding-skip",
    )) || dismissed;
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(Continue to the app|Continue|继续)$/i }),
      "dismiss-continue",
    )) || dismissed;
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(Close|关闭)$/i }),
      "dismiss-close",
    )) || dismissed;
  return dismissed;
}

async function bestEffortSelectGoogleAccount(page, capture, accountEmail) {
  const email = normalizeString(accountEmail);
  if (!email) {
    return false;
  }

  let currentUrl = "";
  try {
    currentUrl = String(page.url?.() ?? "");
  } catch (_) {
    currentUrl = "";
  }
  try {
    const parsed = new URL(currentUrl);
    const host = parsed.hostname.toLowerCase();
    if (host !== "accounts.google.com" && !host.endsWith(".accounts.google.com")) {
      return false;
    }
  } catch (_) {
    return false;
  }

  try {
    const accountRow = page.getByText(email, { exact: true }).first();
    if (!(await accountRow.isVisible({ timeout: 1500 }).catch(() => false))) {
      capture.autoActions.push({
        action: "select-google-account",
        ok: false,
        email,
        reason: "account-row-not-visible",
      });
      return false;
    }
    await accountRow.click({ timeout: 5000, force: true });
    capture.autoActions.push({
      action: "select-google-account",
      ok: true,
      email,
    });
    await page.waitForTimeout(3000);
    return true;
  } catch (error) {
    capture.autoActions.push({
      action: "select-google-account",
      ok: false,
      email,
      error: String(error),
    });
    return false;
  }
}

async function bestEffortAutoPrompt(page, promptText, capture, options = {}) {
  const normalized = normalizeString(promptText);
  if (!normalized) {
    return false;
  }

  const maxAttempts = Math.max(1, Number(options.maxAttempts) || 4);
  const pollMs = Math.max(0, Number(options.pollMs) || 1000);

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const candidates = [
      page.locator('textarea[aria-label="Enter a prompt to generate an app"]').first(),
      page.locator('textarea[placeholder*="Describe an app"]').first(),
      page.locator('textarea[placeholder*="Make changes"]').first(),
      page.locator("textarea").first(),
    ];

    for (const locator of candidates) {
      try {
        if (!(await locator.isVisible({ timeout: 1200 }).catch(() => false))) {
          continue;
        }
        await page.keyboard.press("Escape").catch(() => undefined);
        await page.waitForTimeout(200);
        await locator.click({ timeout: 3000, force: true });
        await locator.fill(normalized, { timeout: 5000, force: true });
        await page.waitForTimeout(300);
        let submission = "ctrl-enter";
        const buildByRole = page.getByRole("button", { name: /^Build/i }).first();
        const buildByText = page.locator("button:has-text('Build')").first();
        if (await buildByRole.isVisible({ timeout: 1200 }).catch(() => false)) {
          await buildByRole.click({ timeout: 5000, force: true });
          submission = "build-button-role";
        } else if (await buildByText.isVisible({ timeout: 1200 }).catch(() => false)) {
          await buildByText.click({ timeout: 5000, force: true });
          submission = "build-button-text";
        } else {
          await page.keyboard.press(
            process.platform === "darwin" ? "Meta+Enter" : "Control+Enter",
          );
        }
        capture.autoActions.push({
          action: "auto-prompt",
          ok: true,
          promptPreview: previewText(normalized, 256),
          submission,
          attempts: attempt,
        });
        return true;
      } catch (error) {
        capture.autoActions.push({
          action: "auto-prompt",
          ok: false,
          attempts: attempt,
          error: String(error),
        });
      }
    }

    if (attempt < maxAttempts && pollMs > 0) {
      await page.waitForTimeout(pollMs);
    }
  }

  capture.autoActions.push({
    action: "auto-prompt",
    ok: false,
    reason: "prompt-textarea-not-visible",
    attempts: maxAttempts,
  });
  return false;
}

function shouldRetryAutoPromptDuringPolling({
  normalizedAutoPrompt,
  autoPromptSubmitted,
  nowMs,
  lastAutoPromptAttemptAtMs,
  retryIntervalMs = 5_000,
}) {
  return (
    Boolean(normalizeString(normalizedAutoPrompt)) &&
    !autoPromptSubmitted &&
    Number(nowMs) - Number(lastAutoPromptAttemptAtMs) >=
      Math.max(0, Number(retryIntervalMs) || 0)
  );
}

async function bestEffortLaunchOwnedApp(page, capture) {
  const clickIfVisible = async (locator, label) => {
    try {
      const candidate = locator.first();
      if (await candidate.isVisible({ timeout: 1200 }).catch(() => false)) {
        await candidate.click({ timeout: 5000, force: true });
        capture.autoActions.push({ action: label, ok: true });
        await page.waitForTimeout(3000);
        return true;
      }
    } catch (error) {
      capture.autoActions.push({ action: label, ok: false, error: String(error) });
    }
    return false;
  };

  if (
    await clickIfVisible(page.getByRole("button", { name: /^Launch/i }), "launch-owned-app-role")
  ) {
    return true;
  }
  if (await clickIfVisible(page.locator("button:has-text('Launch')"), "launch-owned-app-text")) {
    return true;
  }
  return false;
}

async function main() {
  let browser = null;
  let context = null;
  let page = null;
  let localWebSocketServer = null;
  let captureDir = null;
  let capture = null;
  try {
    const raw = await readStdin();
    const input = raw ? JSON.parse(raw) : {};
    if (normalizeString(input.captureDir)) {
      captureDir = ensureCaptureDir(input.captureDir);
    }
    validateInput(input);

    const appUrl = normalizeString(input.appUrl) ?? DEFAULT_APP_URL;
    const timeoutMs = Math.max(Number(input.timeoutMs || DEFAULT_TIMEOUT_MS), 30_000);
    const settleMs = Math.max(Number(input.settleMs || DEFAULT_SETTLE_MS), 2_000);
    const responseBodyLimit = Math.max(
      Number(input.responseBodyLimit || DEFAULT_RESPONSE_BODY_LIMIT),
      1024,
    );
    const authIndex = Number.isFinite(Number(input.authIndex)) ? Number(input.authIndex) : 0;
    const localWsPort = Number.isFinite(Number(input.localWsPort))
      ? Number(input.localWsPort)
      : null;
    const locale = normalizeString(input.locale) ?? DEFAULT_LOCALE;
    const autoSelectGoogleAccountEmail =
      normalizeString(input.autoSelectGoogleAccountEmail) ??
      normalizeString(input.auto_select_google_account_email) ??
      normalizeString(process.env.AISTUDIO_PROBE_AUTO_SELECT_GOOGLE_ACCOUNT_EMAIL);
    const autoPromptMaxAttempts = Number.isFinite(Number(input.autoPromptMaxAttempts))
      ? Math.max(1, Number(input.autoPromptMaxAttempts))
      : Number.isFinite(Number(input.auto_prompt_max_attempts))
        ? Math.max(1, Number(input.auto_prompt_max_attempts))
        : 4;
    const autoPromptPollMs = Number.isFinite(Number(input.autoPromptPollMs))
      ? Math.max(0, Number(input.autoPromptPollMs))
      : Number.isFinite(Number(input.auto_prompt_poll_ms))
        ? Math.max(0, Number(input.auto_prompt_poll_ms))
        : 1000;
    captureDir = captureDir ?? ensureCaptureDir(input.captureDir);
    const requestUrlIncludes = normalizeStringArray(
      input.requestUrlIncludes,
      DEFAULT_URL_PATTERNS,
    );
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ??
        process.env.AISTUDIO_BROWSER_EXECUTABLE_PATH ??
        null,
    );
    if (!executablePath) {
      throw new Error(
        "Unable to locate a Chromium-compatible browser. Set AISTUDIO_BROWSER_EXECUTABLE_PATH.",
      );
    }
    const browserProxySettings = resolveBrowserProxySettings(input);
    const browserProxyLaunchOptions =
      buildBrowserProxyLaunchOptions(browserProxySettings);

    const runtimeState = await resolveRuntimeStateSource(input.runtimeStateObjectKey);
    const localProxyRequest = buildDefaultLocalProxyRequest(input);
    const localProxyLogLevel =
      normalizeString(input.localProxyLogLevel)?.toUpperCase() ?? "DEBUG";
    const localProxyDelayMs = Number.isFinite(Number(input.localProxyDelayMs))
      ? Math.max(Number(input.localProxyDelayMs), 0)
      : DEFAULT_LOCAL_PROXY_DELAY_MS;
    const localProxyMessages = localProxyRequest
      ? [
          { event_type: "set_log_level", level: localProxyLogLevel },
          localProxyRequest,
        ]
      : [];

    capture = {
      startedAt: new Date().toISOString(),
      runtimeStateObjectKey: input.runtimeStateObjectKey,
      runtimeStateMode: runtimeState.mode,
      runtimeStatePath: runtimeState.absolutePath,
      executablePath,
      appUrl,
      browserProxyMode: browserProxySettings?.mode ?? "system",
      browserProxyServer: browserProxySettings?.launchProxy?.server ?? null,
      browserProxyPreflight: null,
      autoSelectGoogleAccountEmail,
      autoPromptMaxAttempts,
      autoPromptPollMs,
      requestUrlIncludes,
      requests: [],
      responses: [],
      websockets: [],
      pageErrors: [],
      console: [],
      autoActions: [],
      localProxyRequest,
      localProxyLogLevel,
      localProxyDelayMs,
    };

    try {
      capture.browserProxyPreflight =
        await assertBrowserProxyReachable(browserProxySettings);
    } catch (error) {
      capture.browserProxyPreflight = {
        ok: false,
        code: error?.code ?? "aistudio_browser_proxy_preflight_failed",
        status: Number(error?.status ?? 503),
        message: error instanceof Error ? error.message : String(error),
        host: error?.host ?? null,
        port: error?.port ?? null,
        server: error?.server ?? capture.browserProxyServer,
        rawProxy: error?.rawProxy ?? browserProxySettings?.rawProxy ?? null,
      };
      throw error;
    }

    const persistCapture = async () => {
      await writeFile(
        path.join(captureDir, "capture.json"),
        `${JSON.stringify(capture, null, 2)}\n`,
        "utf8",
      );
    };

    localWebSocketServer = await createLocalWebSocketCaptureServer(
      localWsPort,
      capture,
      persistCapture,
      {
        initialMessages: localProxyMessages,
        initialDelayMs: localProxyDelayMs,
      },
    );

    const addHookScript = `
      (() => {
        if (Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)) {
          return;
        }
        const maxEvents = 200;
        const pushEvent = (event) => {
          try {
            const target = window.__AISTUDIO_LIVE_CAPTURE__;
            target.push({
              time: new Date().toISOString(),
              ...event,
            });
            if (target.length > maxEvents) {
              target.splice(0, target.length - maxEvents);
            }
          } catch (_) {}
        };
        window.__AISTUDIO_LIVE_CAPTURE__ = [];

        const originalPostMessage = window.postMessage?.bind(window);
        if (originalPostMessage) {
          window.postMessage = function(message, targetOrigin, transfer) {
            pushEvent({
              kind: "window.postMessage",
              targetOrigin: String(targetOrigin ?? "*"),
              messagePreview:
                typeof message === "string"
                  ? message.slice(0, 2048)
                  : JSON.stringify(message ?? null).slice(0, 2048),
            });
            return originalPostMessage(message, targetOrigin, transfer);
          };
        }

        window.addEventListener("message", (event) => {
          const data = event.data;
          if (data && typeof data === "object" && data.type === "requestAuthIndex") {
            try {
              event.source?.postMessage(
                {
                  type: "authIndexResponse",
                  authIndex: globalThis.__AISTUDIO_AUTH_INDEX__ ?? 0,
                },
                "*",
              );
              pushEvent({
                kind: "window.message.reply",
                replyType: "authIndexResponse",
                authIndex: globalThis.__AISTUDIO_AUTH_INDEX__ ?? 0,
              });
            } catch (error) {
              pushEvent({
                kind: "window.message.reply_error",
                error: String(error),
              });
            }
          }
          pushEvent({
            kind: "window.message",
            origin: String(event.origin ?? ""),
            messagePreview:
              typeof event.data === "string"
                ? event.data.slice(0, 2048)
                : JSON.stringify(event.data ?? null).slice(0, 2048),
          });
        });

        const originalFetch = window.fetch?.bind(window);
        if (originalFetch) {
          window.fetch = async (...args) => {
            const [resource, init] = args;
            const method = (init && init.method) || "GET";
            const url = typeof resource === "string" ? resource : resource?.url || String(resource);
            const body =
              typeof init?.body === "string"
                ? init.body.slice(0, 4096)
                : init?.body == null
                  ? null
                  : String(init.body).slice(0, 4096);
            pushEvent({ kind: "fetch.request", method, url, body });
            try {
              const response = await originalFetch(...args);
              const clone = response.clone();
              let bodyPreview = null;
              try {
                bodyPreview = (await clone.text()).slice(0, 4096);
              } catch (_) {}
              pushEvent({
                kind: "fetch.response",
                method,
                url: response.url || url,
                status: response.status,
                contentType: response.headers.get("content-type"),
                bodyPreview,
              });
              return response;
            } catch (error) {
              pushEvent({
                kind: "fetch.error",
                method,
                url,
                error: String(error),
              });
              throw error;
            }
          };
        }

        const OriginalXHR = window.XMLHttpRequest;
        if (OriginalXHR) {
          const open = OriginalXHR.prototype.open;
          const send = OriginalXHR.prototype.send;
          OriginalXHR.prototype.open = function(method, url, ...rest) {
            this.__codexMethod = method;
            this.__codexUrl = url;
            return open.call(this, method, url, ...rest);
          };
          OriginalXHR.prototype.send = function(body) {
            pushEvent({
              kind: "xhr.request",
              method: this.__codexMethod || "GET",
              url: this.__codexUrl || "",
              body:
                typeof body === "string"
                  ? body.slice(0, 4096)
                  : body == null
                    ? null
                    : String(body).slice(0, 4096),
            });
            this.addEventListener("loadend", () => {
              let responseText = null;
              try {
                responseText = String(this.responseText || "").slice(0, 4096);
              } catch (_) {}
              pushEvent({
                kind: "xhr.response",
                method: this.__codexMethod || "GET",
                url: this.__codexUrl || "",
                status: this.status,
                bodyPreview: responseText,
              });
            });
            return send.call(this, body);
          };
        }
      })();
    `;

    const nextCaptureRequestId = createCaptureRequestIdFactory();
    const makeRequestEntry = async (request) => {
      let headers = {};
      try {
        headers = await request.allHeaders();
      } catch (_) {}
      return {
        id: nextCaptureRequestId(),
        time: new Date().toISOString(),
        method: request.method(),
        url: request.url(),
        resourceType: request.resourceType(),
        headers,
        postDataPreview: previewText(request.postData(), 12_000),
      };
    };

    const responseRequestAttributor = createResponseRequestAttributor();
    let captureActive = false;
    let firstMatchAt = null;
    let lastMatchAt = null;

    const markMatch = () => {
      const now = Date.now();
      if (!firstMatchAt) {
        firstMatchAt = now;
      }
      lastMatchAt = now;
    };

    if (runtimeState.mode === "profile_dir") {
      context = await chromium.launchPersistentContext(runtimeState.absolutePath, {
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_PROBE_HEADLESS, false),
        ...(browserProxyLaunchOptions.proxy
          ? { proxy: browserProxyLaunchOptions.proxy }
          : {}),
        locale,
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
          ...browserProxyLaunchOptions.args,
        ],
      });
    } else {
      browser = await chromium.launch({
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_PROBE_HEADLESS, false),
        ...(browserProxyLaunchOptions.proxy
          ? { proxy: browserProxyLaunchOptions.proxy }
          : {}),
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
          ...browserProxyLaunchOptions.args,
        ],
      });
      const storageStateBuffer = await readFile(runtimeState.absolutePath);
      const storageStateJson = JSON.parse(stripUtf8Bom(storageStateBuffer.toString("utf8")));
      context = await browser.newContext({
        storageState: storageStateJson,
        locale,
      });
    }

    await context.addInitScript((index) => {
      globalThis.__AISTUDIO_AUTH_INDEX__ = index;
    }, authIndex);
    await context.addInitScript(addHookScript);
    page = context.pages()[0] ?? (await context.newPage());
    await page.bringToFront().catch(() => undefined);

    page.on("pageerror", (error) => {
      capture.pageErrors.push({
        time: new Date().toISOString(),
        message: String(error),
      });
    });

    page.on("console", async (message) => {
      try {
        capture.console.push({
          time: new Date().toISOString(),
          type: message.type(),
          text: message.text(),
        });
      } catch (_) {}
    });

    page.on("framenavigated", (frame) => {
      capture.console.push({
        time: new Date().toISOString(),
        type: "frame",
        text: `${frame.url()} <- ${frame.name() || "<unnamed>"}`,
      });
    });

    const handleCapturedRequest = async (request, source = "context") => {
      if (!captureActive) {
        return;
      }
      const url = request.url();
      if (!requestMatches(url, requestUrlIncludes)) {
        return;
      }
      markMatch();
      const entry = await makeRequestEntry(request);
      entry.source = source;
      entry.frameUrl = request.frame()?.url() ?? null;
      entry.targetRpcKind = detectAistudioTargetRpcKind(url);
      responseRequestAttributor.trackRequest(request, entry.id);
      capture.requests.push(entry);
      await persistCapture();
    };

    const handleCapturedResponse = async (response, source = "context") => {
      if (!captureActive) {
        return;
      }
      const url = response.url();
      if (!requestMatches(url, requestUrlIncludes)) {
        return;
      }
      markMatch();
      const headers = await response.allHeaders().catch(() => ({}));
      const contentType = headers["content-type"] || "";
      let bodyPreview = null;
      try {
        if (
          contentType.includes("json") ||
          contentType.startsWith("text/") ||
          contentType.includes("javascript") ||
          contentType.includes("xml")
        ) {
          bodyPreview = previewText(await response.text(), responseBodyLimit);
        }
      } catch (error) {
        bodyPreview = `[read-error] ${String(error)}`;
      }
      capture.responses.push({
        time: new Date().toISOString(),
        requestId: responseRequestAttributor.resolveResponse(response),
        url,
        source,
        frameUrl: response.request().frame()?.url() ?? null,
        status: response.status(),
        headers,
        bodyPreview,
        targetRpcKind: detectAistudioTargetRpcKind(url),
      });
      await persistCapture();
    };

    context.on("request", (request) => {
      void handleCapturedRequest(request, "context");
    });

    context.on("response", (response) => {
      void handleCapturedResponse(response, "context");
    });

    page.on("websocket", (ws) => {
      if (!captureActive) {
        return;
      }
      const entry = {
        time: new Date().toISOString(),
        url: ws.url(),
        framesSent: [],
        framesReceived: [],
        closed: false,
        errors: [],
      };
      capture.websockets.push(entry);
      markMatch();
      ws.on("framesent", (event) => {
        entry.framesSent.push(previewText(event.payload, 4096));
      });
      ws.on("framereceived", (event) => {
        entry.framesReceived.push(previewText(event.payload, 4096));
      });
      ws.on("close", () => {
        entry.closed = true;
      });
      ws.on("socketerror", (error) => {
        entry.errors.push(String(error));
      });
    });

    await page.goto(appUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await page.waitForTimeout(5000);
    await bestEffortSelectGoogleAccount(
      page,
      capture,
      autoSelectGoogleAccountEmail,
    );

    const initialSnapshot = await collectPageSnapshot(page, "initial");
    const initialFrames = await collectFrameDiagnostics(page, "initial");
    await writeFile(
      path.join(captureDir, "initial-page.json"),
      `${JSON.stringify(initialSnapshot, null, 2)}\n`,
      "utf8",
    );
    await writeFile(
      path.join(captureDir, "initial-frames.json"),
      `${JSON.stringify(initialFrames, null, 2)}\n`,
      "utf8",
    );
    await page.screenshot({
      path: path.join(captureDir, "initial-page.png"),
      fullPage: true,
    });
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortContinueIntoApp(page, capture);
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortApplyRemixModal(page, capture);
    await bestEffortLaunchOwnedApp(page, capture);
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortApplyRemixModal(page, capture);
    await page.waitForTimeout(2000);
    capture.captureArmedAt = new Date().toISOString();
    captureActive = true;
    await persistCapture();
    await sendActiveTrigger(page, capture);
    await persistCapture();
    await bestEffortDismissAistudioOverlays(page, capture);

    if (localWebSocketServer && localProxyMessages.length > 0) {
      await maybeDispatchLocalProxyMessages(
        page,
        capture,
        localWebSocketServer,
        localProxyMessages,
        localProxyDelayMs,
        "pre_prompt",
        persistCapture,
      );
    }

    capture.runAppDirectFetchProbe = await probeRunAppFrameFetch(page, {
      method: "GET",
      url: "https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger",
      headers: {},
    });
    capture.autoActions.push({
      action: "probe-runapp-direct-fetch",
      ok: Boolean(capture.runAppDirectFetchProbe?.ok),
      status: capture.runAppDirectFetchProbe?.status ?? null,
      error: capture.runAppDirectFetchProbe?.error ?? null,
    });
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortApplyRemixModal(page, capture);
    await persistCapture();

    let autoPromptSubmitted = false;
    const normalizedAutoPrompt = normalizeString(input.autoPrompt);
    if (normalizedAutoPrompt) {
      await bestEffortDismissAistudioOverlays(page, capture);
      autoPromptSubmitted = await bestEffortAutoPrompt(page, normalizedAutoPrompt, capture, {
        maxAttempts: autoPromptMaxAttempts,
        pollMs: autoPromptPollMs,
      });
      await page.waitForTimeout(1500);
      await bestEffortDismissAistudioOverlays(page, capture);
      await bestEffortApplyRemixModal(page, capture);
      await persistCapture();
      if (localWebSocketServer && localProxyMessages.length > 0) {
        await maybeDispatchLocalProxyMessages(
          page,
          capture,
          localWebSocketServer,
          localProxyMessages,
          localProxyDelayMs,
          "post_prompt",
          persistCapture,
        );
      }
    }

    const startedAt = Date.now();
    let lastLocalProxyRedispatchAt = 0;
    let lastRemixApplyAt = 0;
    let lastAutoPromptAttemptAt = 0;
    while (Date.now() - startedAt < timeoutMs) {
      if (
        firstMatchAt &&
        lastMatchAt &&
        Date.now() - lastMatchAt >= settleMs &&
        !hasPendingLocalProxyResponse(capture)
      ) {
        break;
      }
      if (
        localWebSocketServer &&
        localProxyMessages.length > 0 &&
        countOpenUndispatchedLocalProxyConnections(capture) > 0 &&
        Date.now() - lastLocalProxyRedispatchAt >= Math.max(localProxyDelayMs, 1_500)
      ) {
        // Some run.app previews tear down the first local bridge and reconnect a
        // fresh ws://127.0.0.1:9998 client. Re-dispatch per connection so the
        // second live socket is not starved by an earlier global "already sent".
        await maybeDispatchLocalProxyMessages(
          page,
          capture,
          localWebSocketServer,
          localProxyMessages,
          localProxyDelayMs,
          "loop",
          persistCapture,
        );
        lastLocalProxyRedispatchAt = Date.now();
      }
      if (Date.now() - lastRemixApplyAt >= 5_000) {
        if (await bestEffortDismissAistudioOverlays(page, capture)) {
          await persistCapture();
        }
        if (await bestEffortApplyRemixModal(page, capture)) {
          await persistCapture();
        }
        lastRemixApplyAt = Date.now();
      }
      if (
        shouldRetryAutoPromptDuringPolling({
          normalizedAutoPrompt,
          autoPromptSubmitted,
          nowMs: Date.now(),
          lastAutoPromptAttemptAtMs: lastAutoPromptAttemptAt,
        })
      ) {
        autoPromptSubmitted = await bestEffortAutoPrompt(
          page,
          normalizedAutoPrompt,
          capture,
          {
            maxAttempts: 1,
            pollMs: 0,
          },
        );
        lastAutoPromptAttemptAt = Date.now();
        await persistCapture();
      }
      await page.waitForTimeout(500);
    }

    const finalSnapshot = await collectPageSnapshot(page, "final");
    const finalFrames = await collectFrameDiagnostics(page, "final");
    await writeFile(
      path.join(captureDir, "final-page.json"),
      `${JSON.stringify(finalSnapshot, null, 2)}\n`,
      "utf8",
    );
    await writeFile(
      path.join(captureDir, "final-frames.json"),
      `${JSON.stringify(finalFrames, null, 2)}\n`,
      "utf8",
    );
    await page.screenshot({
      path: path.join(captureDir, "final-page.png"),
      fullPage: true,
    });

    capture.finishedAt = new Date().toISOString();
    capture.finalUrl = page.url();
    capture.initialPage = {
      title: initialSnapshot.title,
      url: initialSnapshot.url,
      textboxes: initialSnapshot.textboxes.length,
      buttons: initialSnapshot.buttons.length,
      uiSignals: extractAistudioUiSignals(initialSnapshot),
    };
    capture.finalPage = {
      title: finalSnapshot.title,
      url: finalSnapshot.url,
      textboxes: finalSnapshot.textboxes.length,
      buttons: finalSnapshot.buttons.length,
      uiSignals: extractAistudioUiSignals(finalSnapshot),
    };

    const targetRpcSummary = summarizeTargetRpcContracts(capture);
    const normalizedTargetRpcContract =
      buildNormalizedTargetRpcContract(targetRpcSummary);
    const replayReadyTargetRpcContract = isReplayReadyTargetRpcContract(
      normalizedTargetRpcContract,
    );
    const targetRpcContractObjectKey = replayReadyTargetRpcContract
      ? buildTargetRpcContractObjectKey(input.runtimeStateObjectKey)
      : null;
    capture.targetRpcSummary = {
      capturedTargetRpcContract: targetRpcSummary.capturedTargetRpcContract,
      replayReadyTargetRpcContract,
      targetRpcPairCount: targetRpcSummary.targetRpcPairCount,
      codeAssistantOfflineCount: targetRpcSummary.codeAssistantOfflineCount,
      streamCodeAssistantOfflineGenerationCount:
        targetRpcSummary.streamCodeAssistantOfflineGenerationCount,
      targetRpcContractObjectKey,
    };

    await writeFile(
      path.join(captureDir, "target-rpc-summary.json"),
      `${JSON.stringify(targetRpcSummary, null, 2)}\n`,
      "utf8",
    );
    await writeFile(
      path.join(captureDir, "normalized-target-rpc-contract.json"),
      `${JSON.stringify(normalizedTargetRpcContract, null, 2)}\n`,
      "utf8",
    );
    let targetRpcContractMirrorPath = null;
    if (replayReadyTargetRpcContract && targetRpcContractObjectKey) {
      targetRpcContractMirrorPath = await persistJsonObjectMirror(
        targetRpcContractObjectKey,
        normalizedTargetRpcContract,
      );
    }
    for (const [index, pair] of targetRpcSummary.pairs.entries()) {
      const prefix = `${String(index + 1).padStart(2, "0")}-${pair.kind}`;
      await writeFile(
        path.join(captureDir, `${prefix}.json`),
        `${JSON.stringify(pair, null, 2)}\n`,
        "utf8",
      );
    }

    await persistCapture();

    const failUnlessTargetRpcCaptured = input?.failUnlessTargetRpcCaptured === true;
    const summary = buildProbeSummary({
      captureDir,
      capture,
      executablePath,
      runtimeState,
      appUrl,
      failUnlessTargetRpcCaptured,
      targetRpcSummary,
      normalizedTargetRpcContract,
      targetRpcContractObjectKey,
      targetRpcContractMirrorPath,
      localProxyRequest,
    });
    await writeFile(
      path.join(captureDir, "summary.json"),
      `${JSON.stringify(summary, null, 2)}\n`,
      "utf8",
    );
    printJsonAndSetExitCode(summary, summary.ok ? 0 : 2);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const status = Number(error?.status || error?.statusCode || 500);
    const code = error?.code || "aistudio_live_probe_failed";
    if (capture) {
      capture.failedAt = new Date().toISOString();
      capture.failure = {
        code,
        message,
        status,
      };
    }
    const targetRpcSummary = summarizeTargetRpcContracts(capture ?? {});
    const normalizedTargetRpcContract =
      buildNormalizedTargetRpcContract(targetRpcSummary);
    const replayReadyTargetRpcContract = isReplayReadyTargetRpcContract(
      normalizedTargetRpcContract,
    );
    const summary = {
      ok: false,
      captureDir,
      error: {
        code,
        message,
        status,
      },
      capturedTargetRpcContract: targetRpcSummary.capturedTargetRpcContract,
      replayReadyTargetRpcContract,
      browserProxyMode: capture?.browserProxyMode ?? null,
      browserProxyServer: capture?.browserProxyServer ?? null,
      browserProxyPreflight: capture?.browserProxyPreflight ?? null,
      matchedCodeAssistantOfflineCount:
        targetRpcSummary.codeAssistantOfflineCount,
      matchedStreamCodeAssistantOfflineGenerationCount:
        targetRpcSummary.streamCodeAssistantOfflineGenerationCount,
      targetRpcSummaryPath: captureDir
        ? path.join(captureDir, "target-rpc-summary.json")
        : null,
      normalizedTargetRpcContractPath: captureDir
        ? path.join(captureDir, "normalized-target-rpc-contract.json")
        : null,
    };
    if (captureDir) {
      if (capture) {
        await writeFile(
          path.join(captureDir, "capture.json"),
          `${JSON.stringify(capture, null, 2)}\n`,
          "utf8",
        ).catch(() => {});
      }
      await writeFile(
        path.join(captureDir, "target-rpc-summary.json"),
        `${JSON.stringify(targetRpcSummary, null, 2)}\n`,
        "utf8",
      ).catch(() => {});
      await writeFile(
        path.join(captureDir, "normalized-target-rpc-contract.json"),
        `${JSON.stringify(normalizedTargetRpcContract, null, 2)}\n`,
        "utf8",
      ).catch(() => {});
      await writeFile(
        path.join(captureDir, "summary.json"),
        `${JSON.stringify(summary, null, 2)}\n`,
        "utf8",
      ).catch(() => {});
    }
    printJsonAndSetExitCode(summary, 1);
  } finally {
    await localWebSocketServer?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
  }
}

export {
  assertBrowserProxyReachable,
  buildProbeSummary,
  buildBrowserProxyLaunchOptions,
  bestEffortApplyRemixModal,
  bestEffortDismissAistudioOverlays,
  bestEffortSelectGoogleAccount,
  bestEffortAutoPrompt,
  shouldRetryAutoPromptDuringPolling,
  createCaptureRequestIdFactory,
  createLocalWebSocketCaptureServer,
  createResponseRequestAttributor,
  extractAistudioUiSignals,
  isReplayReadyTargetRpcContract,
  resolveBrowserProxySettings,
  buildNormalizedTargetRpcContract,
  summarizeTargetRpcContracts,
};

if (process.env.AISTUDIO_PROBE_SUPPRESS_MAIN !== "1") {
  main();
}
