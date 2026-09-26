const APP_PATH_REGEX = /(?:https:\/\/gemini\.google\.com)?(\/app\/(?:[0-9a-f]{8,}|\d{13,}))/gi;
const CONVERSATION_ID_REGEX = /\bc_([0-9a-f]{8,})\b/gi;
const RESPONSE_ID_REGEX = /\br_([0-9a-f]{8,})\b/gi;
const SHARE_PATH_REGEX = /(\/share\/[0-9a-z]{8,})/gi;
const CONVERSATION_RESPONSE_PAIR_REGEX =
  /(?:\\?")?(c_[0-9a-f]{8,})(?:\\?")?\s*,\s*(?:\\?")?(r_[0-9a-f]{8,})(?:\\?")?/gi;
export function uniqueStrings(values) {
  return [...new Set(values.filter(Boolean).map((value) => String(value).trim()).filter(Boolean))];
}

export function extractAppPathsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = APP_PATH_REGEX.exec(source)) !== null) {
    matches.push(match[1]);
  }
  return uniqueStrings(matches);
}

export function extractConversationIdsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = CONVERSATION_ID_REGEX.exec(source)) !== null) {
    matches.push(`c_${match[1]}`);
  }
  return uniqueStrings(matches);
}

export function extractResponseIdsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = RESPONSE_ID_REGEX.exec(source)) !== null) {
    matches.push(`r_${match[1]}`);
  }
  return uniqueStrings(matches);
}

export function extractSharePathsFromText(text) {
  const matches = [];
  const source = String(text ?? "");
  let match;
  while ((match = SHARE_PATH_REGEX.exec(source)) !== null) {
    matches.push(match[1]);
  }
  return uniqueStrings(matches);
}

export function extractHandleHintsFromText(text) {
  const appPaths = extractAppPathsFromText(text);
  const conversationIds = extractConversationIdsFromText(text);
  const responseIds = extractResponseIdsFromText(text);
  const sharePaths = extractSharePathsFromText(text);
  const derivedAppPaths = conversationIds.map((id) => `/app/${id.replace(/^c_/, "")}`);
  return {
    appPaths: uniqueStrings([...appPaths, ...derivedAppPaths]),
    conversationIds,
    responseIds,
    sharePaths,
  };
}

export function sanitizeTransportHintUrl(rawUrl) {
  const normalized = String(rawUrl ?? "").trim();
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

export function deriveInvokeBaseUrlFromRequestUrl(rawUrl) {
  const normalized = sanitizeTransportHintUrl(rawUrl);
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

export function deriveVideoInvokePathFromRequestUrl(rawUrl) {
  const normalized = sanitizeTransportHintUrl(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    return /:predictLongRunning$/i.test(parsed.pathname) ? parsed.pathname : null;
  } catch {
    return null;
  }
}

export function deriveMusicWsUrlFromRequestUrl(rawUrl) {
  const normalized = sanitizeTransportHintUrl(rawUrl);
  if (!normalized) {
    return null;
  }
  try {
    const parsed = new URL(normalized);
    return /BidiGenerateMusic/i.test(parsed.pathname) ? parsed.toString() : null;
  } catch {
    return null;
  }
}

export function extractTransportHintsFromUrl(rawUrl) {
  return {
    invokeBaseUrl: deriveInvokeBaseUrlFromRequestUrl(rawUrl),
    musicWsUrl: deriveMusicWsUrlFromRequestUrl(rawUrl),
    videoInvokePath: deriveVideoInvokePathFromRequestUrl(rawUrl),
  };
}

export function mergeTransportHints(target, incoming) {
  target.invokeBaseUrls = uniqueStrings([
    ...(target.invokeBaseUrls || []),
    ...(incoming.invokeBaseUrl ? [incoming.invokeBaseUrl] : []),
  ]);
  target.musicWsUrls = uniqueStrings([
    ...(target.musicWsUrls || []),
    ...(incoming.musicWsUrl ? [incoming.musicWsUrl] : []),
  ]);
  target.videoInvokePaths = uniqueStrings([
    ...(target.videoInvokePaths || []),
    ...(incoming.videoInvokePath ? [incoming.videoInvokePath] : []),
  ]);
}

export function buildHandlePair(conversationId, responseId, source = {}) {
  const normalizedConversationId = normalizeHandleId(conversationId, "c_");
  const normalizedResponseId = normalizeHandleId(responseId, "r_");
  if (!normalizedConversationId || !normalizedResponseId) {
    return null;
  }
  const appPath = `/app/${normalizedConversationId.replace(/^c_/, "")}`;
  return {
    appPath,
    programUrl: `https://gemini.google.com${appPath}`,
    conversationId: normalizedConversationId,
    responseId: normalizedResponseId,
    sourceUrl: source.sourceUrl ?? null,
    sourceRpc: source.sourceRpc ?? null,
    sourceKind: source.sourceKind ?? null,
    sourceSurface: source.sourceSurface ?? null,
    sourceWsUrl: source.sourceWsUrl ?? null,
    sourceTargetDomain: source.sourceTargetDomain ?? null,
    ts: source.ts ?? null,
  };
}

export function normalizeHandleId(value, prefix) {
  const text = String(value ?? "").trim();
  if (!text) {
    return null;
  }
  const normalized = text.startsWith(prefix) ? text : `${prefix}${text.replace(/^[_-]+/, "")}`;
  return new RegExp(`^${prefix}[0-9a-f]{8,}$`, "i").test(normalized) ? normalized : null;
}

export function extractHandlePairsFromText(text, source = {}) {
  const matches = [];
  const input = String(text ?? "");
  let match;
  while ((match = CONVERSATION_RESPONSE_PAIR_REGEX.exec(input)) !== null) {
    const pair = buildHandlePair(match[1], match[2], source);
    if (pair) {
      matches.push(pair);
    }
  }
  return dedupeHandlePairs(matches);
}

export function mergeHints(target, incoming) {
  target.appPaths = uniqueStrings([...(target.appPaths || []), ...(incoming.appPaths || [])]);
  target.conversationIds = uniqueStrings([
    ...(target.conversationIds || []),
    ...(incoming.conversationIds || []),
  ]);
  target.responseIds = uniqueStrings([...(target.responseIds || []), ...(incoming.responseIds || [])]);
  target.sharePaths = uniqueStrings([...(target.sharePaths || []), ...(incoming.sharePaths || [])]);
}

export function dedupeHandlePairs(pairs) {
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

export function mergeHandlePairs(target, incoming) {
  const merged = dedupeHandlePairs([...(target || []), ...(incoming || [])]);
  target.length = 0;
  target.push(...merged);
}

export function extractAppPath(url) {
  if (!url) {
    return null;
  }
  const matches = extractAppPathsFromText(url);
  return matches[0] ?? null;
}

export function selectProgramConversationPair(handlePairs, stableAppPath) {
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
  const preferredByPath =
    stableAppPath &&
    reversed.find(
      (pair) =>
        pair.appPath === stableAppPath &&
        /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
    );
  if (preferredByPath) {
    return preferredByPath;
  }
  const preferredSharePair = reversed.find((pair) =>
    /source-path=%2Fshare%2F/i.test(String(pair.sourceUrl || "")),
  );
  if (preferredSharePair) {
    return preferredSharePair;
  }
  return reversed.find((pair) => pair.appPath && pair.conversationId && pair.responseId) ?? null;
}

export function selectLatestResponsePair(handlePairs) {
  return (
    [...(handlePairs || [])]
      .reverse()
      .find((pair) => pair.conversationId && pair.responseId) ?? null
  );
}

export function hasCanvasProxyProgramCandidate(handlePairs, invokeContract = null) {
  if (invokeContract?.transportKind === "canvas_program_ws_candidate") {
    return true;
  }
  return [...(handlePairs || [])].some((pair) => pair?.sourceSurface === "canvas_proxy_client");
}

export function concreteAppPathFromUrl(url) {
  const matched = extractAppPath(url);
  return matched && /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(matched) ? matched : null;
}

export function selectCanonicalProgramPair(handlePairs, pageUrl, stableAppPath = null) {
  const stablePair = selectProgramConversationPair(handlePairs, stableAppPath);
  const latestPair = selectLatestResponsePair(handlePairs);
  const finalAppPath = concreteAppPathFromUrl(pageUrl);
  if (
    finalAppPath &&
    latestPair?.appPath === finalAppPath &&
    latestPair?.conversationId &&
    latestPair?.responseId
  ) {
    return latestPair;
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
  return stablePair ?? latestPair;
}

export function deriveShareId(shareUrlValue) {
  const match = String(shareUrlValue || "").match(/\/share\/([0-9a-z]{8,})/i);
  return match?.[1] ?? null;
}

export function deriveConversationIdForAppPath(appPath, conversationIds) {
  const suffix = String(appPath || "").split("/").pop()?.trim();
  if (!suffix) {
    return [...(conversationIds || [])].reverse()[0] ?? null;
  }
  const matched = [...(conversationIds || [])]
    .reverse()
    .find((value) => String(value).replace(/^c_/, "") === suffix);
  return matched ?? [...(conversationIds || [])].reverse()[0] ?? null;
}
