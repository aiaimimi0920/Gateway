export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function normalizeObject(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

export function scopeGeminiUrlToAuthUser(rawUrl, authUser) {
  const normalizedUrl = normalizeString(rawUrl);
  const normalizedAuthUser = normalizeString(authUser);
  if (!normalizedUrl || !/^\d+$/.test(normalizedAuthUser ?? "")) {
    return normalizedUrl;
  }
  try {
    const parsed = new URL(normalizedUrl, "https://gemini.google.com");
    if (!/^(?:www\.)?gemini\.google\.com$/i.test(parsed.hostname)) {
      return normalizedUrl;
    }
    const unscopedPath = parsed.pathname.replace(/^\/u\/\d+(?=\/|$)/i, "") || "/";
    if (/^\/share\//i.test(unscopedPath)) {
      parsed.pathname = unscopedPath;
      parsed.searchParams.set("authuser", normalizedAuthUser);
      return parsed.toString();
    }
    parsed.pathname = `/u/${normalizedAuthUser}${unscopedPath === "/" ? "/" : unscopedPath}`;
    return parsed.toString().replace(/\/$/, unscopedPath === "/" ? "/" : "");
  } catch {
    return normalizedUrl;
  }
}

export function applyGeminiAccountScope(args) {
  const normalizedArgs = args && typeof args === "object" ? args : {};
  const authUser = normalizeString(normalizedArgs.authUser);
  if (!/^\d+$/.test(authUser ?? "")) {
    return normalizedArgs;
  }
  const scope = (value) => scopeGeminiUrlToAuthUser(value, authUser);
  return {
    ...normalizedArgs,
    baseUrl: scope(normalizedArgs.baseUrl ?? "https://gemini.google.com"),
    canvasProgramUrl: scope(normalizedArgs.canvasProgramUrl),
    programUrl: scope(normalizedArgs.programUrl),
    pageUrl: scope(normalizedArgs.pageUrl),
  };
}
