import { normalizeString } from "./request-fields.mjs";

const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;

export function parseInput(raw) {
  let input;
  try {
    input = JSON.parse(raw);
  } catch {
    // Native parse diagnostics can embed credentials from malformed stdin.
    throw Object.assign(new Error("Producer worker input must be valid JSON."), {
      status: 400,
      code: "producer_browser_invalid_json",
    });
  }
  validateInput(input);
  return input;
}

export function normalizeBaseUrl(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "producer_browser_missing_base_url",
    });
  }
  return normalized.replace(/\/+$/, "");
}

export function normalizeTimeoutMs(value) {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.max(30_000, Math.floor(value));
  }
  return DEFAULT_TIMEOUT_MS;
}

export function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
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

export function validateInput(input) {
  if (!input || typeof input !== "object") {
    throw Object.assign(new Error("Expected a JSON object on stdin."), {
      status: 400,
      code: "producer_browser_invalid_input",
    });
  }
  if (!normalizeString(input.baseUrl)) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "producer_browser_missing_base_url",
    });
  }
  if (!input.requestBody || typeof input.requestBody !== "object") {
    throw Object.assign(new Error("requestBody must be a JSON object."), {
      status: 400,
      code: "producer_browser_missing_request_body",
    });
  }
}
