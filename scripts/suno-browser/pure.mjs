export function validateInput(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw createError(400, "suno_browser_invalid_input", "Suno browser worker input must be an object.");
  }
}

export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function normalizeBaseUrl(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw createError(400, "suno_browser_missing_base_url", "baseUrl is required.");
  }
  return normalized.replace(/\/+$/, "");
}

export function normalizeLocale(value) {
  const normalized = normalizeString(value);
  return normalized ? normalized.split(",")[0]?.trim() || null : null;
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
  throw createError(
    400,
    "suno_browser_invalid_target_asset_kind",
    "targetAssetKind must be image, audio, or video.",
  );
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

export function extractPrompt(requestBody) {
  if (!requestBody || typeof requestBody !== "object") {
    return null;
  }
  const direct =
    normalizeString(requestBody.prompt) ??
    normalizeString(requestBody.input) ??
    normalizeString(requestBody.lyrics);
  if (direct) {
    return direct;
  }
  if (Array.isArray(requestBody.parts)) {
    for (const part of requestBody.parts) {
      const value = normalizeString(part?.content) ?? normalizeString(part?.text);
      if (value) {
        return value;
      }
    }
  }
  return null;
}

export function readClips(body) {
  return Array.isArray(body?.clips) ? body.clips : [];
}

export function collectClipIds(clips) {
  return clips
    .map((clip) => normalizeString(clip?.id))
    .filter(Boolean);
}

function clipHasTargetAsset(clip, targetAssetKind) {
  if (!clip || typeof clip !== "object") {
    return false;
  }
  const field =
    targetAssetKind === "image"
      ? "image_url"
      : targetAssetKind === "video"
        ? "video_url"
        : "audio_url";
  return Boolean(normalizeString(clip[field]) ?? normalizeString(clip[toCamel(field)]));
}

function clipTerminalStatus(clip) {
  const status = normalizeString(clip?.status)?.toLowerCase();
  return Boolean(status && ["complete", "completed", "error", "failed"].includes(status));
}

function clipCompletedStatus(clip) {
  const status = normalizeString(clip?.status)?.toLowerCase();
  return status === "complete" || status === "completed";
}

export function clipsTerminal(clips) {
  return Array.isArray(clips) && clips.length > 0 && clips.every((clip) => clipTerminalStatus(clip));
}

export function clipsReadyForTarget(clips, targetAssetKind) {
  return (
    Array.isArray(clips) &&
    clips.length > 0 &&
    clips.every(
      (clip) => clipCompletedStatus(clip) && clipHasTargetAsset(clip, targetAssetKind),
    )
  );
}

function toCamel(value) {
  return value.replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
}

export function classifyChallengeLikeBody(bodyText) {
  const normalized = String(bodyText ?? "").toLowerCase();
  return (
    normalized.includes("challenge") ||
    normalized.includes("captcha") ||
    normalized.includes("cloudflare") ||
    normalized.includes("just a moment") ||
    normalized.includes("cf-chl")
  );
}

export function createError(status, code, message, body) {
  return Object.assign(new Error(message), {
    status,
    code,
    body: normalizeString(body) ?? undefined,
  });
}

export function normalizeError(error) {
  return {
    status: Number.isFinite(error?.status) ? Number(error.status) : 500,
    code: normalizeString(error?.code) ?? "suno_browser_worker_failed",
    message: normalizeString(error?.message) ?? "Suno browser worker failed.",
    body: normalizeString(error?.body) ?? undefined,
  };
}
