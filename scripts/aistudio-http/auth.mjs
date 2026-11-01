/** Request-local Cookie authentication. No browser, profile, API key account, or storage export. */
import { createHash } from "node:crypto";

export const ORIGIN = "https://aistudio.google.com";
export const RPC = "https://alkalimakersuite-pa.clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/";
export const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:152.0) Gecko/20100101 Firefox/152.0";
const SIGNERS = [["SAPISIDHASH", "SAPISID"], ["SAPISID1PHASH", "__Secure-1PAPISID"], ["SAPISID3PHASH", "__Secure-3PAPISID"]];
const DOMAINS = new Set(["google.com", "aistudio.google.com", "alkalimakersuite-pa.clients6.google.com"]);

export function matchingCookies(bundle, target) {
  const url = new URL(target), now = Date.now() / 1000;
  return bundle.cookies.filter(cookie => {
    const domain = cookie.domain.replace(/^\./, "");
    const host = url.hostname === domain || cookie.domain.startsWith(".") && url.hostname.endsWith(`.${domain}`);
    const pathname = url.pathname === cookie.path || url.pathname.startsWith(cookie.path.endsWith("/") ? cookie.path : `${cookie.path}/`);
    return host && pathname && (!cookie.secure || url.protocol === "https:") && (cookie.expires < 0 || cookie.expires > now);
  }).sort((a, b) => b.path.length - a.path.length);
}

export function validateBundle(bundle) {
  if (bundle?.schemaVersion !== 1 || bundle?.provider !== "aistudio-web" || !/^\d{1,3}$/.test(bundle.authUser)
    || !Array.isArray(bundle.cookies) || bundle.cookies.length > 64 || Buffer.byteLength(JSON.stringify(bundle)) > 32768) {
    throw new Error("aistudio_http_cookie_material_invalid");
  }
  for (const c of bundle.cookies) {
    if (typeof c.name !== "string" || !/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(c.name)
      || typeof c.value !== "string" || c.value.length > 8192 || /[;\r\n\0]/.test(c.value)
      || typeof c.domain !== "string" || !DOMAINS.has(c.domain.replace(/^\./, ""))
      || typeof c.path !== "string" || !c.path.startsWith("/") || !Number.isFinite(c.expires)
      || typeof c.secure !== "boolean" || c.partitionKey) throw new Error("aistudio_http_cookie_material_invalid");
  }
  authorization(bundle);
  return bundle;
}

export function cookieHeader(bundle, target) {
  return matchingCookies(bundle, target).map(c => `${c.name}=${c.value}`).join("; ");
}

export function authorization(bundle, timestamp = Math.floor(Date.now() / 1000)) {
  const available = matchingCookies(bundle, `${ORIGIN}/`);
  return SIGNERS.map(([label, name]) => {
    const cookie = available.find(c => c.name === name && c.value);
    if (!cookie) throw new Error("aistudio_http_signing_cookie_missing");
    const digest = createHash("sha1").update(`${timestamp} ${cookie.value} ${ORIGIN}`).digest("hex");
    return `${label} ${timestamp}_${digest}`;
  }).join(" ");
}

export function requestHeaders(bundle, publicKey, visitId, target = RPC) {
  return { "Content-Type": "application/json+protobuf", Authorization: authorization(bundle), Cookie: cookieHeader(bundle, target),
    "User-Agent": USER_AGENT, "X-Goog-Api-Key": publicKey, "X-Goog-Authuser": bundle.authUser,
    "X-User-Agent": "grpc-web-javascript/0.1", "X-Aistudio-Visit-Id": visitId, Origin: ORIGIN, Referer: `${ORIGIN}/`,
    Accept: "*/*", "Accept-Language": "zh-CN,zh;q=0.9,en;q=0.8", "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors", "Sec-Fetch-Site": "same-site" };
}
