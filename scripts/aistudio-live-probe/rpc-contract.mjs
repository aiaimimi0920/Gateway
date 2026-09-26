import { normalizeString } from "./input-text.mjs";

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

export { buildNormalizedTargetRpcContract, isReplayReadyTargetRpcContract, isSuccessfulTargetRpcStatus };
