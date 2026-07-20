#!/usr/bin/env node

import fs from "node:fs/promises";
import { copyFileSync, existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import crypto from "node:crypto";

const DEFAULT_BASE_URL = "https://gemini.google.com";
const DEFAULT_API_BASE_URL = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_BROWSERLESS_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";
const DEFAULT_TEXT_MODEL = "gemini-3-flash-preview";
const DEFAULT_TTS_MODEL = "gemini-2.5-flash-preview-tts";
const DEFAULT_IMAGE_MODEL = "gemini-2.5-flash-image";
const DEFAULT_VIDEO_MODEL = "veo-3.1-generate-preview";
const DEFAULT_MUSIC_MODEL = "lyria-realtime-exp";
const DEFAULT_MUSIC_WS_URL =
  "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic";
const DEFAULT_LOCALE = "zh-CN";
const DEFAULT_AUTH_USER = "0";
const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_VIDEO_POLL_INTERVAL_MS = 5_000;
const DEFAULT_VIDEO_POLL_TIMEOUT_MS = 600_000;

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const entry = argv[index];
    if (!entry.startsWith("--")) {
      continue;
    }
    const key = entry.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      args[key] = true;
      continue;
    }
    args[key] = next;
    index += 1;
  }
  return args;
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function nowStamp() {
  return new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
}

async function ensureDir(dirPath) {
  await fs.mkdir(dirPath, { recursive: true });
}

async function readJson(filePath) {
  const raw = await fs.readFile(filePath, "utf8");
  const normalized = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;
  return JSON.parse(normalized);
}

async function writeJson(filePath, value) {
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function writeText(filePath, value) {
  await fs.writeFile(filePath, String(value ?? ""), "utf8");
}

async function writeBuffer(filePath, value) {
  await fs.writeFile(filePath, value);
}

function fileExists(filePath) {
  try {
    return existsSync(filePath);
  } catch {
    return false;
  }
}

function pathStatMtime(filePath) {
  try {
    return statSync(filePath).mtimeMs;
  } catch {
    return 0;
  }
}

function findLatestBrowserState(repoRoot) {
  const runtimeRoot = path.join(repoRoot, ".runtime", "gemini-canvas-program-runtime");
  if (!fileExists(runtimeRoot)) {
    return null;
  }
  const candidates = [];
  const queue = [runtimeRoot];
  while (queue.length) {
    const currentDir = queue.shift();
    let entries = [];
    try {
      entries = requireDirEntries(currentDir);
    } catch {
      continue;
    }
    for (const entry of entries) {
      const fullPath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        queue.push(fullPath);
        continue;
      }
      if (entry.isFile() && entry.name === "browser-state.json") {
        candidates.push(fullPath);
      }
    }
  }
  if (!candidates.length) {
    return null;
  }
  candidates.sort((left, right) => pathStatMtime(right) - pathStatMtime(left));
  return candidates[0];
}

function requireDirEntries(dirPath) {
  return readdirSync(dirPath, { withFileTypes: true });
}

function uniqueStrings(values) {
  return Array.from(
    new Set(
      (values || [])
        .map((value) => String(value ?? "").trim())
        .filter(Boolean),
    ),
  );
}

function normalizeOperation(rawOperation) {
  const operation = String(rawOperation ?? "")
    .trim()
    .toLowerCase();
  switch (operation) {
    case "text":
    case "chat":
      return "text";
    case "tts":
    case "audio-speech":
    case "speech":
      return "tts";
    case "image":
    case "image-create":
    case "images":
      return "image";
    case "music":
    case "music-create":
    case "audio":
      return "music";
    case "video":
    case "video-create":
    case "videos":
      return "video-create";
    default:
      return operation || "text";
  }
}

function defaultPromptForOperation(operation) {
  const marker = Date.now();
  switch (operation) {
    case "tts":
      return `BROWSERLESS_CANVAS_TTS_${marker} Read this sentence naturally: browserless Gemini Canvas TTS probe ok.`;
    case "image":
      return `BROWSERLESS_CANVAS_IMAGE_${marker} A high-contrast industrial terminal badge that says BROWSERLESS OK.`;
    case "music":
      return `BROWSERLESS_CANVAS_MUSIC_${marker} A short electronic cue with a clear pulse.`;
    case "video-create":
      return "A four second shot of a red cube slowly rotating on a white table.";
    case "text":
    default:
      return `BROWSERLESS_CANVAS_TEXT_${marker} Reply with exactly: ok`;
  }
}

function defaultModelForOperation(operation) {
  switch (operation) {
    case "tts":
      return DEFAULT_TTS_MODEL;
    case "image":
      return DEFAULT_IMAGE_MODEL;
    case "music":
      return DEFAULT_MUSIC_MODEL;
    case "video-create":
      return DEFAULT_VIDEO_MODEL;
    case "text":
    default:
      return DEFAULT_TEXT_MODEL;
  }
}

function inferBaseUrl(browserState) {
  return (
    normalizeString(browserState?.baseUrl) ??
    normalizeString(browserState?.base_url) ??
    originFromUrl(browserState?.shareUrl) ??
    originFromUrl(browserState?.canvasProgramUrl) ??
    originFromUrl(browserState?.pageUrl) ??
    DEFAULT_BASE_URL
  );
}

function inferApiBaseUrl(browserState, args) {
  return (
    normalizeString(args["api-base-url"]) ??
    normalizeString(browserState?.apiBaseUrl) ??
    normalizeString(browserState?.api_base_url) ??
    DEFAULT_API_BASE_URL
  );
}

function originFromUrl(rawUrl) {
  const normalized = normalizeString(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    return new URL(normalized).origin;
  } catch {
    return null;
  }
}

function resolveProgramHandlePath(browserState, repoRoot) {
  const explicit = normalizeString(browserState?.sourceProgramHandlePath);
  if (explicit && fileExists(explicit)) {
    return explicit;
  }
  const browserStateDir = path.dirname(browserState.__path);
  const sibling = path.join(browserStateDir, "program-handle.json");
  if (fileExists(sibling)) {
    return sibling;
  }
  const probeRoot = path.join(repoRoot, ".runtime", "gemini-canvas-program-handle-probe");
  if (!fileExists(probeRoot)) {
    return null;
  }
  let newest = null;
  const entries = requireDirEntries(probeRoot)
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(probeRoot, entry.name, "program-handle.json"))
    .filter((candidate) => fileExists(candidate));
  for (const candidate of entries) {
    if (!newest || pathStatMtime(candidate) > pathStatMtime(newest)) {
      newest = candidate;
    }
  }
  return newest;
}

async function resolveProgramHandle(browserState, repoRoot) {
  const handlePath = resolveProgramHandlePath(browserState, repoRoot);
  if (!handlePath) {
    return { path: null, json: null };
  }
  return {
    path: handlePath,
    json: await readJson(handlePath),
  };
}

function resolveProfileDir(browserState, programHandle, repoRoot, args) {
  const explicit = normalizeString(args["profile-dir"]);
  if (explicit && fileExists(explicit)) {
    return explicit;
  }
  const handleProfileDir = normalizeString(programHandle?.json?.runtimeProfileDir);
  if (handleProfileDir && fileExists(handleProfileDir)) {
    return handleProfileDir;
  }
  const browserProfileDir = normalizeString(browserState?.runtimeProfileDir);
  if (browserProfileDir && fileExists(browserProfileDir)) {
    return browserProfileDir;
  }
  const runtimeStateObjectKey = normalizeString(browserState?.runtimeStateObjectKey);
  if (runtimeStateObjectKey) {
    const candidate = path.join(
      repoRoot,
      ".runtime",
      "ai-gateway-objects",
      ...runtimeStateObjectKey.split("/"),
    );
    if (fileExists(candidate)) {
      return candidate;
    }
  }
  return null;
}

function walkFiles(rootDir, options = {}) {
  const {
    maxDepth = 6,
    fileName = null,
    includePath = null,
  } = options;
  const results = [];
  const queue = [{ dir: rootDir, depth: 0 }];
  while (queue.length) {
    const current = queue.shift();
    let entries = [];
    try {
      entries = requireDirEntries(current.dir);
    } catch {
      continue;
    }
    for (const entry of entries) {
      const fullPath = path.join(current.dir, entry.name);
      if (entry.isDirectory()) {
        if (current.depth < maxDepth) {
          queue.push({ dir: fullPath, depth: current.depth + 1 });
        }
        continue;
      }
      if (!entry.isFile()) {
        continue;
      }
      if (fileName && entry.name !== fileName) {
        continue;
      }
      if (includePath && !fullPath.toLowerCase().includes(includePath.toLowerCase())) {
        continue;
      }
      results.push(fullPath);
    }
  }
  return results;
}

function resolveStorageStatePath(repoRoot, profileDir, args) {
  const explicit = normalizeString(args["storage-state"]);
  const candidatePool = [];
  if (explicit) {
    candidatePool.push(path.resolve(explicit));
  }
  if (profileDir) {
    candidatePool.push(path.join(profileDir, "storage-state.json"));
    candidatePool.push(path.join(path.dirname(profileDir), "storage-state.json"));
  }
  const runtimeObjectRoot = path.join(repoRoot, ".runtime", "ai-gateway-objects");
  if (fileExists(runtimeObjectRoot)) {
    candidatePool.push(
      ...walkFiles(runtimeObjectRoot, {
        maxDepth: 7,
        fileName: "storage-state.json",
        includePath: "gemini-canvas",
      }),
    );
  }
  const runtimeRoot = path.join(repoRoot, ".runtime");
  if (fileExists(runtimeRoot)) {
    candidatePool.push(
      ...walkFiles(runtimeRoot, {
        maxDepth: 6,
        fileName: "storage-state.json",
        includePath: "gemini-canvas",
      }),
    );
  }

  const uniqueCandidates = uniqueStrings(candidatePool).filter((candidate) => fileExists(candidate));
  if (!uniqueCandidates.length) {
    return null;
  }
  uniqueCandidates.sort((left, right) => pathStatMtime(right) - pathStatMtime(left));
  return uniqueCandidates[0];
}

function cookieMatchesUrl(cookie, url) {
  const hostname = url.hostname.toLowerCase();
  const pathname = url.pathname || "/";
  const cookieDomain = String(cookie.domain || "").trim().toLowerCase();
  if (!cookieDomain) {
    return false;
  }
  const domain = cookieDomain.startsWith(".") ? cookieDomain.slice(1) : cookieDomain;
  const domainMatch = hostname === domain || hostname.endsWith(`.${domain}`);
  if (!domainMatch) {
    return false;
  }
  const cookiePath = String(cookie.path || "/").trim() || "/";
  return pathname.startsWith(cookiePath);
}

function buildPureHttpSession(storageState, requestUrl, baseUrl, authUser = DEFAULT_AUTH_USER) {
  const targetUrl = new URL(requestUrl);
  const base = new URL(baseUrl);
  const cookies = Array.isArray(storageState?.cookies) ? storageState.cookies : [];
  const matched = cookies
    .filter((cookie) => cookieMatchesUrl(cookie, targetUrl) || cookieMatchesUrl(cookie, base))
    .map((cookie, index) => ({
      name: String(cookie.name || "").trim(),
      value: String(cookie.value || "").trim(),
      path: String(cookie.path || "/").trim() || "/",
      domain: String(cookie.domain || "").trim(),
      index,
    }))
    .filter((cookie) => cookie.name && cookie.value);

  matched.sort((left, right) => {
    if (right.path.length !== left.path.length) {
      return right.path.length - left.path.length;
    }
    return left.index - right.index;
  });

  const sapisidCookie = matched.find((cookie) =>
    ["__Secure-1PAPISID", "__Secure-3PAPISID", "SAPISID"].includes(cookie.name),
  );
  if (!sapisidCookie) {
    throw new Error("missing SAPISID-compatible cookie in storage-state");
  }
  return {
    cookieHeader: matched.map((cookie) => `${cookie.name}=${cookie.value}`).join("; "),
    sapisid: sapisidCookie.value,
    authUser: String(authUser || DEFAULT_AUTH_USER).trim() || DEFAULT_AUTH_USER,
  };
}

function mergeSetCookie(session, setCookieValues) {
  if (!setCookieValues?.length) {
    return session;
  }
  const parsed = new Map();
  for (const segment of String(session.cookieHeader || "").split(";")) {
    const [name, ...rest] = segment.split("=");
    const trimmedName = name?.trim();
    if (!trimmedName) {
      continue;
    }
    parsed.set(trimmedName, rest.join("=").trim());
  }
  let nextSapisid = session.sapisid;
  for (const setCookie of setCookieValues) {
    const lines = String(setCookie || "")
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);
    for (const line of lines) {
      const cookiePair = line.split(";", 1)[0];
      const eqIndex = cookiePair.indexOf("=");
      if (eqIndex <= 0) {
        continue;
      }
      const name = cookiePair.slice(0, eqIndex).trim();
      const value = cookiePair.slice(eqIndex + 1).trim();
      if (!name) {
        continue;
      }
      parsed.set(name, value);
      if (["__Secure-1PAPISID", "__Secure-3PAPISID", "SAPISID"].includes(name) && value) {
        nextSapisid = value;
      }
    }
  }
  return {
    ...session,
    sapisid: nextSapisid,
    cookieHeader: Array.from(parsed.entries())
      .map(([name, value]) => `${name}=${value}`)
      .join("; "),
  };
}

function buildSapisidAuthorization(sapisid, origin, timestampSecs = null) {
  const ts = Number.isFinite(timestampSecs) ? Number(timestampSecs) : Math.floor(Date.now() / 1000);
  const digest = crypto
    .createHash("sha1")
    .update(`${ts} ${sapisid} ${origin}`)
    .digest("hex");
  return `SAPISIDHASH ${ts}_${digest} SAPISID1PHASH ${ts}_${digest} SAPISID3PHASH ${ts}_${digest}`;
}

function buildBrowserClientHints(headers) {
  headers["sec-ch-ua"] =
    headers["sec-ch-ua"] ??
    "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not A(Brand\";v=\"24\"";
  headers["sec-ch-ua-mobile"] = headers["sec-ch-ua-mobile"] ?? "?0";
  headers["sec-ch-ua-platform"] = headers["sec-ch-ua-platform"] ?? "\"Windows\"";
  headers["sec-ch-ua-platform-version"] =
    headers["sec-ch-ua-platform-version"] ?? "\"10.0.0\"";
  headers["sec-ch-ua-arch"] = headers["sec-ch-ua-arch"] ?? "\"x86\"";
  headers["sec-ch-ua-bitness"] = headers["sec-ch-ua-bitness"] ?? "\"64\"";
  headers["sec-ch-ua-model"] = headers["sec-ch-ua-model"] ?? "\"\"";
  headers["sec-ch-ua-wow64"] = headers["sec-ch-ua-wow64"] ?? "?0";
  headers["sec-ch-ua-form-factors"] =
    headers["sec-ch-ua-form-factors"] ?? "\"Desktop\"";
  headers["sec-fetch-dest"] = headers["sec-fetch-dest"] ?? "empty";
  headers["sec-fetch-mode"] = headers["sec-fetch-mode"] ?? "cors";
  headers["sec-fetch-site"] = headers["sec-fetch-site"] ?? "cross-site";
}

function buildJsonHeaders({
  session,
  pageOrigin,
  pageReferer,
  targetOrigin,
  locale,
  apiKey,
  authMode,
  includeBody = true,
}) {
  if (authMode.kind === "api_key_only") {
    const headers = {
      accept: "application/json",
      "user-agent": DEFAULT_BROWSERLESS_USER_AGENT,
      origin: pageOrigin,
      referer: pageReferer,
      "x-goog-authuser": session.authUser,
    };
    if (includeBody) {
      headers["content-type"] = "application/json";
    }
    if (apiKey && authMode.apiKeyPlacement !== "none") {
      headers["x-goog-api-key"] = apiKey;
    }
    return headers;
  }

  const headers = {
    accept: "application/json",
    "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
    "user-agent":
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
    origin: pageOrigin,
    referer: pageReferer,
    "x-same-domain": "1",
  };
  if (includeBody) {
    headers["content-type"] = "application/json";
  }
  if (apiKey && authMode.apiKeyPlacement !== "none") {
    headers["x-goog-api-key"] = apiKey;
  }

  buildBrowserClientHints(headers);
  headers["sec-fetch-site"] =
    String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase()
      ? "same-origin"
      : "cross-site";

  const authorization = buildSapisidAuthorization(session.sapisid, pageOrigin);
  headers.cookie = session.cookieHeader;
  headers.authorization = authorization;
  headers["x-origin"] = pageOrigin;
  headers["x-goog-authuser"] = session.authUser;
  return headers;
}

function buildJsonHeadersWithOptions({
  session,
  pageOrigin,
  pageReferer,
  targetOrigin,
  locale,
  apiKey,
  apiKeyPlacement = "none",
  includeSignedHeaders = true,
  preserveCrossOriginOrigin = true,
  preserveCrossOriginReferer = true,
  signedOriginOverride = null,
  refererOverride = null,
  includeBody = true,
}) {
  const referer = normalizeString(refererOverride) ?? pageReferer;
  const signedOrigin = normalizeString(signedOriginOverride) ?? pageOrigin;
  if (!includeSignedHeaders) {
    const headers = {
      accept: "application/json",
      "user-agent": DEFAULT_BROWSERLESS_USER_AGENT,
    };
    if (includeBody) {
      headers["content-type"] = "application/json";
    }
    if (apiKey && apiKeyPlacement !== "none") {
      headers["x-goog-api-key"] = apiKey;
      headers["x-goog-authuser"] = session.authUser;
    }
    const includePageContext =
      String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase() ||
      preserveCrossOriginOrigin;
    if (includePageContext) {
      headers.origin = pageOrigin;
      headers.referer = referer;
    } else if (preserveCrossOriginReferer) {
      headers.referer = referer;
    }
    return headers;
  }

  const headers = {
    accept: "application/json",
    "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
    "user-agent":
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
    "x-same-domain": "1",
  };
  if (includeBody) {
    headers["content-type"] = "application/json";
  }
  if (apiKey && apiKeyPlacement !== "none") {
    headers["x-goog-api-key"] = apiKey;
  }
  const includePageContext =
    String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase() ||
    preserveCrossOriginOrigin;
  if (includePageContext) {
    headers.origin = pageOrigin;
    headers.referer = referer;
  }

  buildBrowserClientHints(headers);
  headers["sec-fetch-site"] =
    String(targetOrigin || "").toLowerCase() === String(pageOrigin || "").toLowerCase()
      ? "same-origin"
      : "cross-site";

  headers.cookie = session.cookieHeader;
  headers.authorization = buildSapisidAuthorization(session.sapisid, signedOrigin);
  headers["x-origin"] = signedOrigin;
  headers["x-goog-authuser"] = session.authUser;
  headers.referer = referer;

  if (!includePageContext) {
    delete headers.origin;
    if (preserveCrossOriginReferer) {
      headers.referer = referer;
    } else {
      delete headers.referer;
    }
  }

  return headers;
}

function buildFormHeaders({
  session,
  pageOrigin,
  pageReferer,
  locale,
}) {
  const headers = {
    accept: "*/*",
    "accept-language": `${locale},zh;q=0.9,en;q=0.8`,
    "content-type": "application/x-www-form-urlencoded;charset=UTF-8",
    "user-agent":
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
    origin: pageOrigin,
    referer: pageReferer,
    "x-same-domain": "1",
    cookie: session.cookieHeader,
    authorization: buildSapisidAuthorization(session.sapisid, pageOrigin),
    "x-origin": pageOrigin,
    "x-goog-authuser": session.authUser,
  };
  buildBrowserClientHints(headers);
  headers["sec-fetch-site"] = "same-origin";
  return headers;
}

function appendApiKeyQueryIfMissing(requestUrl, apiKey) {
  const trimmedKey = normalizeString(apiKey);
  if (!trimmedKey) {
    return requestUrl;
  }
  const parsed = new URL(requestUrl);
  const existingKey = parsed.searchParams.get("key");
  if (!existingKey) {
    parsed.searchParams.set("key", trimmedKey);
  }
  return parsed.toString();
}

function buildAuthAttempts(requestUrl, apiKey) {
  const parsed = new URL(requestUrl);
  const host = parsed.host.toLowerCase();
  const pathname = parsed.pathname.toLowerCase();
  const isApiHost =
    host.includes("generativelanguage.googleapis.com") || host.includes("clients6.google.com");
  if (apiKey && isApiHost) {
    const preferQueryFirst =
      pathname.includes(":predict") ||
      pathname.includes("generatecontent") && pathname.includes("image");
    if (preferQueryFirst) {
      return [
        { label: "api_key_only_query", kind: "api_key_only", apiKeyPlacement: "query" },
        { label: "api_key_only_header", kind: "api_key_only", apiKeyPlacement: "header" },
        { label: "signed_session_query", kind: "signed_session", apiKeyPlacement: "query" },
        { label: "signed_session_header", kind: "signed_session", apiKeyPlacement: "header" },
      ];
    }
    return [
      { label: "api_key_only_header", kind: "api_key_only", apiKeyPlacement: "header" },
      { label: "signed_session_header", kind: "signed_session", apiKeyPlacement: "header" },
      { label: "signed_session_query", kind: "signed_session", apiKeyPlacement: "query" },
    ];
  }
  if (apiKey && host.includes("googleusercontent.com")) {
    return [{ label: "signed_session_download", kind: "signed_session", apiKeyPlacement: "none" }];
  }
  return [{ label: "signed_session", kind: "signed_session", apiKeyPlacement: "none" }];
}

function responseHeadersToObject(response) {
  const headers = {};
  response.headers.forEach((value, name) => {
    headers[name] = value;
  });
  return headers;
}

function getSetCookieValues(response) {
  if (typeof response.headers.getSetCookie === "function") {
    return response.headers.getSetCookie();
  }
  const single = response.headers.get("set-cookie");
  return single ? [single] : [];
}

async function collectTextResponse(response) {
  const text = await response.text();
  return {
    status: response.status,
    ok: response.ok,
    finalUrl: response.url,
    headers: responseHeadersToObject(response),
    setCookie: getSetCookieValues(response),
    text,
  };
}

async function collectBytesResponse(response) {
  const bytes = Buffer.from(await response.arrayBuffer());
  return {
    status: response.status,
    ok: response.ok,
    finalUrl: response.url,
    headers: responseHeadersToObject(response),
    setCookie: getSetCookieValues(response),
    bytes,
  };
}

function tryParseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function textPreview(value, max = 800) {
  const text = String(value ?? "");
  return text.length > max ? `${text.slice(0, max)}...[truncated]` : text;
}

function redactHeaders(headers) {
  const cloned = { ...(headers || {}) };
  for (const key of Object.keys(cloned)) {
    const lowered = key.toLowerCase();
    if (["cookie", "authorization", "x-goog-api-key"].includes(lowered)) {
      cloned[key] = "<redacted>";
    }
  }
  return cloned;
}

function mimeToExt(mimeType) {
  const normalized = String(mimeType || "").toLowerCase();
  if (normalized.includes("png")) {
    return ".png";
  }
  if (normalized.includes("jpeg") || normalized.includes("jpg")) {
    return ".jpg";
  }
  if (normalized.includes("webp")) {
    return ".webp";
  }
  if (normalized.includes("gif")) {
    return ".gif";
  }
  if (normalized.includes("wav")) {
    return ".wav";
  }
  if (normalized.includes("ogg")) {
    return ".ogg";
  }
  if (normalized.includes("mpeg") || normalized.includes("mp3")) {
    return ".mp3";
  }
  if (normalized.includes("mp4")) {
    return ".mp4";
  }
  if (normalized.includes("webm")) {
    return ".webm";
  }
  if (normalized.startsWith("audio/l16") || normalized.startsWith("audio/pcm")) {
    return ".pcm";
  }
  return ".bin";
}

function buildTextRequestBody(prompt) {
  return {
    contents: [
      {
        role: "user",
        parts: [{ text: prompt }],
      },
    ],
  };
}

function buildTtsRequestBody(prompt, voiceName = null) {
  const body = buildTextRequestBody(prompt);
  body.generationConfig = {
    responseModalities: ["AUDIO"],
  };
  if (normalizeString(voiceName)) {
    body.generationConfig.speechConfig = {
      voiceConfig: {
        prebuiltVoiceConfig: {
          voiceName: voiceName.trim(),
        },
      },
    };
  }
  return body;
}

function buildImageRequestBody(prompt, aspectRatio = "1:1") {
  return {
    contents: [
      {
        role: "user",
        parts: [{ text: prompt }],
      },
    ],
    generationConfig: {
      responseModalities: ["IMAGE"],
      imageConfig: {
        aspectRatio,
      },
    },
  };
}

function buildVideoCreateRequestBody(prompt, aspectRatio = "16:9", durationSeconds = null) {
  const parameters = {
    aspectRatio,
  };
  if (Number.isFinite(durationSeconds) && durationSeconds > 0) {
    parameters.durationSeconds = durationSeconds;
  }
  return {
    instances: [{ prompt }],
    parameters,
  };
}

function extractTextFromGenerateContentResponse(body) {
  const parts = body?.candidates?.[0]?.content?.parts;
  if (!Array.isArray(parts)) {
    return null;
  }
  const texts = parts
    .map((part) => normalizeString(part?.text))
    .filter(Boolean);
  return texts.length ? texts.join("\n") : null;
}

function extractAudioFromGenerateContentResponse(body) {
  const parts = body?.candidates?.[0]?.content?.parts;
  if (!Array.isArray(parts)) {
    return null;
  }
  for (const part of parts) {
    const inlineData = part?.inlineData ?? part?.inline_data;
    const mimeType =
      normalizeString(inlineData?.mimeType) ??
      normalizeString(inlineData?.mime_type) ??
      "audio/L16;codec=pcm;rate=24000";
    const raw = normalizeString(inlineData?.data);
    if (!raw) {
      continue;
    }
    return {
      mimeType,
      bytes: Buffer.from(raw, "base64"),
    };
  }
  return null;
}

function extractInlineImageFromGenerateContentResponse(body) {
  const candidates = Array.isArray(body?.candidates) ? body.candidates : [];
  for (const candidate of candidates) {
    const parts = candidate?.content?.parts;
    if (!Array.isArray(parts)) {
      continue;
    }
    for (const part of parts) {
      const inlineData = part?.inlineData ?? part?.inline_data;
      const mimeType =
        normalizeString(inlineData?.mimeType) ??
        normalizeString(inlineData?.mime_type) ??
        "image/png";
      const raw = normalizeString(inlineData?.data);
      if (!raw || !mimeType.startsWith("image/")) {
        continue;
      }
      return {
        mimeType,
        bytes: Buffer.from(raw, "base64"),
      };
    }
  }
  return null;
}

function extractImagenImages(body) {
  const images = [];
  const collections = [];
  if (Array.isArray(body?.generatedImages)) {
    collections.push(body.generatedImages);
  }
  if (Array.isArray(body?.predictions)) {
    collections.push(body.predictions);
  }
  for (const collection of collections) {
    for (const candidate of collection) {
      const imageRecord = candidate?.image ?? candidate;
      const raw =
        normalizeString(imageRecord?.imageBytes) ??
        normalizeString(imageRecord?.bytesBase64Encoded) ??
        normalizeString(imageRecord?.b64_json) ??
        normalizeString(imageRecord?.b64Json);
      if (!raw) {
        continue;
      }
      const mimeType =
        normalizeString(imageRecord?.mimeType) ??
        normalizeString(imageRecord?.mime_type) ??
        "image/png";
      images.push({
        mimeType,
        bytes: Buffer.from(raw, "base64"),
      });
    }
  }
  return images;
}

function parsePcmSampleRate(mimeType) {
  for (const segment of String(mimeType || "").split(";")) {
    const trimmed = segment.trim();
    if (trimmed.startsWith("rate=")) {
      const parsed = Number(trimmed.slice("rate=".length));
      if (Number.isFinite(parsed) && parsed > 0) {
        return parsed;
      }
    }
  }
  return 24_000;
}

function parsePcmChannels(mimeType) {
  for (const segment of String(mimeType || "").split(";")) {
    const trimmed = segment.trim();
    if (trimmed.startsWith("channels=")) {
      const parsed = Number(trimmed.slice("channels=".length));
      if (Number.isFinite(parsed) && parsed > 0) {
        return parsed;
      }
    }
  }
  return 1;
}

function pcmAudioToWavBytes(rawPcm, mimeType) {
  const sampleRate = parsePcmSampleRate(mimeType);
  const channels = parsePcmChannels(mimeType);
  const bitsPerSample = 16;
  const pcmLe = Buffer.from(rawPcm);
  if (String(mimeType || "").toLowerCase().startsWith("audio/l16")) {
    for (let index = 0; index + 1 < pcmLe.length; index += 2) {
      const first = pcmLe[index];
      pcmLe[index] = pcmLe[index + 1];
      pcmLe[index + 1] = first;
    }
  }

  const byteRate = sampleRate * channels * (bitsPerSample / 8);
  const blockAlign = channels * (bitsPerSample / 8);
  const dataLen = pcmLe.length;
  const riffLen = 36 + dataLen;

  const header = Buffer.alloc(44);
  header.write("RIFF", 0, "ascii");
  header.writeUInt32LE(riffLen, 4);
  header.write("WAVE", 8, "ascii");
  header.write("fmt ", 12, "ascii");
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20);
  header.writeUInt16LE(channels, 22);
  header.writeUInt32LE(sampleRate, 24);
  header.writeUInt32LE(byteRate, 28);
  header.writeUInt16LE(blockAlign, 32);
  header.writeUInt16LE(bitsPerSample, 34);
  header.write("data", 36, "ascii");
  header.writeUInt32LE(dataLen, 40);
  return Buffer.concat([header, pcmLe]);
}

function extractVideoUriFromOperation(body) {
  return (
    normalizeString(body?.response?.generateVideoResponse?.generatedSamples?.[0]?.video?.uri) ??
    normalizeString(body?.response?.generatedVideos?.[0]?.video?.uri) ??
    normalizeString(body?.response?.generated_videos?.[0]?.video?.uri)
  );
}

function summarizeJsonBody(body) {
  if (!body || typeof body !== "object") {
    return null;
  }
  return {
    name: normalizeString(body?.name),
    done: body?.done === true,
    error: body?.error ?? null,
    hasCandidates: Array.isArray(body?.candidates) ? body.candidates.length : 0,
    hasGeneratedImages: Array.isArray(body?.generatedImages) ? body.generatedImages.length : 0,
    hasPredictions: Array.isArray(body?.predictions) ? body.predictions.length : 0,
    videoUri: extractVideoUriFromOperation(body),
  };
}

function normalizeRemoteMusicWsUrl(rawUrl) {
  const normalized = normalizeString(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    const host = parsed.hostname.toLowerCase();
    if (host === "127.0.0.1" || host === "localhost" || host === "::1") {
      return null;
    }
    return parsed.toString();
  } catch {
    return null;
  }
}

function extractXsrfToken(bodyText) {
  const patterns = [
    /\["xsrf","([^"]+)"\]/i,
    /"xsrf"\s*,\s*"([^"]+)"/i,
  ];
  for (const pattern of patterns) {
    const match = String(bodyText || "").match(pattern);
    if (match?.[1]?.trim()) {
      return match[1].trim();
    }
  }
  return null;
}

function deriveProgramHandle(bodyText, baseUrl) {
  const text = String(bodyText || "");
  const appPaths = uniqueStrings(text.match(/\/app\/[0-9a-f]{16}/gi) || []);
  const conversationIds = uniqueStrings(text.match(/\bc_[0-9a-f]{16}\b/gi) || []);
  const responseIds = uniqueStrings(text.match(/\br_[0-9a-f]{16}\b/gi) || []);
  const suffix = conversationIds
    .map((value) => value.replace(/^c_/i, ""))
    .find((candidate) => appPaths.includes(`/app/${candidate}`));
  const appPath = appPaths[0] ?? (suffix ? `/app/${suffix}` : null);
  return {
    appPath,
    programUrl: appPath ? `${baseUrl.replace(/\/+$/, "")}${appPath}` : null,
    conversationId: suffix ? `c_${suffix}` : conversationIds[0] ?? null,
    responseId: responseIds[0] ?? null,
    sourceSurface: /Browser API Proxy Client/i.test(text) ? "canvas_proxy_client" : null,
  };
}

async function archiveAttempt(outDir, prefix, attempt, responseRecord) {
  await writeJson(path.join(outDir, `${prefix}.request.json`), attempt);
  const responseJson = {
    status: responseRecord.status,
    ok: responseRecord.ok,
    finalUrl: responseRecord.finalUrl,
    headers: responseRecord.headers,
    bodyPreview: "text" in responseRecord ? textPreview(responseRecord.text, 4000) : `<${responseRecord.bytes.length} bytes>`,
  };
  await writeJson(path.join(outDir, `${prefix}.response.json`), responseJson);
  if ("text" in responseRecord) {
    await writeText(path.join(outDir, `${prefix}.response.body.txt`), responseRecord.text);
  } else {
    await writeBuffer(path.join(outDir, `${prefix}.response.body.bin`), responseRecord.bytes);
  }
}

function looksLikeQuotaOrPlanGate(status, bodyText) {
  if (!matchesAny(status, [400, 403, 429])) {
    return false;
  }
  const normalized = String(bodyText || "").toLowerCase();
  return /(quota|paid plans|billing|resource_exhausted|rate limits?)/i.test(normalized);
}

function matchesAny(value, accepted) {
  return accepted.includes(value);
}

async function sendExactMinimalApiKeyOnlyJson(context, prefix, requestUrl, requestBody, timeoutMs) {
  const url = appendApiKeyQueryIfMissing(requestUrl, context.apiKey);
  const headers = {
    Accept: "application/json",
    "Content-Type": "application/json",
    "X-Goog-AuthUser": context.session.authUser,
    "x-goog-api-key": context.apiKey,
    Origin: context.pageOrigin,
    Referer: context.pageReferer,
    "User-Agent": DEFAULT_BROWSERLESS_USER_AGENT,
  };
  const requestRecord = {
    label: "exact_minimal_api_key_only",
    kind: "api_key_only_exact",
    method: "POST",
    url,
    headers,
    headersRedacted: redactHeaders(headers),
    body: requestBody,
  };
  const response = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(requestBody),
    redirect: "follow",
    signal: AbortSignal.timeout(timeoutMs),
  });
  const responseRecord = await collectTextResponse(response);
  await archiveAttempt(outDirFromContext(context), prefix, requestRecord, responseRecord);
  return {
    ok: responseRecord.ok,
    request: requestRecord,
    response: responseRecord,
    responseJson: tryParseJson(responseRecord.text),
  };
}

async function sendJsonWithAuthAttempts(context, prefix, requestUrl, requestBody, timeoutMs) {
  const attempts = buildAuthAttempts(requestUrl, context.apiKey);
  const results = [];
  for (let index = 0; index < attempts.length; index += 1) {
    const attempt = attempts[index];
    const url =
      attempt.apiKeyPlacement === "query"
        ? appendApiKeyQueryIfMissing(requestUrl, context.apiKey)
        : requestUrl;
    const headers = buildJsonHeaders({
      session: context.session,
      pageOrigin: context.pageOrigin,
      pageReferer: context.pageReferer,
      targetOrigin: originFromUrl(url) ?? context.pageOrigin,
      locale: context.locale,
      apiKey: context.apiKey,
      authMode: attempt,
      includeBody: true,
    });
    const requestRecord = {
      label: attempt.label,
      kind: attempt.kind,
      apiKeyPlacement: attempt.apiKeyPlacement,
      method: "POST",
      url,
      headers,
      body: requestBody,
      headersRedacted: redactHeaders(headers),
    };
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(requestBody),
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });
    const responseRecord = await collectTextResponse(response);
    context.session = mergeSetCookie(context.session, responseRecord.setCookie);
    results.push({
      request: requestRecord,
      response: responseRecord,
    });
    await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(index + 1).padStart(2, "0")}`, requestRecord, responseRecord);
    if (responseRecord.ok) {
      return {
        ok: true,
        url,
        attempt,
        request: requestRecord,
        response: responseRecord,
        responseJson: tryParseJson(responseRecord.text),
        attempts: results,
      };
    }
  }
  const last = results[results.length - 1];
  return {
    ok: false,
    request: last?.request ?? null,
    response: last?.response ?? null,
    responseJson: tryParseJson(last?.response?.text),
    attempts: results,
  };
}

async function sendJsonWithCustomAttempts(context, prefix, attempts, timeoutMs) {
  const results = [];
  for (let index = 0; index < attempts.length; index += 1) {
    const attempt = attempts[index];
    const candidateKeys = [];
    if (attempt.includeSignedHeaders && normalizeString(context.apiKey)) {
      candidateKeys.push(null, context.apiKey);
    } else if (normalizeString(context.apiKey)) {
      candidateKeys.push(context.apiKey);
    } else {
      candidateKeys.push(null);
    }
    const uniqueKeys = [];
    for (const candidateKey of candidateKeys) {
      const keyTag = candidateKey ?? "__none__";
      if (!uniqueKeys.includes(keyTag)) {
        uniqueKeys.push(keyTag);
      }
    }

    for (const candidateKeyTag of uniqueKeys) {
      const candidateKey = candidateKeyTag === "__none__" ? null : candidateKeyTag;
      const apiKeyPlacements =
        candidateKey && /generativelanguage\.googleapis\.com|clients6\.google\.com/i.test(attempt.requestUrl)
          ? ["header", "query"]
          : ["none"];
      if (candidateKey && attempt.includeApiKey !== false && !apiKeyPlacements.includes("header")) {
        apiKeyPlacements.unshift("header");
      }
      for (const apiKeyPlacement of apiKeyPlacements) {
        const url =
          candidateKey && apiKeyPlacement === "query"
            ? appendApiKeyQueryIfMissing(attempt.requestUrl, candidateKey)
            : attempt.requestUrl;
        const headers = buildJsonHeadersWithOptions({
          session: context.session,
          pageOrigin: context.pageOrigin,
          pageReferer: context.pageReferer,
          targetOrigin: originFromUrl(url) ?? context.pageOrigin,
          locale: context.locale,
          apiKey: candidateKey,
          apiKeyPlacement,
          includeSignedHeaders: attempt.includeSignedHeaders,
          preserveCrossOriginOrigin: attempt.preserveCrossOriginOrigin,
          preserveCrossOriginReferer: attempt.preserveCrossOriginReferer,
          signedOriginOverride: attempt.signedOriginOverride,
          refererOverride: attempt.refererOverride,
          includeBody: true,
        });
        const requestRecord = {
          label: attempt.label,
          kind: attempt.kind,
          requestVariant: attempt.requestVariant,
          apiKeyPlacement,
          apiKeyPresent: Boolean(candidateKey),
          method: "POST",
          url,
          headers,
          headersRedacted: redactHeaders(headers),
          body: attempt.requestBody,
        };
        const response = await fetch(url, {
          method: "POST",
          headers,
          body: JSON.stringify(attempt.requestBody),
          redirect: "follow",
          signal: AbortSignal.timeout(timeoutMs),
        });
        const responseRecord = await collectTextResponse(response);
        context.session = mergeSetCookie(context.session, responseRecord.setCookie);
        results.push({
          request: requestRecord,
          response: responseRecord,
        });
        await archiveAttempt(
          outDirFromContext(context),
          `${prefix}.attempt-${String(results.length).padStart(2, "0")}`,
          requestRecord,
          responseRecord,
        );
        const responseJson = tryParseJson(responseRecord.text);
        if (responseRecord.ok) {
          return {
            ok: true,
            request: requestRecord,
            response: responseRecord,
            responseJson,
            attempts: results,
          };
        }
      }
    }
  }
  const last = results[results.length - 1];
  return {
    ok: false,
    request: last?.request ?? null,
    response: last?.response ?? null,
    responseJson: tryParseJson(last?.response?.text),
    attempts: results,
  };
}

async function sendGetBytesWithAuthAttempts(context, prefix, requestUrl, timeoutMs) {
  const attempts = buildAuthAttempts(requestUrl, context.apiKey);
  const results = [];
  for (let index = 0; index < attempts.length; index += 1) {
    const attempt = attempts[index];
    const url =
      attempt.apiKeyPlacement === "query"
        ? appendApiKeyQueryIfMissing(requestUrl, context.apiKey)
        : requestUrl;
    const headers = buildJsonHeaders({
      session: context.session,
      pageOrigin: context.pageOrigin,
      pageReferer: context.pageReferer,
      targetOrigin: originFromUrl(url) ?? context.pageOrigin,
      locale: context.locale,
      apiKey: context.apiKey,
      authMode: attempt,
      includeBody: false,
    });
    headers.accept = "*/*";
    delete headers["content-type"];
    const requestRecord = {
      label: attempt.label,
      kind: attempt.kind,
      apiKeyPlacement: attempt.apiKeyPlacement,
      method: "GET",
      url,
      headers,
      headersRedacted: redactHeaders(headers),
    };
    const response = await fetch(url, {
      method: "GET",
      headers,
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });
    const responseRecord = await collectBytesResponse(response);
    context.session = mergeSetCookie(context.session, responseRecord.setCookie);
    results.push({
      request: requestRecord,
      response: responseRecord,
    });
    await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(index + 1).padStart(2, "0")}`, requestRecord, responseRecord);
    if (responseRecord.ok) {
      return {
        ok: true,
        url,
        attempt,
        request: requestRecord,
        response: responseRecord,
        attempts: results,
      };
    }
  }
  const last = results[results.length - 1];
  return {
    ok: false,
    request: last?.request ?? null,
    response: last?.response ?? null,
    attempts: results,
  };
}

async function sendFormRequest(context, prefix, requestUrl, formBody, timeoutMs, allowXsrfRetry = true) {
  let currentBody = formBody;
  let attemptIndex = 0;
  while (true) {
    attemptIndex += 1;
    const headers = buildFormHeaders({
      session: context.session,
      pageOrigin: context.pageOrigin,
      pageReferer: context.pageReferer,
      locale: context.locale,
    });
    const requestRecord = {
      method: "POST",
      url: requestUrl,
      headers,
      body: currentBody,
      headersRedacted: redactHeaders(headers),
    };
    const response = await fetch(requestUrl, {
      method: "POST",
      headers,
      body: currentBody,
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });
    const responseRecord = await collectTextResponse(response);
    context.session = mergeSetCookie(context.session, responseRecord.setCookie);
    await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(attemptIndex).padStart(2, "0")}`, requestRecord, responseRecord);
    if (responseRecord.ok) {
      return {
        ok: true,
        request: requestRecord,
        response: responseRecord,
        responseJson: tryParseJson(responseRecord.text),
      };
    }
    if (allowXsrfRetry) {
      const xsrfToken = extractXsrfToken(responseRecord.text);
      if (xsrfToken && !currentBody.includes(`at=${encodeURIComponent(xsrfToken)}`)) {
        const params = new URLSearchParams(currentBody.endsWith("&") ? currentBody.slice(0, -1) : currentBody);
        params.set("at", xsrfToken);
        currentBody = `${params.toString()}&`;
        allowXsrfRetry = false;
        continue;
      }
    }
    return {
      ok: false,
      request: requestRecord,
      response: responseRecord,
      responseJson: tryParseJson(responseRecord.text),
    };
  }
}

function outDirFromContext(context) {
  return context.outDir;
}

async function probeText(context) {
  const requestUrl = `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:generateContent`;
  const requestBody = buildTextRequestBody(context.prompt);
  const sendResult = await sendJsonWithAuthAttempts(
    context,
    "text.generate-content",
    requestUrl,
    requestBody,
    context.timeoutMs,
  );
  const text = extractTextFromGenerateContentResponse(sendResult.responseJson);
  return {
    kind: "official_like_generate_content",
    requestUrl,
    requestBody,
    status: sendResult.response?.status ?? null,
    ok: sendResult.ok,
    responseText: text,
    bodySummary: summarizeJsonBody(sendResult.responseJson),
  };
}

async function probeTts(context) {
  const requestUrl = `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:generateContent`;
  const requestBody = buildTtsRequestBody(context.prompt, context.voiceName);
  const sendResult = await sendJsonWithAuthAttempts(
    context,
    "tts.generate-content",
    requestUrl,
    requestBody,
    context.timeoutMs,
  );
  const audio = extractAudioFromGenerateContentResponse(sendResult.responseJson);
  let audioAsset = null;
  if (audio) {
    const rawExt = mimeToExt(audio.mimeType);
    const rawPath = path.join(context.outDir, `tts-audio${rawExt}`);
    await writeBuffer(rawPath, audio.bytes);
    let wavPath = null;
    if (
      String(audio.mimeType).toLowerCase().startsWith("audio/l16") ||
      String(audio.mimeType).toLowerCase().startsWith("audio/pcm")
    ) {
      const wavBytes = pcmAudioToWavBytes(audio.bytes, audio.mimeType);
      wavPath = path.join(context.outDir, "tts-audio.wav");
      await writeBuffer(wavPath, wavBytes);
    }
    audioAsset = {
      mimeType: audio.mimeType,
      bytesLength: audio.bytes.length,
      rawPath,
      wavPath,
    };
  }
  return {
    kind: "official_like_generate_content_tts",
    requestUrl,
    requestBody,
    status: sendResult.response?.status ?? null,
    ok: sendResult.ok,
    bodySummary: summarizeJsonBody(sendResult.responseJson),
    audioAsset,
  };
}

async function probeImage(context) {
  const previewBaseUrl = "https://geminiweb-pa.clients6.google.com/v1beta";
  const previewModel = "gemini-2.5-flash-image-preview";
  const officialRequestUrl = `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:generateContent`;
  const previewRequestUrl = `${previewBaseUrl}/models/${previewModel}:generateContent`;
  const shareUrl = normalizeString(context.browserState.shareUrl) ?? context.pageReferer;
  const appUrl =
    normalizeString(context.browserState.canvasProgramUrl) ??
    `${context.baseUrl.replace(/\/+$/, "")}/app`;
  const directHttpTextImageBody = {
    contents: [
      {
        role: "user",
        parts: [{ text: context.prompt }],
      },
    ],
    generationConfig: {
      responseModalities: ["TEXT", "IMAGE"],
      imageConfig: {
        aspectRatio: context.aspectRatio,
      },
    },
  };
  const officialDirectResult = await sendJsonWithAuthAttempts(
    context,
    "image.google-api-official",
    officialRequestUrl,
    directHttpTextImageBody,
    context.timeoutMs,
  );
  const exactOfficialResult = await sendExactMinimalApiKeyOnlyJson(
    context,
    "image.google-api-exact",
    officialRequestUrl,
    directHttpTextImageBody,
    context.timeoutMs,
  );
  if (
    exactOfficialResult.response &&
    looksLikeQuotaOrPlanGate(
      exactOfficialResult.response.status,
      exactOfficialResult.response.text,
    )
  ) {
    return {
      kind: "official_like_generate_content_image",
      requestUrl: exactOfficialResult.request.url,
      requestBody: exactOfficialResult.request.body,
      status: exactOfficialResult.response.status,
      ok: false,
      bodySummary: summarizeJsonBody(exactOfficialResult.responseJson),
      imageAsset: null,
      error: "image_official_gate",
    };
  }
  let imageAsset = extractInlineImageFromGenerateContentResponse(officialDirectResult.responseJson);
  if (!imageAsset) {
    const images = extractImagenImages(officialDirectResult.responseJson);
    imageAsset = images[0] ?? null;
  }
  const officialErrorMessage = normalizeString(officialDirectResult.responseJson?.error?.message);
  const officialGateAccepted =
    (officialDirectResult.response?.status === 429 ||
      officialDirectResult.response?.status === 403 ||
      officialDirectResult.response?.status === 400) &&
    Boolean(officialErrorMessage) &&
    /(quota|paid plans|billing|resource_exhausted|rate limits?)/i.test(officialErrorMessage);
  if (imageAsset || officialGateAccepted) {
    let savedOfficialAsset = null;
    if (imageAsset) {
      const ext = mimeToExt(imageAsset.mimeType);
      const assetPath = path.join(context.outDir, `image-asset${ext}`);
      await writeBuffer(assetPath, imageAsset.bytes);
      savedOfficialAsset = {
        mimeType: imageAsset.mimeType,
        bytesLength: imageAsset.bytes.length,
        assetPath,
      };
    }
    return {
      kind: "official_like_generate_content_image",
      requestUrl: officialDirectResult.request?.url ?? officialRequestUrl,
      requestBody: officialDirectResult.request?.body ?? directHttpTextImageBody,
      status: officialDirectResult.response?.status ?? null,
      ok: Boolean(officialDirectResult.ok && imageAsset),
      bodySummary: summarizeJsonBody(officialDirectResult.responseJson),
      imageAsset: savedOfficialAsset,
      error: imageAsset ? null : "image_official_gate",
    };
  }
  const previewImageOnlyBody = buildImageRequestBody(context.prompt, context.aspectRatio);
  const plainMinimalImageBody = {
    contents: directHttpTextImageBody.contents,
    generationConfig: {
      imageConfig: {
        aspectRatio: context.aspectRatio,
      },
    },
  };
  const imagenPredictBody = {
    instances: [{ prompt: context.prompt }],
    parameters: {
      sampleCount: 1,
      aspectRatio: context.aspectRatio,
    },
  };
  const attempts = [
    {
      label: "clients6_signed_app_text_image",
      kind: "generateContent",
      requestVariant: "preview_text_image",
      requestUrl: previewRequestUrl,
      requestBody: directHttpTextImageBody,
      includeSignedHeaders: true,
      preserveCrossOriginOrigin: true,
      preserveCrossOriginReferer: true,
      signedOriginOverride: context.pageOrigin,
      refererOverride: appUrl,
    },
    {
      label: "clients6_signed_share_text_image",
      kind: "generateContent",
      requestVariant: "preview_text_image",
      requestUrl: previewRequestUrl,
      requestBody: directHttpTextImageBody,
      includeSignedHeaders: true,
      preserveCrossOriginOrigin: true,
      preserveCrossOriginReferer: true,
      signedOriginOverride: context.pageOrigin,
      refererOverride: shareUrl,
    },
    {
      label: "clients6_signed_share_image_only",
      kind: "generateContent",
      requestVariant: "preview_image_only",
      requestUrl: previewRequestUrl,
      requestBody: previewImageOnlyBody,
      includeSignedHeaders: true,
      preserveCrossOriginOrigin: true,
      preserveCrossOriginReferer: true,
      signedOriginOverride: context.pageOrigin,
      refererOverride: shareUrl,
    },
    {
      label: "clients6_plain_share_text_image",
      kind: "generateContent",
      requestVariant: "preview_text_image",
      requestUrl: previewRequestUrl,
      requestBody: directHttpTextImageBody,
      includeSignedHeaders: false,
      preserveCrossOriginOrigin: true,
      preserveCrossOriginReferer: true,
      signedOriginOverride: null,
      refererOverride: shareUrl,
    },
    {
      label: "clients6_plain_minimal_image_only",
      kind: "generateContent",
      requestVariant: "preview_minimal_image_only",
      requestUrl: previewRequestUrl,
      requestBody: plainMinimalImageBody,
      includeSignedHeaders: false,
      preserveCrossOriginOrigin: false,
      preserveCrossOriginReferer: false,
      signedOriginOverride: null,
      refererOverride: null,
    },
    {
      label: "google_api_official",
      kind: "generateContent",
      requestVariant: "official_image_only",
      requestUrl: officialRequestUrl,
      requestBody: buildImageRequestBody(context.prompt, context.aspectRatio),
      includeSignedHeaders: false,
      preserveCrossOriginOrigin: false,
      preserveCrossOriginReferer: true,
      signedOriginOverride: null,
      refererOverride: appUrl,
    },
    {
      label: "google_api_imagen4_predict",
      kind: "imagenPredict",
      requestVariant: "imagen_predict",
      requestUrl: `${context.apiBaseUrl.replace(/\/+$/, "")}/models/imagen-4.0-generate-001:predict`,
      requestBody: imagenPredictBody,
      includeSignedHeaders: false,
      preserveCrossOriginOrigin: false,
      preserveCrossOriginReferer: true,
      signedOriginOverride: null,
      refererOverride: appUrl,
    },
  ];

  let sendResult = null;
  imageAsset = null;
  let responseKind = null;
  for (const attempt of attempts) {
    sendResult = await sendJsonWithCustomAttempts(
      context,
      `image.${attempt.label}`,
      [attempt],
      context.timeoutMs,
    );
    if (!sendResult.ok) {
      continue;
    }
    if (attempt.kind === "generateContent") {
      imageAsset = extractInlineImageFromGenerateContentResponse(sendResult.responseJson);
      responseKind = attempt.kind;
    } else if (attempt.kind === "imagenPredict") {
      const images = extractImagenImages(sendResult.responseJson);
      imageAsset = images[0] ?? null;
      responseKind = attempt.kind;
    }
    if (imageAsset) {
      break;
    }
  }

  let savedAsset = null;
  if (imageAsset) {
    const ext = mimeToExt(imageAsset.mimeType);
    const assetPath = path.join(context.outDir, `image-asset${ext}`);
    await writeBuffer(assetPath, imageAsset.bytes);
    savedAsset = {
      mimeType: imageAsset.mimeType,
      bytesLength: imageAsset.bytes.length,
      assetPath,
    };
  }
  return {
    kind: responseKind === "imagenPredict" ? "imagen_predict_image" : "official_like_generate_content_image",
    requestUrl: sendResult?.request?.url ?? officialRequestUrl,
    requestBody: sendResult?.request?.body ?? buildImageRequestBody(context.prompt, context.aspectRatio),
    status: sendResult.response?.status ?? null,
    ok: Boolean(sendResult?.ok && imageAsset),
    bodySummary: summarizeJsonBody(sendResult.responseJson),
    imageAsset: savedAsset,
  };
}

async function probeVideoCreate(context) {
  const invokeContractRequestUrl = normalizeString(
    context.browserState?.canvasProgramInvokeContract?.requestUrl,
  );
  const browserStateVideoInvokePath = normalizeString(context.browserState?.videoInvokePath);
  const requestUrl = invokeContractRequestUrl
    ? invokeContractRequestUrl
    : browserStateVideoInvokePath
      ? buildVideoInvokeUrl(
          context.apiBaseUrl,
          browserStateVideoInvokePath,
          context.model,
        )
      : `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:predictLongRunning`;
  const requestBody = buildVideoCreateRequestBody(
    context.prompt,
    context.aspectRatio,
    context.durationSeconds,
  );
  const exactAttempts = [];
  for (let retry = 0; retry < 3; retry += 1) {
    const exactResult = await sendExactMinimalApiKeyOnlyJson(
      context,
      `video.create.exact-${String(retry + 1).padStart(2, "0")}`,
      requestUrl,
      requestBody,
      context.timeoutMs,
    );
    exactAttempts.push(exactResult);
    if (
      exactResult.response &&
      looksLikeQuotaOrPlanGate(exactResult.response.status, exactResult.response.text)
    ) {
      return {
        kind: "official_like_predict_long_running",
        requestUrl: exactResult.request.url,
        requestBody: exactResult.request.body,
        status: exactResult.response.status,
        ok: false,
        bodySummary: summarizeJsonBody(exactResult.responseJson),
        finalPollSummary: null,
        pollRecords: null,
        asset: null,
        error: "video_official_gate",
      };
    }
    const exactErrorMessage = normalizeString(exactResult.responseJson?.error?.message);
    if (!/origin doesn't match host for xd3/i.test(exactErrorMessage || "")) {
      break;
    }
    await sleep(750);
  }
  const createResult = await sendJsonWithAuthAttempts(
    context,
    "video.create",
    requestUrl,
    requestBody,
    context.timeoutMs,
  );
  const operationName = normalizeString(createResult.responseJson?.name);
  if (!operationName) {
    return {
      kind: "official_like_predict_long_running",
      requestUrl,
      requestBody,
      status: createResult.response?.status ?? null,
      ok: false,
      bodySummary: summarizeJsonBody(createResult.responseJson),
      error: "video_operation_name_missing",
    };
  }
  const operationUrl = operationName.startsWith("http://") || operationName.startsWith("https://")
    ? operationName
    : `${context.apiBaseUrl.replace(/\/+$/, "")}/${operationName.replace(/^\/+/, "")}`;

  const deadline = Date.now() + context.videoPollTimeoutMs;
  const pollRecords = [];
  let finalPollJson = createResult.responseJson;
  while (Date.now() < deadline) {
    if (finalPollJson?.done === true) {
      break;
    }
    await sleep(DEFAULT_VIDEO_POLL_INTERVAL_MS);
    const pollResult = await sendGetJsonWithAuthAttempts(
      context,
      `video.poll-${String(pollRecords.length + 1).padStart(2, "0")}`,
      operationUrl,
      Math.min(context.timeoutMs, 30_000),
    );
    finalPollJson = pollResult.responseJson;
    pollRecords.push({
      status: pollResult.response?.status ?? null,
      ok: pollResult.ok,
      bodySummary: summarizeJsonBody(finalPollJson),
    });
    if (!pollResult.ok) {
      return {
        kind: "official_like_predict_long_running",
        requestUrl,
        requestBody,
        status: createResult.response?.status ?? null,
        ok: false,
        operationUrl,
        bodySummary: summarizeJsonBody(createResult.responseJson),
        pollRecords,
        error: "video_poll_failed",
      };
    }
  }
  const videoUri = extractVideoUriFromOperation(finalPollJson);
  let asset = null;
  if (videoUri) {
    const bytesResult = await sendGetBytesWithAuthAttempts(
      context,
      "video.asset",
      videoUri,
      Math.min(context.videoPollTimeoutMs, 120_000),
    );
    if (bytesResult.ok) {
      const contentType =
        normalizeString(bytesResult.response?.headers?.["content-type"]) ?? "video/mp4";
      const ext = mimeToExt(contentType);
      const assetPath = path.join(context.outDir, `video-asset${ext}`);
      await writeBuffer(assetPath, bytesResult.response.bytes);
      asset = {
        url: videoUri,
        contentType,
        bytesLength: bytesResult.response.bytes.length,
        assetPath,
      };
    }
  }
  return {
    kind: "official_like_predict_long_running",
    requestUrl,
    requestBody,
    status: createResult.response?.status ?? null,
    ok: finalPollJson?.done === true && !finalPollJson?.error,
    operationUrl,
    bodySummary: summarizeJsonBody(createResult.responseJson),
    finalPollSummary: summarizeJsonBody(finalPollJson),
    pollRecords,
    asset,
  };
}

async function probeMusic(context) {
  const browserStateWsUrl =
    normalizeRemoteMusicWsUrl(context.browserState?.musicWsUrl) ??
    normalizeRemoteMusicWsUrl(context.browserState?.canvasProgramInvokeContract?.musicWsUrl) ??
    null;
  const requestUrl =
    normalizeString(context.args?.["music-ws-url"]) ??
    browserStateWsUrl ??
    `${DEFAULT_MUSIC_WS_URL}?key=${encodeURIComponent(context.apiKey)}`;

  const requestBody = {
    setup: { model: `models/${context.model}` },
    client_content: {
      weightedPrompts: [{ text: context.prompt, weight: 1.0 }],
    },
    playback_control: "PLAY",
    note: "Browserless music probe uses only key query and does not send authuser query or durationSeconds music_generation_config.",
  };

  await writeJson(path.join(context.outDir, "music.ws.request.json"), {
    requestUrl,
    requestBody,
  });

  const result = await new Promise((resolve) => {
    const events = [];
    const ws = new WebSocket(requestUrl);
    const timeout = setTimeout(() => {
      resolve({
        ok: false,
        error: "music_ws_timeout",
        events,
      });
      try {
        ws.close();
      } catch {}
    }, Math.min(context.timeoutMs, 60_000));
    let setupComplete = false;
    let audioMimeType = "audio/l16;rate=48000;channels=2";
    const chunks = [];

    const finish = (value) => {
      clearTimeout(timeout);
      resolve({
        setupComplete,
        events,
        ...value,
      });
      try {
        ws.close();
      } catch {}
    };

    ws.addEventListener("open", () => {
      ws.send(JSON.stringify({ setup: requestBody.setup }));
    });

    ws.addEventListener("message", async (event) => {
      let text;
      if (typeof event.data === "string") {
        text = event.data;
      } else if (event.data && typeof event.data.text === "function") {
        text = await event.data.text();
      } else {
        text = String(event.data);
      }
      events.push(textPreview(text, 1200));

      let parsed = null;
      try {
        parsed = JSON.parse(text);
      } catch {
        parsed = null;
      }
      if (!setupComplete && (parsed?.setupComplete || parsed?.setup_complete)) {
        setupComplete = true;
        ws.send(JSON.stringify({ client_content: requestBody.client_content }));
        ws.send(JSON.stringify({ playback_control: requestBody.playback_control }));
      }

      const audioChunks =
        parsed?.serverContent?.audioChunks ?? parsed?.server_content?.audio_chunks ?? null;
      if (!Array.isArray(audioChunks) || !audioChunks.length) {
        return;
      }
      for (const chunk of audioChunks) {
        const raw = normalizeString(chunk?.data);
        if (!raw) {
          continue;
        }
        chunks.push(Buffer.from(raw, "base64"));
        audioMimeType =
          normalizeString(chunk?.mimeType) ??
          normalizeString(chunk?.mime_type) ??
          audioMimeType;
      }
      if (!chunks.length) {
        return;
      }
      const pcm = Buffer.concat(chunks);
      const rawExt = mimeToExt(audioMimeType) || ".bin";
      const rawPath = path.join(context.outDir, `music-audio${rawExt}`);
      await writeBuffer(rawPath, pcm);
      let wavPath = null;
      if (String(audioMimeType).toLowerCase().startsWith("audio/l16")) {
        const wavBytes = pcmAudioToWavBytes(pcm, audioMimeType);
        wavPath = path.join(context.outDir, "music-audio.wav");
        await writeBuffer(wavPath, wavBytes);
      }
      finish({
        ok: true,
        audioAsset: {
          mimeType: audioMimeType,
          bytesLength: pcm.length,
          rawPath,
          wavPath,
        },
        bodySummary: {
          audioChunkCount: audioChunks.length,
          audioBytes: pcm.length,
          mimeType: audioMimeType,
        },
      });
    });

    ws.addEventListener("error", (event) => {
      finish({
        ok: false,
        error: "music_ws_error",
        message: event?.message ?? String(event?.error ?? event),
      });
    });

    ws.addEventListener("close", (event) => {
      if (chunks.length) {
        return;
      }
      finish({
        ok: false,
        error: "music_ws_closed_without_audio",
        message: `close code=${event.code} reason=${event.reason || ""}`.trim(),
      });
    });
  });

  await writeJson(path.join(context.outDir, "music.ws.response.json"), result);
  return {
    kind: "official_music_ws_browserless",
    requestUrl,
    requestBody,
    status: result.ok ? 101 : null,
    ok: result.ok,
    bodySummary: result.bodySummary ?? null,
    audioAsset: result.audioAsset ?? null,
    error: result.error ?? null,
    responseText: result.message ?? null,
  };
}

function buildVideoInvokeUrl(apiBaseUrl, videoInvokePath, model) {
  const normalizedPath = normalizeString(videoInvokePath);
  if (!normalizedPath) {
    return `${apiBaseUrl.replace(/\/+$/, "")}/models/${model}:predictLongRunning`;
  }
  if (normalizedPath.startsWith("http://") || normalizedPath.startsWith("https://")) {
    return normalizedPath;
  }
  return `${apiBaseUrl.replace(/\/+$/, "")}${normalizedPath.startsWith("/") ? normalizedPath : `/${normalizedPath}`}`;
}

async function sendGetJsonWithAuthAttempts(context, prefix, requestUrl, timeoutMs) {
  const attempts = buildAuthAttempts(requestUrl, context.apiKey);
  const results = [];
  for (let index = 0; index < attempts.length; index += 1) {
    const attempt = attempts[index];
    const url =
      attempt.apiKeyPlacement === "query"
        ? appendApiKeyQueryIfMissing(requestUrl, context.apiKey)
        : requestUrl;
    const headers = buildJsonHeaders({
      session: context.session,
      pageOrigin: context.pageOrigin,
      pageReferer: context.pageReferer,
      targetOrigin: originFromUrl(url) ?? context.pageOrigin,
      locale: context.locale,
      apiKey: context.apiKey,
      authMode: attempt,
      includeBody: false,
    });
    headers.accept = "application/json";
    delete headers["content-type"];
    const requestRecord = {
      label: attempt.label,
      kind: attempt.kind,
      apiKeyPlacement: attempt.apiKeyPlacement,
      method: "GET",
      url,
      headers,
      headersRedacted: redactHeaders(headers),
    };
    const response = await fetch(url, {
      method: "GET",
      headers,
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });
    const responseRecord = await collectTextResponse(response);
    context.session = mergeSetCookie(context.session, responseRecord.setCookie);
    results.push({
      request: requestRecord,
      response: responseRecord,
    });
    await archiveAttempt(outDirFromContext(context), `${prefix}.attempt-${String(index + 1).padStart(2, "0")}`, requestRecord, responseRecord);
    if (responseRecord.ok) {
      return {
        ok: true,
        request: requestRecord,
        response: responseRecord,
        responseJson: tryParseJson(responseRecord.text),
        attempts: results,
      };
    }
  }
  const last = results[results.length - 1];
  return {
    ok: false,
    request: last?.request ?? null,
    response: last?.response ?? null,
    responseJson: tryParseJson(last?.response?.text),
    attempts: results,
  };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function buildSummaryBase(context) {
  return {
    ok: false,
    operation: context.operation,
    browserStatePath: context.browserStatePath,
    storageStatePath: context.storageStatePath,
    profileDir: context.profileDir,
    outDir: context.outDir,
    material: {
      shareId: context.browserState.shareId ?? null,
      shareUrl: context.browserState.shareUrl ?? null,
      canvasProgramUrl: context.browserState.canvasProgramUrl ?? null,
      appPath: context.browserState.appPath ?? null,
      conversationId: context.browserState.conversationId ?? null,
      responseId: context.browserState.responseId ?? null,
      invokeContract: context.browserState.canvasProgramInvokeContract ?? null,
      sourceProgramHandlePath: context.programHandlePath,
      googleApiKeyPresent: Boolean(context.apiKey),
    },
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");

  const browserStatePath = path.resolve(
    normalizeString(args["browser-state"]) ?? findLatestBrowserState(repoRoot) ?? "",
  );
  if (!fileExists(browserStatePath)) {
    throw new Error(
      "Missing --browser-state and no latest .runtime/gemini-canvas-program-runtime/*/browser-state.json was found.",
    );
  }

  const browserState = await readJson(browserStatePath);
  browserState.__path = browserStatePath;

  const programHandle = await resolveProgramHandle(browserState, repoRoot);
  const profileDir = resolveProfileDir(browserState, programHandle, repoRoot, args);
  const storageStatePath = resolveStorageStatePath(repoRoot, profileDir, args);
  const outDir = path.resolve(
    normalizeString(args["out-dir"]) ??
      path.join(
        repoRoot,
        "output",
        `gemini_canvas_program_browserless_invoke_${nowStamp()}`,
      ),
  );
  await ensureDir(outDir);
  copyFileSync(browserStatePath, path.join(outDir, "input.browser-state.json"));
  if (programHandle.path && fileExists(programHandle.path)) {
    copyFileSync(programHandle.path, path.join(outDir, "input.program-handle.json"));
  }

  const baseUrl = normalizeString(args["base-url"]) ?? inferBaseUrl(browserState);
  const apiBaseUrl = inferApiBaseUrl(browserState, args);
  const pageReferer =
    normalizeString(args["page-referer"]) ??
    normalizeString(browserState.canvasProgramUrl) ??
    normalizeString(browserState.pageUrl) ??
    normalizeString(browserState.shareUrl) ??
    `${baseUrl.replace(/\/+$/, "")}/canvas`;
  const pageOrigin = originFromUrl(pageReferer) ?? originFromUrl(baseUrl) ?? DEFAULT_BASE_URL;
  const locale =
    normalizeString(args.locale) ??
    normalizeString(programHandle?.json?.bootstrap?.language) ??
    DEFAULT_LOCALE;
  const materialOperation = normalizeOperation(
    normalizeString(browserState?.canvasProgramInvokeContract?.operation),
  );
  const operation = normalizeOperation(
    normalizeString(args.operation) ??
      materialOperation ??
      "text",
  );
  const materialPrompt =
    materialOperation === operation
      ? normalizeString(browserState?.canvasProgramInvokeContract?.prompt)
      : null;
  const prompt =
    normalizeString(args.prompt) ??
    materialPrompt ??
    defaultPromptForOperation(operation);
  const model = normalizeString(args.model) ?? defaultModelForOperation(operation);
  const voiceName = normalizeString(args.voice);
  const aspectRatio =
    normalizeString(args["aspect-ratio"]) ??
    normalizeString(browserState?.canvasProgramInvokeContract?.aspectRatio) ??
    normalizeString(browserState?.canvasProgramInvokeContract?.aspect_ratio) ??
    (operation === "video-create" ? "16:9" : "1:1");
  const durationSeconds =
    Number.parseFloat(
      args["duration-seconds"] ??
        browserState?.canvasProgramInvokeContract?.durationSeconds ??
        browserState?.canvasProgramInvokeContract?.duration_seconds ??
        (operation === "video-create" ? "4" : ""),
    ) || null;
  const apiKey =
    normalizeString(args["api-key"]) ??
    normalizeString(browserState.googleApiKey) ??
    (Array.isArray(browserState.apiKeys)
      ? normalizeString(browserState.apiKeys.find((value) => normalizeString(value)))
      : null);
  const timeoutMs = Math.max(Number(args["timeout-ms"] || DEFAULT_TIMEOUT_MS), 5_000);
  const videoPollTimeoutMs = Math.max(
    Number(args["video-poll-timeout-ms"] || DEFAULT_VIDEO_POLL_TIMEOUT_MS),
    timeoutMs,
  );

  let summary = {
    ...buildSummaryBase({
      operation,
      browserStatePath,
      storageStatePath,
      profileDir,
      outDir,
      browserState,
      programHandlePath: programHandle.path,
      apiKey,
    }),
  };

  await writeJson(path.join(outDir, "resolved-material.json"), {
    browserStatePath,
    programHandlePath: programHandle.path,
    profileDir,
    storageStatePath,
    baseUrl,
    apiBaseUrl,
    pageOrigin,
    pageReferer,
    locale,
    operation,
    model,
    prompt,
    voiceName,
    aspectRatio,
    durationSeconds,
    googleApiKeyPresent: Boolean(apiKey),
  });

  if (!storageStatePath || !fileExists(storageStatePath)) {
    summary = {
      ...summary,
      error: "missing_storage_state",
      message:
        "No exported storage-state.json could be resolved from the materialized browser-state/profile. Provide --storage-state or export browser storage state first.",
    };
    await writeJson(path.join(outDir, "summary.json"), summary);
    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
    process.exitCode = 1;
    return;
  }

  const storageState = await readJson(storageStatePath);
  let session = buildPureHttpSession(storageState, pageReferer, baseUrl, args["auth-user"] ?? DEFAULT_AUTH_USER);

  const context = {
    args,
    repoRoot,
    outDir,
    browserStatePath,
    browserState,
    programHandlePath: programHandle.path,
    programHandle: programHandle.json,
    profileDir,
    storageStatePath,
    storageState,
    baseUrl,
    apiBaseUrl,
    pageOrigin,
    pageReferer,
    locale,
    operation,
    model,
    prompt,
    voiceName,
    aspectRatio,
    durationSeconds,
    timeoutMs,
    videoPollTimeoutMs,
    apiKey,
    session,
  };

  let result;
  switch (operation) {
    case "text":
      result = await probeText(context);
      break;
    case "tts":
      result = await probeTts(context);
      break;
    case "image":
      result = await probeImage(context);
      break;
    case "music":
      result = await probeMusic(context);
      break;
    case "video-create":
      result = await probeVideoCreate(context);
      break;
    default:
      summary = {
        ...summary,
        error: "unsupported_operation",
        message: `Unsupported operation '${operation}'. Expected text, tts, image, music, or video-create.`,
      };
      await writeJson(path.join(outDir, "summary.json"), summary);
      process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
      process.exitCode = 1;
      return;
  }

  const finalSummary = {
    ...summary,
    ok: Boolean(result?.ok),
    executionPath: result?.kind ?? null,
    requestUrl: result?.requestUrl ?? null,
    requestBody: result?.requestBody ?? null,
    status: result?.status ?? null,
    responseText: result?.responseText ?? null,
    bodySummary: result?.bodySummary ?? null,
    finalPollSummary: result?.finalPollSummary ?? null,
    pollRecords: result?.pollRecords ?? null,
    audioAsset: result?.audioAsset ?? null,
    imageAsset: result?.imageAsset ?? null,
    asset: result?.asset ?? null,
    error: result?.error ?? null,
    notes: [
      "This script is browserless only after material resolution. It does not invoke connected client, browser pool, or ws://127.0.0.1:9998.",
      "If the materialized browser-state lacks enough HTTP contract or no exported storage-state is available, the probe reports unsupported_by_material instead of silently falling back.",
    ],
  };
  await writeJson(path.join(outDir, "summary.json"), finalSummary);
  process.stdout.write(`${JSON.stringify(finalSummary, null, 2)}\n`);
  if (!finalSummary.ok) {
    process.exitCode = 1;
  }
}

main().catch(async (error) => {
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");
  const fallbackDir = path.join(
    repoRoot,
    ".runtime",
    "gemini-canvas-program-browserless-invoke",
    `failed-${nowStamp()}`,
  );
  await ensureDir(fallbackDir);
  const summary = {
    ok: false,
    error: "probe_crashed",
    message: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack : null,
    outDir: fallbackDir,
  };
  await writeJson(path.join(fallbackDir, "summary.json"), summary);
  process.stderr.write(`${summary.message}\n`);
  process.exitCode = 1;
});
