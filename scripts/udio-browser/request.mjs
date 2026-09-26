export const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
export const DEFAULT_WAIT_TIMEOUT_MS = 4 * 60 * 1000;
export const DEFAULT_POLL_INTERVAL_MS = 3_000;
export const DEFAULT_CHALLENGE_RETRY_INTERVAL_MS = 5_000;
export const DEFAULT_LOCALE = "en-US";
export const DEFAULT_NAVIGATION_TIMEOUT_MS = 45_000;
export const DEFAULT_CDP_CONNECT_TIMEOUT_MS = 300_000;

export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function normalizeBaseUrl(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "udio_browser_missing_base_url",
    });
  }
  return normalized.replace(/\/+$/, "");
}

export function normalizeLocale(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    return null;
  }
  return normalized.split(",")[0]?.trim() || null;
}

export function normalizeTimeoutMs(value, fallback, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(Math.max(Math.trunc(parsed), min), max);
}

export function normalizeTargetAssetKind(value) {
  const normalized = normalizeString(String(value ?? ""))?.toLowerCase();
  if (!normalized) {
    return "audio";
  }
  if (["image", "audio", "video"].includes(normalized)) {
    return normalized;
  }
  throw Object.assign(new Error("targetAssetKind must be image, audio, or video."), {
    status: 400,
    code: "udio_browser_invalid_target_asset_kind",
  });
}

export function parseBoolean(value, fallback) {
  if (typeof value === "boolean") {
    return value;
  }
  const normalized = normalizeString(String(value ?? ""))?.toLowerCase();
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

export function normalizePathname(value) {
  const normalized = String(value || "/").trim();
  if (!normalized) {
    return "/";
  }
  return normalized.replace(/\/+$/, "") || "/";
}

export function validateInput(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw Object.assign(new Error("Expected a JSON object on stdin."), {
      status: 400,
      code: "udio_browser_invalid_input",
    });
  }
  if (!normalizeString(input.baseUrl)) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "udio_browser_missing_base_url",
    });
  }
  if (!input.requestBody || typeof input.requestBody !== "object" || Array.isArray(input.requestBody)) {
    throw Object.assign(new Error("requestBody must be a JSON object."), {
      status: 400,
      code: "udio_browser_missing_request_body",
    });
  }
  normalizeTargetAssetKind(input.targetAssetKind);
  const browserCdpUrl = normalizeString(
    input.browserCdpUrl ?? process.env.UDIO_BROWSER_CDP_URL ?? null,
  );
  if (
    !normalizeString(input.cookieHeader) &&
    !normalizeString(input.runtimeStateObjectKey) &&
    !browserCdpUrl
  ) {
    throw Object.assign(
      new Error("Udio browser worker requires cookieHeader, runtimeStateObjectKey, or browserCdpUrl."),
      {
        status: 400,
        code: "udio_browser_missing_runtime_auth",
      },
    );
  }
}

export function deepClone(value) {
  return JSON.parse(JSON.stringify(value ?? {}));
}
