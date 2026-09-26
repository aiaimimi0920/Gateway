import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function isInterestingNetworkUrl(url) {
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

export function isLikelyAvatarUrl(url) {
  const lowered = String(url || "").toLowerCase();
  return (
    lowered.includes("lh3.googleusercontent.com/a/") ||
    lowered.includes("lh3.googleusercontent.com/u/0/ogw") ||
    lowered.includes("=s64-")
  );
}

export function isLikelyNoiseMediaUrl(url) {
  const lowered = String(url || "").toLowerCase();
  return (
    lowered.includes("googleadservices.com") ||
    lowered.includes("/pagead/") ||
    lowered.includes("1p-conversion") ||
    lowered.includes("doubleclick.net") ||
    lowered.includes("google-analytics.com")
  );
}

export function inferMimeTypeFromUrl(url, fallback) {
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

export function isBlobLikeUrl(url) {
  return /^blob:/i.test(String(url || "").trim());
}

export function isAudioLikeMimeType(mimeType) {
  return /^audio\//i.test(String(mimeType || "").trim());
}

export function isAudioLikeUrl(url) {
  return /googlevideo|gvt1|\.wav(\?|$)|\.mp3(\?|$)|\.ogg(\?|$)|filename=.*\.(wav|mp3|ogg)(\b|$)/i.test(
    String(url || ""),
  );
}

export function isVideoLikeUrl(url) {
  return /googlevideo|gvt1|\.mp4(\?|$)|\.webm(\?|$)|\.mov(\?|$)|filename=.*\.(mp4|webm|mov)(\b|$)/i.test(
    String(url || ""),
  );
}

export function normalizeGeminiBrowserAssetUrl(url) {
  const value = normalizeString(url);
  if (!value) {
    return null;
  }
  try {
    const parsed = new URL(value);
    if (
      parsed.protocol === "http:" &&
      (
        /(^|\.)googleusercontent\.com$/i.test(parsed.hostname) ||
        /(^|\.)googlevideo\.com$/i.test(parsed.hostname) ||
        /(^|\.)gvt1\.com$/i.test(parsed.hostname)
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

export function pushUniqueMediaUrl(store, candidate) {
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
