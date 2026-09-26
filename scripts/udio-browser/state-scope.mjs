import { isIP } from "node:net";

export function runtimeStateOrigin(baseUrl) {
  const url = new URL(baseUrl);
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) {
    throw new TypeError("Udio runtime state requires an HTTP origin without credentials.");
  }
  return url;
}

export function cookieMatchesOrigin(cookie, target) {
  if (typeof cookie?.domain !== "string") return false;
  const domain = cookie.domain.toLowerCase();
  const hostname = target.hostname.toLowerCase();
  // Host-only cookies must not be broadened to subdomains. IPs never suffix-match.
  const matches = domain.startsWith(".")
    ? domain.length > 1 && (hostname === domain.slice(1) ||
      (!isIP(hostname) && hostname.endsWith(domain)))
    : hostname === domain;
  if (!matches || (cookie.secure && target.protocol !== "https:")) return false;
  // Do not export partitions associated with another top-level origin.
  return cookie.partitionKey === undefined || cookie.partitionKey === target.origin;
}

export function scopeRuntimeState(state, baseUrl) {
  const target = runtimeStateOrigin(baseUrl);
  // A borrowed CDP context may hold unrelated accounts. Only provider state leaves it.
  return {
    cookies: (Array.isArray(state?.cookies) ? state.cookies : [])
      .filter((cookie) => cookieMatchesOrigin(cookie, target)),
    origins: (Array.isArray(state?.origins) ? state.origins : [])
      .filter((entry) => entry?.origin === target.origin),
  };
}
