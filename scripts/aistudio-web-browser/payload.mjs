import { normalizeString } from "./settings.mjs";

export function validateInput(input) {
  if (!normalizeString(input?.runtimeStateObjectKey)) {
    throw new Error("runtimeStateObjectKey is required.");
  }
  if (!input?.requestSpec || typeof input.requestSpec !== "object") {
    throw new Error("requestSpec is required.");
  }
  if (!normalizeString(input.requestSpec?.url)) {
    throw new Error("requestSpec.url is required.");
  }
}

export function normalizeHeaders(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(value)
      .map(([key, val]) => [String(key), typeof val === "string" ? val : String(val ?? "")])
      .filter(([key, val]) => key.trim() && val.trim()),
  );
}

export function stripUtf8Bom(text) {
  return typeof text === "string" && text.charCodeAt(0) === 0xfeff
    ? text.slice(1)
    : text;
}

export function tryParseJson(text) {
  try {
    return JSON.parse(text);
  } catch (_) {
    return null;
  }
}

export function previewText(text, maxLength = 200) {
  if (typeof text !== "string") {
    return "";
  }
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

export function isGenerateContentRequestSpec(requestSpec) {
  const url = normalizeString(requestSpec?.url) ?? "";
  if (!/\/models\/[^/?#:]+:(stream)?generateContent/i.test(url)) {
    return false;
  }
  const parsedBody = tryParseJson(
    typeof requestSpec?.body === "string" ? requestSpec.body : "",
  );
  if (!parsedBody || typeof parsedBody !== "object") {
    return true;
  }
  const responseModalities = parsedBody?.generationConfig?.responseModalities;
  if (Array.isArray(responseModalities)) {
    const normalizedModalities = responseModalities
      .map((entry) => normalizeString(String(entry)))
      .filter(Boolean)
      .map((entry) => entry.toUpperCase());
    if (normalizedModalities.some((entry) => entry !== "TEXT")) {
      return false;
    }
  }
  if (parsedBody?.generationConfig?.speechConfig) {
    return false;
  }
  return true;
}

export function extractRequestedModelFromUrl(url) {
  const normalized = normalizeString(url) ?? "";
  const match = normalized.match(/\/models\/([^/?#:]+):(stream)?generateContent/i);
  return match?.[1] ?? null;
}

export function collectGeminiTextStrings(node, acc = []) {
  if (typeof node === "string") {
    acc.push(node);
    return acc;
  }
  if (Array.isArray(node)) {
    for (const entry of node) {
      collectGeminiTextStrings(entry, acc);
    }
    return acc;
  }
  if (node && typeof node === "object") {
    for (const value of Object.values(node)) {
      collectGeminiTextStrings(value, acc);
    }
  }
  return acc;
}
