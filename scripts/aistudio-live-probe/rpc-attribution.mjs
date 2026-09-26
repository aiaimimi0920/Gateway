import { normalizeString } from "./input-text.mjs";

const AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
const AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";

function requestMatches(url, patterns) {
  return patterns.some((pattern) => url.includes(pattern));
}


function requestMethodAndUrlKey(method, url) {
  const normalizedMethod = normalizeString(method);
  const normalizedUrl = normalizeString(url);
  return normalizedMethod && normalizedUrl ? `${normalizedMethod} ${normalizedUrl}` : null;
}

function getRequestMethod(request) {
  if (!request) {
    return null;
  }
  if (typeof request.method === "function") {
    return request.method();
  }
  return request.method;
}

function getRequestUrl(request) {
  if (!request) {
    return null;
  }
  if (typeof request.url === "function") {
    return request.url();
  }
  return request.url;
}

function removePendingRequestId(pendingByKey, requestId) {
  if (!requestId) {
    return;
  }
  for (const [key, pending] of pendingByKey.entries()) {
    const index = pending.indexOf(requestId);
    if (index < 0) {
      continue;
    }
    pending.splice(index, 1);
    if (!pending.length) {
      pendingByKey.delete(key);
    }
    return;
  }
}

function createCaptureRequestIdFactory(now = () => Date.now()) {
  let sequence = 0;
  return () => {
    sequence += 1;
    return `${now()}-${sequence}`;
  };
}

function createResponseRequestAttributor() {
  const requestIdsByRequest = new WeakMap();
  const pendingByKey = new Map();

  const trackRequest = (request, requestId) => {
    if (!request || typeof request !== "object" || !requestId) {
      return;
    }
    requestIdsByRequest.set(request, requestId);
    const key = requestMethodAndUrlKey(
      getRequestMethod(request),
      getRequestUrl(request),
    );
    if (!key) {
      return;
    }
    const pending = pendingByKey.get(key) ?? [];
    pending.push(requestId);
    pendingByKey.set(key, pending);
  };

  const resolveResponse = (response) => {
    const request =
      response && typeof response.request === "function"
        ? response.request()
        : response?.request;
    if (request && typeof request === "object" && requestIdsByRequest.has(request)) {
      const requestId = requestIdsByRequest.get(request);
      removePendingRequestId(pendingByKey, requestId);
      return requestId;
    }
    const key = requestMethodAndUrlKey(
      getRequestMethod(request),
      typeof response?.url === "function" ? response.url() : response?.url,
    );
    const pending = key ? pendingByKey.get(key) : null;
    if (!pending?.length) {
      return null;
    }
    const requestId = pending.shift() ?? null;
    if (!pending.length) {
      pendingByKey.delete(key);
    }
    return requestId;
  };

  return { trackRequest, resolveResponse };
}

function isAistudioTargetRpcUrl(url) {
  const normalized = normalizeString(url) ?? "";
  return (
    normalized.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH) ||
    normalized.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH)
  );
}

function detectAistudioTargetRpcKind(url) {
  const normalized = normalizeString(url) ?? "";
  if (normalized.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH)) {
    return "code_assistant_offline";
  }
  if (normalized.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH)) {
    return "stream_code_assistant_offline_generation";
  }
  return null;
}

function hasCompleteTargetRpcPair(entries) {
  return entries.some((entry) => entry?.request && entry?.response);
}

function summarizeTargetRpcContracts(capture) {
  const requestById = new Map(
    (Array.isArray(capture?.requests) ? capture.requests : [])
      .filter((entry) => entry && entry.id)
      .map((entry) => [entry.id, entry]),
  );
  const pairs = [];
  for (const response of Array.isArray(capture?.responses) ? capture.responses : []) {
    const kind = detectAistudioTargetRpcKind(response?.url);
    if (!kind) {
      continue;
    }
    const request = response?.requestId ? requestById.get(response.requestId) ?? null : null;
    pairs.push({
      kind,
      request,
      response,
    });
  }
  for (const request of Array.isArray(capture?.requests) ? capture.requests : []) {
    const kind = detectAistudioTargetRpcKind(request?.url);
    if (!kind) {
      continue;
    }
    const alreadyPaired = pairs.some((entry) => entry.request?.id === request.id);
    if (!alreadyPaired) {
      pairs.push({
        kind,
        request,
        response: null,
      });
    }
  }

  const codeAssistantOffline = pairs.filter((entry) => entry.kind === "code_assistant_offline");
  const streamCodeAssistantOfflineGeneration = pairs.filter(
    (entry) => entry.kind === "stream_code_assistant_offline_generation",
  );

  return {
    capturedTargetRpcContract:
      hasCompleteTargetRpcPair(codeAssistantOffline) &&
      hasCompleteTargetRpcPair(streamCodeAssistantOfflineGeneration),
    targetRpcPairCount: pairs.length,
    codeAssistantOfflineCount: codeAssistantOffline.length,
    streamCodeAssistantOfflineGenerationCount:
      streamCodeAssistantOfflineGeneration.length,
    pairs,
  };
}

export { requestMatches, createCaptureRequestIdFactory, createResponseRequestAttributor, detectAistudioTargetRpcKind, summarizeTargetRpcContracts };
