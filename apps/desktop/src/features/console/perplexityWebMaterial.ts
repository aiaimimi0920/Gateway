import { fromPerplexityCookieHeader, fromPerplexityStorageState, PERPLEXITY_SESSION_COOKIE, validCookieValue } from "./perplexityWebCookies";

export type PerplexityWebMaterial = { product: "perplexity-web"; token: string; cookieHeader: string; userAgent: string; expiresAt?: string };
const invalid = () => new Error("会话材料 Perplexity 格式无效；需要本站 session 和原会话 User-Agent，不接受 API Key/OAuth 或目的地配置。");
function suppliedExpiry(value: unknown, now: number): number | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw invalid();
  const match = /^(\d{4})-(\d\d)-(\d\d)T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d(?:\.\d{1,9})?(?:Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)$/.exec(value);
  if (!match || Number(match[2]) < 1 || Number(match[2]) > 12 || Number(match[3]) < 1
    || Number(match[3]) > new Date(Date.UTC(Number(match[1]), Number(match[2]), 0)).getUTCDate()) throw invalid();
  const expiry = Date.parse(value);
  if (!Number.isFinite(expiry) || expiry <= now) throw invalid();
  return expiry;
}

/** Product-specific projection reused by the ordinary form and secret staging. No I/O. */
export function parsePerplexityWebMaterial(source: string, now = Date.now()): PerplexityWebMaterial {
  if (source.length > 128 * 1024 || new TextEncoder().encode(source).length > 128 * 1024) throw new Error("会话材料不能超过 128 KiB。");
  let input: Record<string, unknown>;
  try { input = JSON.parse(source); } catch { throw invalid(); }
  if (!input || typeof input !== "object" || Array.isArray(input) || input.product !== "perplexity-web"
    || Object.keys(input).some((key) => !["product", "token", "cookieHeader", "cfClearance", "storageState", "userAgent", "expiresAt"].includes(key))) throw invalid();
  const ua = input.userAgent;
  if (typeof ua !== "string" || !ua.trim() || ua.length > 1024 || !/^[\x20-\x7e]+$/.test(ua)) throw invalid();
  let cookies;
  if (input.storageState !== undefined) {
    if ([input.token, input.cookieHeader, input.cfClearance].some((v) => v !== undefined)) throw invalid();
    cookies = fromPerplexityStorageState(input.storageState, now);
  } else if (input.cookieHeader !== undefined) {
    if (input.cfClearance !== undefined) throw invalid();
    cookies = fromPerplexityCookieHeader(input.cookieHeader);
    if (input.token !== undefined && input.token !== cookies.token) throw invalid();
  } else {
    if (!validCookieValue(input.token) || input.cfClearance !== undefined && !validCookieValue(input.cfClearance)) throw invalid();
    cookies = fromPerplexityCookieHeader(`${PERPLEXITY_SESSION_COOKIE}=${input.token}${input.cfClearance ? `; cf_clearance=${input.cfClearance}` : ""}`);
  }
  const expiry = Math.min(cookies.expiresAt ?? Infinity, suppliedExpiry(input.expiresAt, now) ?? Infinity);
  return { product: "perplexity-web", token: cookies.token, cookieHeader: cookies.cookieHeader, userAgent: ua,
    ...(Number.isFinite(expiry) ? { expiresAt: new Date(expiry).toISOString() } : {}) };
}
