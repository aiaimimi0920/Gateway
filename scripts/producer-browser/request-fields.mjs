export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function readStringFields(value, fields) {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const field of fields) {
    const normalized = normalizeString(value[field]);
    if (normalized) {
      return normalized;
    }
  }
  return null;
}

export function readNumberFields(value, fields) {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const field of fields) {
    const candidate = value[field];
    if (typeof candidate === "number" && Number.isFinite(candidate)) {
      return candidate;
    }
    if (typeof candidate === "string" && candidate.trim()) {
      const parsed = Number(candidate);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return null;
}
