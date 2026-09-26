import { normalizeString } from "./input-text.mjs";

const DEFAULT_LOCAL_PROXY_MODEL = "gemini-3-flash-preview";
const DEFAULT_LOCAL_PROXY_STREAMING_MODE = "fake";
const DEFAULT_LOCAL_PROXY_PROMPT = "Reply with exactly OK.";

function createRequestId(prefix = "req") {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

function createRequestAttemptId(requestId, attemptNumber = 1) {
  return `${requestId}_attempt_${attemptNumber}_${Math.random().toString(36).slice(2, 8)}`;
}

function buildDefaultLocalProxyRequest(input) {
  const explicit = input?.localProxyRequest;
  if (explicit === false || input?.disableLocalProxyRequest === true) {
    return null;
  }

  const requestSpec =
    explicit && typeof explicit === "object" && !Array.isArray(explicit) ? explicit : {};
  const requestId = normalizeString(requestSpec.request_id) ?? createRequestId("req");
  const requestAttemptId =
    normalizeString(requestSpec.request_attempt_id) ?? createRequestAttemptId(requestId, 1);
  const streamingMode =
    normalizeString(requestSpec.streaming_mode) ??
    normalizeString(input?.localProxyStreamingMode) ??
    DEFAULT_LOCAL_PROXY_STREAMING_MODE;
  const method = (normalizeString(requestSpec.method) ?? "POST").toUpperCase();
  // Keep GET/HEAD probes truly bodyless; otherwise we accidentally introduce
  // preflight/CORS behavior that does not exist in the real browser-owned lane.
  const expectsRequestBody = ["POST", "PUT", "PATCH"].includes(method);
  const model =
    normalizeString(requestSpec.model) ??
    normalizeString(input?.localProxyModel) ??
    DEFAULT_LOCAL_PROXY_MODEL;
  const prompt =
    normalizeString(requestSpec.prompt) ??
    normalizeString(input?.localProxyPrompt) ??
    normalizeString(input?.autoPrompt) ??
    DEFAULT_LOCAL_PROXY_PROMPT;
  const pathValue =
    normalizeString(requestSpec.path) ??
    `/v1beta/models/${model}:${streamingMode === "real" ? "streamGenerateContent" : "generateContent"}`;
  const queryParams =
    requestSpec.query_params &&
    typeof requestSpec.query_params === "object" &&
    !Array.isArray(requestSpec.query_params)
      ? requestSpec.query_params
      : streamingMode === "real"
        ? { alt: "sse" }
        : {};
  const headers = normalizeHeadersObject(requestSpec.headers);
  const hasContentType = Object.keys(headers).some(
    (key) => key.toLowerCase() === "content-type",
  );
  if (expectsRequestBody && !hasContentType) {
    headers["content-type"] = "application/json";
  }

  let body = requestSpec.body;
  if (body && typeof body !== "string") {
    body = JSON.stringify(body);
  }
  if ((body === undefined || body === null || body === "") && expectsRequestBody) {
    body = JSON.stringify({
      contents: [
        {
          role: "user",
          parts: [{ text: prompt }],
        },
      ],
    });
  }

  return {
    event_type: "proxy_request",
    request_id: requestId,
    request_attempt_id: requestAttemptId,
    method,
    path: pathValue,
    query_params: queryParams,
    headers,
    body,
    streaming_mode: streamingMode,
    is_generative:
      typeof requestSpec.is_generative === "boolean" ? requestSpec.is_generative : true,
  };
}

function normalizeHeadersObject(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(value)
      .map(([key, entry]) => [String(key), typeof entry === "string" ? entry : String(entry ?? "")])
      .filter(([key, entry]) => key.trim() && entry.trim()),
  );
}

export { buildDefaultLocalProxyRequest };
