/** Only the two fixed-site cookies leave the importer; no profile flattening or URL decoding. */
export const PERPLEXITY_SESSION_COOKIE = "__Secure-next-auth.session-token";
const CLEARANCE = "cf_clearance";
type Cookie = { name: string; value: string; expires?: number };
export type PerplexityCookies = { token: string; cookieHeader: string; expiresAt?: number };
const invalid = () => new Error("会话材料 Perplexity Cookie 无效、重复、分片不完整或已过期。");
export function validCookieValue(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= 16 * 1024
    && /^[\x21\x23-\x2b\x2d-\x3a\x3c-\x5b\x5d-\x7e]+$/.test(value);
}
function assemble(cookies: Cookie[], chunked: boolean): PerplexityCookies {
  const values = new Map<string, Cookie>();
  for (const cookie of cookies) {
    if (!validCookieValue(cookie.value) || values.has(cookie.name)) throw invalid();
    values.set(cookie.name, cookie);
  }
  const chunks = cookies.filter((c) => c.name.startsWith(`${PERPLEXITY_SESSION_COOKIE}.`));
  let token = values.get(PERPLEXITY_SESSION_COOKIE)?.value;
  if (chunks.length) {
    if (!chunked || token || chunks.length > 16) throw invalid();
    token = Array.from({ length: chunks.length }, (_, index) => {
      const value = values.get(`${PERPLEXITY_SESSION_COOKIE}.${index}`)?.value;
      if (!value) throw invalid();
      return value;
    }).join("");
  }
  if (!validCookieValue(token)) throw invalid();
  const clearance = values.get(CLEARANCE)?.value;
  const expiry = cookies.flatMap((c) => c.expires !== undefined && c.expires !== -1 ? [c.expires * 1000] : []);
  return { token, cookieHeader: `${PERPLEXITY_SESSION_COOKIE}=${token}${clearance ? `; ${CLEARANCE}=${clearance}` : ""}`,
    ...(expiry.length ? { expiresAt: Math.min(...expiry) } : {}) };
}
const selected = (name: string) => name === PERPLEXITY_SESSION_COOKIE || name.startsWith(`${PERPLEXITY_SESSION_COOKIE}.`) || name === CLEARANCE;

/** Raw header mode is an explicit operator assertion; it proves no domain/secure attributes. */
export function fromPerplexityCookieHeader(header: unknown): PerplexityCookies {
  if (typeof header !== "string" || !header || header.length > 65536 || /[\x00-\x1f\x7f-\x9f]/.test(header)) throw invalid();
  const names = new Set<string>();
  const cookies: Cookie[] = [];
  for (const part of header.split(";")) {
    const match = /^\s*([!#$%&'*+.^_`|~0-9A-Za-z-]+)=([^;]*)\s*$/.exec(part);
    if (!match || names.has(match[1])) throw invalid();
    names.add(match[1]);
    if (selected(match[1])) cookies.push({ name: match[1], value: match[2].trim() });
  }
  // Header-only chunks have no browser attributes; require a normalized unsplit value instead.
  return assemble(cookies, false);
}

/** Playwright storageState cookies retain attributes until admission, including every selected chunk. */
export function fromPerplexityStorageState(state: unknown, now: number): PerplexityCookies {
  if (!state || typeof state !== "object" || Array.isArray(state)) throw invalid();
  const input = state as Record<string, unknown>;
  if (Object.keys(input).some((key) => !["cookies", "origins"].includes(key)) || !Array.isArray(input.cookies)
    || input.cookies.length > 256 || input.origins !== undefined && (!Array.isArray(input.origins) || input.origins.length > 64)) throw invalid();
  const cookies: Cookie[] = [];
  for (const raw of input.cookies) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) throw invalid();
    const c = raw as Record<string, unknown>;
    if (typeof c.name !== "string" || typeof c.domain !== "string") throw invalid();
    if (!selected(c.name)) continue;
    // A host-only apex cookie does not match www. A domain cookie must use the explicit dot form.
    if (!["www.perplexity.ai", ".www.perplexity.ai", ".perplexity.ai"].includes(c.domain)) continue;
    if (c.path !== "/" || c.secure !== true || !validCookieValue(c.value) || typeof c.expires !== "number"
      || !Number.isFinite(c.expires) || c.expires !== -1 && (c.expires * 1000 <= now || c.expires * 1000 > 8640000000000000)
      || c.partitionKey !== undefined) throw invalid();
    cookies.push({ name: c.name, value: c.value, expires: c.expires });
  }
  // origins/localStorage are deliberately not projected; they cannot set identity or destinations.
  return assemble(cookies, true);
}
