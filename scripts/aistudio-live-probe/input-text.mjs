function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeStringArray(value, fallback) {
  if (!Array.isArray(value)) {
    return fallback;
  }
  const normalized = value
    .map((entry) => normalizeString(entry))
    .filter(Boolean);
  return normalized.length ? normalized : fallback;
}

function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
  if (!normalized) return fallback;
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  return fallback;
}

function stripUtf8Bom(text) {
  return typeof text === "string" && text.charCodeAt(0) === 0xfeff
    ? text.slice(1)
    : text;
}

function tryParseJson(text) {
  try {
    return JSON.parse(text);
  } catch (_) {
    return null;
  }
}

function previewText(value, limit = 4096) {
  return String(value ?? "").slice(0, limit);
}

export { normalizeString, normalizeStringArray, parseBoolean, stripUtf8Bom, tryParseJson, previewText };
