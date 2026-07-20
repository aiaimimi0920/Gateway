#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveGeminiCanvasManualLiveVendorStorageStatePath } from "./gemini-canvas-runtime-paths.mjs";

const GEMINI_CANVAS_BROWSER_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";
const GEMINI_CANVAS_BROWSER_SEC_CH_UA =
  "\"Not.A/Brand\";v=\"8\", \"Chromium\";v=\"147\", \"Google Chrome\";v=\"147\"";
const GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION = "\"147.0.7727.102\"";
const GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION_LIST =
  "\"Not.A/Brand\";v=\"8.0.0.0\", \"Chromium\";v=\"147.0.7727.102\", \"Google Chrome\";v=\"147.0.7727.102\"";
const GEMINI_CANVAS_BROWSER_SEC_CH_UA_PLATFORM_VERSION = "\"19.0.0\"";
const GEMINI_CANVAS_BROWSER_CHANNEL = "stable";
const GEMINI_CANVAS_BROWSER_COPYRIGHT = "Copyright 2026 Google LLC. All Rights reserved.";
const GEMINI_CANVAS_BROWSER_VALIDATION = "B2gM+WTW2xHE15IAjh8nDoMc5x0=";
const GEMINI_CANVAS_BROWSER_YEAR = "2026";
const GEMINI_WEB_MODEL_HEADER_KEY = "x-goog-ext-525001261-jspb";
const GEMINI_WEB_MODEL_HEADER_2_KEY = "x-goog-ext-73010989-jspb";
const GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER =
  "[1,null,null,null,\"fbb127bbb056c959\",null,null,null,[4],null,null,null,null,null,1]";
const GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2 = "[0]";

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const entry = argv[i];
    if (!entry.startsWith("--")) {
      continue;
    }
    const key = entry.slice(2);
    const next = argv[i + 1];
    if (!next || next.startsWith("--")) {
      args[key] = true;
      continue;
    }
    args[key] = next;
    i += 1;
  }
  return args;
}

function nowStamp() {
  return new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
}

async function readJson(filePath) {
  const raw = (await fs.readFile(filePath, "utf8")).replace(/^\uFEFF/, "");
  return JSON.parse(raw);
}

async function writeJson(filePath, value) {
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

async function writeText(filePath, value) {
  await fs.writeFile(filePath, value, "utf8");
}

function normalizeString(value) {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function domainMatches(cookieDomain, hostname) {
  const domain = String(cookieDomain || "").replace(/^\./, "").trim().toLowerCase();
  const host = String(hostname || "").trim().toLowerCase();
  if (!domain || !host) {
    return false;
  }
  return host === domain || host.endsWith(`.${domain}`);
}

function pathMatches(cookiePath, requestPath) {
  const cookiePathNormalized = normalizeString(cookiePath) ?? "/";
  const requestPathNormalized = normalizeString(requestPath) ?? "/";
  return requestPathNormalized.startsWith(cookiePathNormalized);
}

function buildSessionFromStorageState(storageState, targetUrl) {
  const parsed = new URL(targetUrl);
  const cookies = Array.isArray(storageState.cookies) ? storageState.cookies : [];
  const matched = cookies.filter(
    (cookie) =>
      domainMatches(cookie.domain, parsed.hostname) &&
      pathMatches(cookie.path, parsed.pathname),
  );
  if (!matched.length) {
    throw new Error("no Google cookies matched the StreamGenerate request URL");
  }
  const cookieHeader = matched
    .map((cookie) => `${cookie.name}=${cookie.value}`)
    .join("; ");
  const sapisidCookie =
    matched.find((cookie) => cookie.name === "SAPISID") ??
    cookies.find(
      (cookie) =>
        domainMatches(cookie.domain, parsed.hostname) && cookie.name === "SAPISID",
    );
  if (!sapisidCookie?.value) {
    throw new Error("missing SAPISID cookie");
  }
  return {
    cookieHeader,
    sapisid: sapisidCookie.value,
  };
}

function buildSapisidAuthorization(sapisid, origin, timestampSecs = null) {
  const ts = Number.isFinite(timestampSecs) ? Number(timestampSecs) : Math.floor(Date.now() / 1000);
  const digest = crypto
    .createHash("sha1")
    .update(`${ts} ${String(sapisid).trim()} ${String(origin).trim()}`)
    .digest("hex");
  return `SAPISIDHASH ${ts}_${digest} SAPISID1PHASH ${ts}_${digest} SAPISID3PHASH ${ts}_${digest}`;
}

function findLastRpcCapture(captureJson, label, type) {
  const result = captureJson.result || captureJson;
  const captures = Array.isArray(result.rpcCaptures) ? result.rpcCaptures : [];
  return (
    [...captures].reverse().find(
      (entry) => entry?.label === label && entry?.type === type,
    ) ?? null
  );
}

function listRpcCaptures(captureJson, type = null) {
  const result = captureJson.result || captureJson;
  const captures = Array.isArray(result.rpcCaptures) ? result.rpcCaptures : [];
  if (!type) {
    return captures;
  }
  return captures.filter((entry) => entry?.type === type);
}

function parseCookieHeader(cookieHeader) {
  const jar = new Map();
  for (const segment of String(cookieHeader || "").split(/;\s*/)) {
    if (!segment || !segment.includes("=")) {
      continue;
    }
    const index = segment.indexOf("=");
    const name = segment.slice(0, index).trim();
    const value = segment.slice(index + 1).trim();
    if (!name) {
      continue;
    }
    jar.set(name, value);
  }
  return jar;
}

function serializeCookieJar(jar) {
  return [...jar.entries()].map(([name, value]) => `${name}=${value}`).join("; ");
}

function collectSetCookieValues(headers) {
  if (headers && typeof headers.getSetCookie === "function") {
    return headers.getSetCookie();
  }
  const combined = headers?.get?.("set-cookie");
  if (!combined) {
    return [];
  }
  return String(combined)
    .split(/,(?=[^;,\r\n]+=)/)
    .map((value) => value.trim())
    .filter(Boolean);
}

function applySetCookieValues(cookieJar, setCookieValues) {
  for (const entry of setCookieValues || []) {
    const normalized = normalizeString(entry);
    if (!normalized || !normalized.includes("=")) {
      continue;
    }
    const [pair] = normalized.split(";");
    const index = pair.indexOf("=");
    if (index <= 0) {
      continue;
    }
    const name = pair.slice(0, index).trim();
    const value = pair.slice(index + 1).trim();
    if (!name) {
      continue;
    }
    cookieJar.set(name, value);
  }
}

function buildRequestHeaders(session, origin, referer, authUser = "0") {
  const headers = {
    accept: "*/*",
    "accept-language": "zh-CN,zh;q=0.9,en;q=0.8",
    authorization: buildSapisidAuthorization(session.sapisid, origin),
    "content-type": "application/x-www-form-urlencoded;charset=UTF-8",
    cookie: session.cookieHeader,
    origin,
    referer,
    "sec-ch-ua": "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not_A Brand\";v=\"24\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"Windows\"",
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "cors",
    "sec-fetch-site": "same-origin",
    "user-agent":
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
    "x-goog-authuser": String(authUser || "0"),
    "x-origin": origin,
    "x-same-domain": "1",
  };
  return headers;
}

function buildPreludeBatchedHeaders(session, referer, acceptLanguage = "zh-CN") {
  return {
    accept: "*/*",
    "accept-language": acceptLanguage,
    cookie: session.cookieHeader,
    "content-type": "application/x-www-form-urlencoded;charset=UTF-8",
    priority: "u=1, i",
    referer,
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "cors",
    "sec-fetch-site": "same-origin",
    "sec-ch-ua": GEMINI_CANVAS_BROWSER_SEC_CH_UA,
    "sec-ch-ua-full-version": GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION,
    "sec-ch-ua-full-version-list": GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION_LIST,
    "sec-ch-ua-platform": "\"Windows\"",
    "sec-ch-ua-platform-version": GEMINI_CANVAS_BROWSER_SEC_CH_UA_PLATFORM_VERSION,
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-arch": "\"x86\"",
    "sec-ch-ua-bitness": "\"64\"",
    "sec-ch-ua-form-factors": "\"Desktop\"",
    "sec-ch-ua-model": "\"\"",
    "sec-ch-ua-wow64": "?0",
    "user-agent": GEMINI_CANVAS_BROWSER_USER_AGENT,
    "x-browser-channel": GEMINI_CANVAS_BROWSER_CHANNEL,
    "x-browser-copyright": GEMINI_CANVAS_BROWSER_COPYRIGHT,
    "x-browser-validation": GEMINI_CANVAS_BROWSER_VALIDATION,
    "x-browser-year": GEMINI_CANVAS_BROWSER_YEAR,
    "x-goog-ext-525001261-jspb": GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER,
    "x-goog-ext-73010989-jspb": GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2,
    "x-same-domain": "1",
  };
}

function selectPreludeRequests(captureJson, streamRequest, requestedLabels) {
  const labelSet = new Set(
    (requestedLabels || [])
      .map((value) => normalizeString(value))
      .filter(Boolean),
  );
  const requests = listRpcCaptures(captureJson, "request");
  const streamIndex = requests.findIndex(
    (entry) =>
      entry?.label === streamRequest?.label &&
      entry?.url === streamRequest?.url &&
      entry?.bodyText === streamRequest?.bodyText,
  );
  const scopedRequests = streamIndex >= 0 ? requests.slice(0, streamIndex) : requests;
  return scopedRequests.filter((entry) => labelSet.has(entry?.label));
}

async function replaySingleRequest(requestCapture, session, defaultOrigin, defaultReferer, authUser, timeoutMs) {
  const referer = normalizeString(requestCapture?.headers?.referer) ?? defaultReferer;
  const isBatchedPrelude = String(requestCapture?.url || "").includes("/data/batchexecute?");
  const headers = isBatchedPrelude
    ? buildPreludeBatchedHeaders(session, referer)
    : buildRequestHeaders(session, defaultOrigin, referer, authUser);
  const response = await fetch(requestCapture.url, {
    method: requestCapture.method || "POST",
    headers,
    body: requestCapture.bodyText,
    redirect: "manual",
    signal: AbortSignal.timeout(timeoutMs),
  });
  const bodyText = await response.text();
  const setCookieValues = collectSetCookieValues(response.headers);
  return {
    label: requestCapture.label ?? null,
    rpcId: requestCapture.rpcId ?? null,
    sourcePath: requestCapture.sourcePath ?? null,
    requestUrl: requestCapture.url,
    referer,
    status: response.status,
    statusText: response.statusText,
    finalUrl: response.url,
    setCookieCount: setCookieValues.length,
    bodyPreview: bodyText.slice(0, 320),
    bodyLength: bodyText.length,
    setCookieValues,
  };
}

function timeout(ms, label) {
  return new Promise((_, reject) => {
    setTimeout(() => reject(new Error(label)), ms);
  });
}

function bodyContainsMusicAcceptedProgress(text) {
  const raw = String(text || "");
  const normalized = raw.toLowerCase();
  return (
    (raw.includes("music_generation") && raw.includes("action_input")) ||
    normalized.includes("track details") ||
    normalized.includes("generating your music") ||
    normalized.includes("i've put together a 30-second electronic cue") ||
    normalized.includes("i’ve put together a 30-second electronic cue") ||
    normalized.includes("electronic cue for you") ||
    raw.includes('"11":["Electronic Music Cue Generation') ||
    raw.includes('\\"11\\":[\\"Electronic Music Cue Generation') ||
    ((raw.includes('"26":"') || raw.includes('\\"26\\":\\"')) &&
      (raw.includes('"44":true') || raw.includes('\\"44\\":true')))
  );
}

async function collectStreamBody(response, timeoutMs) {
  if (!response.body) {
    return { chunkCount: 0, bodyText: "", endReason: "missing_body" };
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let chunkCount = 0;
  let bodyText = "";
  let endReason = "done";
  try {
    while (true) {
      const readResult = await Promise.race([
        reader.read(),
        timeout(Math.min(timeoutMs, 20000), "stream_idle_timeout"),
      ]);
      if (readResult.done) {
        endReason = "done";
        break;
      }
      chunkCount += 1;
      bodyText += decoder.decode(readResult.value, { stream: true });
      if (bodyText.includes("video_placeholder")) {
        endReason = "video_placeholder";
      }
      if (
        bodyText.includes(
          "I couldn't do that because I'm getting a lot of requests right now. Please try again later.",
        )
      ) {
        endReason = "busy_message";
        break;
      }
      if (chunkCount >= 50) {
        endReason = "chunk_cap";
        break;
      }
    }
  } catch (error) {
    endReason = error instanceof Error ? error.message : String(error);
  }
  try {
    await reader.cancel();
  } catch {
    // ignore
  }
  bodyText += decoder.decode();
  return {
    chunkCount,
    bodyText,
    endReason,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");
  const runtimePath = path.resolve(
    repoRoot,
    args.runtime ||
      resolveGeminiCanvasManualLiveVendorStorageStatePath(),
  );
  const capturePath = path.resolve(
    repoRoot,
    args.capture || "output/tmp-video-streamgenerate-current.json",
  );
  const outDir = path.resolve(
    repoRoot,
    args.outDir ||
      path.join(
        "output",
        `gemini_canvas_video_streamgenerate_exact_replay_${nowStamp()}`,
      ),
  );
  await fs.mkdir(outDir, { recursive: true });

  const storageState = await readJson(runtimePath);
  const captureJson = await readJson(capturePath);
  const streamRequest = findLastRpcCapture(captureJson, "StreamGenerate", "request");
  if (!streamRequest?.url || !streamRequest?.bodyText) {
    throw new Error("capture JSON does not contain a usable StreamGenerate request");
  }
  const result = captureJson.result || captureJson;
  const origin = "https://gemini.google.com";
  const referer =
    normalizeString(args.referer) ??
    normalizeString(result.finalUrl) ??
    `${origin}/app`;
  const authUser = normalizeString(args.authUser) ?? "0";
  const session = buildSessionFromStorageState(storageState, streamRequest.url);
  const cookieJar = parseCookieHeader(session.cookieHeader);
  const requestedPreludeLabels = String(args.preludeLabels || "")
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
  const preludeRequests =
    requestedPreludeLabels.length > 0
      ? selectPreludeRequests(captureJson, streamRequest, requestedPreludeLabels)
      : [];
  const preludeResults = [];
  for (const requestCapture of preludeRequests) {
    const currentSession = {
      ...session,
      cookieHeader: serializeCookieJar(cookieJar),
    };
    const preludeResult = await replaySingleRequest(
      requestCapture,
      currentSession,
      origin,
      referer,
      authUser,
      Math.max(15000, Number(args.timeoutMs || 45000)),
    );
    applySetCookieValues(cookieJar, preludeResult.setCookieValues);
    preludeResults.push({
      ...preludeResult,
      setCookieValues: undefined,
    });
  }
  const headers = buildRequestHeaders(
    {
      ...session,
      cookieHeader: serializeCookieJar(cookieJar),
    },
    origin,
    referer,
    authUser,
  );

  const response = await fetch(streamRequest.url, {
    method: "POST",
    headers,
    body: streamRequest.bodyText,
    redirect: "manual",
    signal: AbortSignal.timeout(Math.max(30000, Number(args.timeoutMs || 45000))),
  });
  const collected = await collectStreamBody(
    response,
    Math.max(30000, Number(args.timeoutMs || 45000)),
  );

  const summary = {
    ok: response.ok,
    status: response.status,
    statusText: response.statusText,
    finalUrl: response.url,
    requestUrl: streamRequest.url,
    referer,
    authUser,
    preludeCount: preludeResults.length,
    preludeResults,
    chunkCount: collected.chunkCount,
    bodyLength: collected.bodyText.length,
    endReason: collected.endReason,
    containsVideoPlaceholder: collected.bodyText.includes("video_placeholder"),
    containsBusyMessage: collected.bodyText.includes(
      "I couldn't do that because I'm getting a lot of requests right now. Please try again later.",
    ),
    containsMusicAcceptedProgress: bodyContainsMusicAcceptedProgress(
      collected.bodyText,
    ),
    containsMusicResponseIdToken18:
      collected.bodyText.includes('"18":"r_') ||
      collected.bodyText.includes('\\"18\\":\\"r_'),
    containsMusicOpaqueToken21:
      collected.bodyText.includes('"21":[') ||
      collected.bodyText.includes('\\"21\\":['),
    containsMusicOpaqueToken26:
      collected.bodyText.includes('"26":"') ||
      collected.bodyText.includes('\\"26\\":\\"'),
    responseHeaders: Object.fromEntries(response.headers.entries()),
  };

  await writeJson(path.join(outDir, "summary.json"), summary);
  await writeJson(path.join(outDir, "request.json"), {
    url: streamRequest.url,
    headers,
    bodyText: streamRequest.bodyText,
  });
  await writeText(path.join(outDir, "response.txt"), collected.bodyText);

  console.log(JSON.stringify(summary, null, 2));
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(message);
  process.exit(1);
});
