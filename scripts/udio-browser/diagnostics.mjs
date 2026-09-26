import { appendFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { normalizeString } from "./request.mjs";

const NUMBER_FIELDS = ["timeoutMs", "waitTimeoutMs", "pollIntervalMs", "manualChallengeWaitMs",
  "cdpConnectTimeoutMs", "contextPageCount", "cookieCount", "tokenLength", "promptLength",
  "textLength", "songCount", "remainingMs", "status"];
const BOOLEAN_FIELDS = ["hasBrowserCdpUrl", "hasRuntimeStateObjectKey", "hasCookieHeader",
  "headless", "hasStoredState", "hasAuthToken", "required", "hasCaptchaToken", "ok",
  "transportError", "challengeOutcome", "completed"];
const ENUM_FIELDS = {
  targetAssetKind: ["audio", "image", "video"],
  source: ["pre_submit", "submit_retry"],
  mode: ["current_page", "challenge_surface"],
};

function diagnosticMetadata(details) {
  const result = {};
  if (!details || typeof details !== "object") return result;
  // Never serialize arbitrary error text, URL credentials, provider data or getters.
  const value = (key) => Object.getOwnPropertyDescriptor(details, key)?.value;
  for (const key of NUMBER_FIELDS) {
    const number = value(key);
    if (typeof number === "number" && Number.isFinite(number) && number >= 0 && number <= Number.MAX_SAFE_INTEGER) result[key] = number;
  }
  for (const key of BOOLEAN_FIELDS) {
    if (typeof value(key) === "boolean") result[key] = value(key);
  }
  for (const [key, choices] of Object.entries(ENUM_FIELDS)) {
    if (choices.includes(value(key))) result[key] = value(key);
  }
  const trackIds = value("trackIds");
  if (Array.isArray(trackIds)) result.trackCount = trackIds.length;
  return result;
}

export async function debugLog(logPath, stage, details = {}) {
  const normalizedPath = normalizeString(logPath);
  if (!normalizedPath) {
    return;
  }
  try {
    const entry = {
      ts: new Date().toISOString(),
      stage: typeof stage === "string" && /^[a-z_]{1,80}$/.test(stage) ? stage : "unknown",
      details: diagnosticMetadata(details),
    };
    await mkdir(path.dirname(normalizedPath), { recursive: true });
    await appendFile(normalizedPath, `${JSON.stringify(entry)}\n`, { encoding: "utf8", mode: 0o600 });
  } catch {
    // Optional diagnostics must not alter worker success, failure or cleanup.
  }
}
