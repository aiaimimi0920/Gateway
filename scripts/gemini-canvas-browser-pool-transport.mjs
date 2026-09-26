import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createTransportHintOwner({ uniqueStrings }) {
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

  return { extractTransportHintsFromNetworkUrl, mergeTransportHints, hasTransportHints };
}
