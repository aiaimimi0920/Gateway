import { normalizeObject } from "./gemini-canvas-browser-pool-input.mjs";

export function sanitizeCanvasProxyHeaders(headers) {
  const normalized = normalizeObject(headers);
  const forbidden = new Set([
    "host",
    "connection",
    "content-length",
    "origin",
    "referer",
    "user-agent",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-dest",
  ]);
  return Object.fromEntries(
    Object.entries(normalized).filter(([key]) => !forbidden.has(String(key).toLowerCase())),
  );
}

export function sanitizeBrowserFetchHeaders(headers) {
  const normalized = normalizeObject(headers);
  const forbiddenPrefixes = ["sec-", "proxy-"];
  const forbidden = new Set([
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "host",
    "keep-alive",
    "origin",
    "permissions-policy",
    "priority",
    "referer",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "user-agent",
    "via",
  ]);
  return Object.fromEntries(
    Object.entries(normalized).filter(([key]) => {
      const lowered = String(key).toLowerCase();
      if (forbidden.has(lowered)) {
        return false;
      }
      return !forbiddenPrefixes.some((prefix) => lowered.startsWith(prefix));
    }),
  );
}
