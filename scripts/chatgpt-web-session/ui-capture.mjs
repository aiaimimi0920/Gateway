import { writeFile } from "node:fs/promises";
import { normalizeString, parseBoolean } from "./configuration.mjs";
import { parseCookieNames } from "./cookies.mjs";
import { safeParseJson } from "./json-parsing.mjs";

const MAX_CAPTURE_REQUESTS = 128;

function captureUrl(value) {
  if (value.length > 8192) return "[redacted-url]";
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" && url.protocol !== "http:") return "[redacted-url]";
    return `${url.origin}${url.pathname}`.slice(0, 2048);
  } catch {
    return "[redacted-url]";
  }
}

export function createChatGptUiRequestCapture(page) {
  const records = [];
  let active = true;
  let droppedRequests = 0;
  const rawHeaders = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_HEADERS,
    false,
  );
  const rawRequests = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_REQUESTS,
    false,
  );
  const rawResponses = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_RESPONSES,
    false,
  );
  const captureAllPosts = parseBoolean(
    process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_ALL_POSTS,
    false,
  );
  const recordByRequest = new Map();
  const onRequest = (request) => {
    if (!active) return;
    try {
      const url = request.url();
      const matchesTarget =
        captureAllPosts ||
        url.includes("/backend-api/conversation") ||
        url.includes("/backend-api/f/conversation") ||
        url.includes("/ces/");
      if (
        request.method().toUpperCase() !== "POST" ||
        !matchesTarget
      ) {
        return;
      }
      if (records.length >= MAX_CAPTURE_REQUESTS) {
        droppedRequests = Math.min(Number.MAX_SAFE_INTEGER, droppedRequests + 1);
        return;
      }
      const postData = rawRequests ? request.postData() : null;
      const record = {
        url: captureUrl(url),
        method: "POST",
        headers: rawHeaders ? request.headers() : redactCapturedRequestHeaders(request.headers()),
        postDataJson: rawRequests ? safeParseJson(postData) : null,
        postDataPreview: rawRequests ? String(postData ?? "").slice(0, 4000) : null,
      };
      records.push(record);
      recordByRequest.set(request, record);
    } catch {
      // Capture is diagnostic-only; do not break the live UI relay.
    }
  };
  const onResponse = async (response) => {
    if (!active) return;
    try {
      const request = response.request();
      const record = recordByRequest.get(request);
      if (!record) {
        return;
      }
      // Claim once before awaiting body data; duplicates cannot create more reads.
      recordByRequest.delete(request);
      const headers = response.headers();
      const bodyText = rawResponses ? await response.text().catch(() => null) : null;
      if (!active) return;
      record.response = {
        status: response.status(),
        headers: rawHeaders ? headers : redactCapturedRequestHeaders(headers),
        bodyJson: rawResponses ? safeParseJson(bodyText) : null,
        bodyPreview: rawResponses ? String(bodyText ?? "").slice(0, 4000) : null,
        bodyLength: rawResponses ? String(bodyText ?? "").length : null,
      };
    } catch {
      // Capture is diagnostic-only; do not break the live UI relay.
    }
  };
  page.on("request", onRequest);
  page.on("response", onResponse);
  return {
    page, onRequest, onResponse, records, rawHeaders, rawRequests, rawResponses,
    get droppedRequests() { return droppedRequests; },
    detach() {
      active = false;
      page.off("request", onRequest);
      page.off("response", onResponse);
      recordByRequest.clear();
    },
  };
}

export async function finalizeChatGptUiRelayCapture(capture, result) {
  if (!capture) {
    return result;
  }
  capture.detach();
  const summary = {
    capturedAt: new Date().toISOString(),
    rawHeaders: capture.rawHeaders === true,
    rawRequests: capture.rawRequests === true,
    rawResponses: capture.rawResponses === true,
    droppedRequests: capture.droppedRequests,
    requests: capture.records,
  };
  const targetPath = normalizeString(process.env.CHATGPT_WEB_UI_REQUEST_CAPTURE_PATH);
  if (targetPath) {
    await writeFile(targetPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8").catch(() => {});
  }
  return {
    ...result,
    uiRequestCapture: summary,
  };
}

export function redactCapturedRequestHeaders(headers) {
  const result = Object.create(null);
  let count = 0;
  for (const originalName in headers ?? {}) {
    if (!Object.hasOwn(headers, originalName)) continue;
    if (count++ >= 64) break;
    const name = originalName.slice(0, 128);
    const value = String(headers[originalName] ?? "");
    const lower = name.toLowerCase();
    if (lower === "authorization") {
      result[name] = { redacted: true, kind: "authorization", length: String(value ?? "").length };
    } else if (lower === "cookie") {
      result[name] = {
        redacted: true,
        kind: "cookie",
        length: String(value ?? "").length,
        cookieNames: parseCookieNames(value.slice(0, 4096)).slice(0, 32).map((name) => name.slice(0, 128)),
      };
    } else if (lower === "x-oai-is") {
      result[name] = { redacted: true, kind: "x-oai-is", length: String(value ?? "").length };
    } else if (lower.includes("token")) {
      result[name] = { redacted: true, kind: "token", length: String(value ?? "").length };
    } else if (
      (lower === "content-type" && value.length <= 256 &&
        /^[\w.+-]+\/[\w.+-]+(?:;\s*charset=[\w-]+)?$/i.test(value)) ||
      (lower === "content-length" && /^\d{1,16}$/.test(value))
    ) {
      result[name] = value;
    } else {
      // Unknown headers can carry credentials, redirects or private metadata.
      result[name] = { redacted: true, kind: "header", length: value.length };
    }
  }
  return result;
}
