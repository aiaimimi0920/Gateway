import { createHash } from "node:crypto";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createGoogleAuthOwner({ parseCookieHeader }) {
  function inferFetchOrigin(baseUrl, url) {
    const preferred = normalizeString(baseUrl);
    if (preferred) {
      return new URL(preferred).origin;
    }
    return new URL(url).origin;
  }

  function inferGoogleAuthUser(pageUrl) {
    const current = normalizeString(pageUrl);
    if (!current) {
      return "0";
    }
    try {
      const parsed = new URL(current);
      const explicit = parsed.searchParams.get("authuser");
      if (explicit && /^\d+$/.test(explicit)) {
        return explicit;
      }
      const match = parsed.pathname.match(/\/u\/(\d+)\b/i);
      if (match?.[1]) {
        return match[1];
      }
    } catch {
      // Ignore malformed page URLs and fall back to the primary account slot.
    }
    return "0";
  }

  async function buildGoogleFetchAuthHeaders(entry, baseUrl, url) {
    const target = new URL(url);
    const hostname = target.hostname.toLowerCase();
    if (!hostname.endsWith("googleapis.com") && !hostname.endsWith("google.com")) {
      return {};
    }

    const origin = inferFetchOrigin(baseUrl, url);
    const cookies = await entry.context.cookies([origin, url]);
    const cookieHeader = cookies.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
    const cookieMap = parseCookieHeader(cookieHeader);
    const sapisid =
      cookieMap.get("__Secure-1PAPISID") ||
      cookieMap.get("__Secure-3PAPISID") ||
      cookieMap.get("SAPISID");
    if (!sapisid) {
      return {};
    }

    const timestamp = Math.floor(Date.now() / 1000);
    const hash = createHash("sha1")
      .update(`${timestamp} ${sapisid} ${origin}`, "utf8")
      .digest("hex");
    const authUser = inferGoogleAuthUser(entry.page?.url?.());
    return {
      Authorization: `SAPISIDHASH ${timestamp}_${hash} SAPISID1PHASH ${timestamp}_${hash} SAPISID3PHASH ${timestamp}_${hash}`,
      "X-Origin": origin,
      "X-Goog-AuthUser": authUser,
    };
  }

  return { inferGoogleAuthUser, buildGoogleFetchAuthHeaders };
}
