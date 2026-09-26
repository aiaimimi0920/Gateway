#!/usr/bin/env node

import { createBrowserlessAuthOwner } from "./gemini-canvas-browserless-auth.mjs";
import { createBrowserlessRequestOwner } from "./gemini-canvas-browserless-requests.mjs";
import {
  responseHeadersToObject,
  getSetCookieValues,
  collectTextResponse,
  collectBytesResponse,
  tryParseJson,
  textPreview,
  redactHeaders,
} from "./gemini-canvas-browserless-response.mjs";
import { createBrowserlessPayloadOwner } from "./gemini-canvas-browserless-payload.mjs";
import { createBrowserlessGenerateContentOperations } from "./gemini-canvas-browserless-generate-content.mjs";
import { createBrowserlessVideoOperation } from "./gemini-canvas-browserless-video.mjs";
import { createBrowserlessMusicOperation } from "./gemini-canvas-browserless-music.mjs";
import { createBrowserlessMaterialOwner } from "./gemini-canvas-browserless-material.mjs";
import { createBrowserlessInvocationOptions } from "./gemini-canvas-browserless-options.mjs";
import fs from "node:fs/promises";
import { copyFileSync } from "node:fs";
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

const {
  fileExists,
  pathStatMtime,
  findLatestBrowserState,
  uniqueStrings,
  resolveProgramHandlePath,
  resolveProgramHandle,
  resolveProfileDir,
  walkFiles,
  resolveStorageStatePath,
} = createBrowserlessMaterialOwner({ normalizeString, readJson });

const {
  normalizeOperation,
  defaultPromptForOperation,
  defaultModelForOperation,
  inferBaseUrl,
  inferApiBaseUrl,
  originFromUrl,
  resolveInvocationOptions,
} = createBrowserlessInvocationOptions({
  normalizeString,
  DEFAULT_BASE_URL,
  DEFAULT_API_BASE_URL,
  DEFAULT_TEXT_MODEL,
  DEFAULT_TTS_MODEL,
  DEFAULT_IMAGE_MODEL,
  DEFAULT_MUSIC_MODEL,
  DEFAULT_VIDEO_MODEL,
  DEFAULT_LOCALE,
  DEFAULT_TIMEOUT_MS,
  DEFAULT_VIDEO_POLL_TIMEOUT_MS,
});

const {
  cookieMatchesUrl,
  buildPureHttpSession,
  mergeSetCookie,
  buildSapisidAuthorization,
  buildBrowserClientHints,
  buildJsonHeaders,
  buildJsonHeadersWithOptions,
  buildFormHeaders,
  appendApiKeyQueryIfMissing,
  buildAuthAttempts,
} = createBrowserlessAuthOwner({
  DEFAULT_AUTH_USER,
  DEFAULT_BROWSERLESS_USER_AGENT,
  normalizeString,
});

const {
  mimeToExt,
  buildTextRequestBody,
  buildTtsRequestBody,
  buildImageRequestBody,
  buildVideoCreateRequestBody,
  extractTextFromGenerateContentResponse,
  extractAudioFromGenerateContentResponse,
  extractInlineImageFromGenerateContentResponse,
  extractImagenImages,
  parsePcmSampleRate,
  parsePcmChannels,
  pcmAudioToWavBytes,
  extractVideoUriFromOperation,
  summarizeJsonBody,
} = createBrowserlessPayloadOwner({
  normalizeString,
});

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

const {
  sendExactMinimalApiKeyOnlyJson,
  sendJsonWithAuthAttempts,
  sendJsonWithCustomAttempts,
  sendGetBytesWithAuthAttempts,
  sendFormRequest,
  sendGetJsonWithAuthAttempts,
} = createBrowserlessRequestOwner({
  DEFAULT_BROWSERLESS_USER_AGENT,
  appendApiKeyQueryIfMissing,
  buildAuthAttempts,
  buildJsonHeaders,
  buildJsonHeadersWithOptions,
  buildFormHeaders,
  mergeSetCookie,
  originFromUrl,
  archiveAttempt,
  outDirFromContext,
  extractXsrfToken,
  normalizeString,
});

function outDirFromContext(context) {
  return context.outDir;
}

const {
  probeText,
  probeTts,
  probeImage,
} = createBrowserlessGenerateContentOperations({
  normalizeString,
  writeBuffer,
  buildTextRequestBody,
  buildTtsRequestBody,
  buildImageRequestBody,
  extractTextFromGenerateContentResponse,
  extractAudioFromGenerateContentResponse,
  extractInlineImageFromGenerateContentResponse,
  extractImagenImages,
  mimeToExt,
  pcmAudioToWavBytes,
  summarizeJsonBody,
  sendJsonWithAuthAttempts,
  sendExactMinimalApiKeyOnlyJson,
  sendJsonWithCustomAttempts,
  looksLikeQuotaOrPlanGate,
});

const {
  probeVideoCreate,
  buildVideoInvokeUrl,
} = createBrowserlessVideoOperation({
  DEFAULT_VIDEO_POLL_INTERVAL_MS,
  normalizeString,
  writeBuffer,
  buildVideoCreateRequestBody,
  extractVideoUriFromOperation,
  mimeToExt,
  summarizeJsonBody,
  sendJsonWithAuthAttempts,
  sendExactMinimalApiKeyOnlyJson,
  sendGetJsonWithAuthAttempts,
  sendGetBytesWithAuthAttempts,
  looksLikeQuotaOrPlanGate,
});

const {
  probeMusic,
  normalizeRemoteMusicWsUrl,
} = createBrowserlessMusicOperation({
  DEFAULT_MUSIC_WS_URL,
  normalizeString,
  writeJson,
  writeBuffer,
  mimeToExt,
  pcmAudioToWavBytes,
  textPreview,
});

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

  const {
    baseUrl,
    apiBaseUrl,
    pageReferer,
    pageOrigin,
    locale,
    operation,
    prompt,
    model,
    voiceName,
    aspectRatio,
    durationSeconds,
    apiKey,
    timeoutMs,
    videoPollTimeoutMs,
  } = resolveInvocationOptions(browserState, programHandle, args);

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
