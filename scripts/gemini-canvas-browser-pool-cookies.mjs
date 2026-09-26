import { existsSync, lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createCookieOwner({ log }) {
  function parseCookieHeader(rawHeader) {
    const result = new Map();
    const normalized = normalizeString(rawHeader);
    if (!normalized) {
      return result;
    }
    for (const segment of normalized.split(";")) {
      const trimmed = segment.trim();
      if (!trimmed) {
        continue;
      }
      const separatorIndex = trimmed.indexOf("=");
      if (separatorIndex <= 0) {
        continue;
      }
      const name = trimmed.slice(0, separatorIndex).trim();
      const value = trimmed.slice(separatorIndex + 1).trim();
      if (name) {
        result.set(name, value);
      }
    }
    return result;
  }

  function shouldSyncGeminiCookie(name) {
    return /^(?:COMPASS|NID|SID|HSID|SSID|APISID|SAPISID|SIDCC|__Secure-(?:1|3)PSID(?:TS|CC|RTS)?|__Secure-(?:1|3)PAPISID)$/.test(
      String(name || ""),
    );
  }

  function buildGeminiCookieSyncObjects(rawHeader, baseUrl) {
    const cookieMap = parseCookieHeader(rawHeader);
    if (!cookieMap.size) {
      return [];
    }
    const originUrl = normalizeString(baseUrl) ?? "https://gemini.google.com";
    const cookies = [];
    for (const [name, value] of cookieMap.entries()) {
      if (!name || !value) {
        continue;
      }
      if (!shouldSyncGeminiCookie(name)) {
        continue;
      }
      const secure = name.startsWith("__Secure-") || name.startsWith("__Host-") || originUrl.startsWith("https://");
      if (name.startsWith("__Host-")) {
        cookies.push({
          name,
          value,
          url: originUrl,
          path: "/",
          secure: true,
          httpOnly: false,
        });
        continue;
      }
      cookies.push({
        name,
        value,
        domain: ".google.com",
        path: "/",
        secure,
        httpOnly: false,
      });
    }
    const deduped = [];
    const seen = new Set();
    for (const cookie of cookies) {
      const key = `${cookie.name}|${cookie.domain ?? cookie.url ?? ""}|${cookie.path ?? "/"}`;
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      deduped.push(cookie);
    }
    return deduped;
  }

  async function syncCookieHeaderIntoContext(context, rawHeader, baseUrl) {
    const cookies = buildGeminiCookieSyncObjects(rawHeader, baseUrl);
    if (!cookies.length) {
      return 0;
    }
    try {
      await context.addCookies(cookies);
      return cookies.length;
    } catch (error) {
      let accepted = 0;
      for (const cookie of cookies) {
        const domain = normalizeString(cookie.domain);
        const path = normalizeString(cookie.path) ?? "/";
        const candidates = [
          cookie,
          domain?.startsWith(".")
            ? {
                ...cookie,
                domain: domain.slice(1),
                path,
              }
            : null,
        ].filter(Boolean);
        let synced = false;
        for (const candidate of candidates) {
          try {
            await context.addCookies([candidate]);
            accepted += 1;
            synced = true;
            break;
          } catch {
            // Try the next normalized shape below.
          }
        }
        if (!synced) {
          log(
            "cookie sync skipped invalid cookie",
            JSON.stringify({
              name: cookie.name,
              domain: cookie.domain ?? null,
              hasUrl: Boolean(cookie.url),
              path: cookie.path ?? null,
            }),
          );
        }
      }
      if (accepted > 0) {
        return accepted;
      }
      throw error;
    }
  }

  async function contextHasGeminiAuthCookies(context, baseUrl) {
    try {
      const origin = normalizeString(baseUrl) ?? "https://gemini.google.com";
      const cookies = await context.cookies([origin]);
      return cookies.some((cookie) =>
        /^(?:SID|HSID|SSID|APISID|SAPISID|SIDCC|__Secure-(?:1|3)PSID(?:TS|CC|RTS)?|__Secure-(?:1|3)PAPISID)$/.test(
          String(cookie?.name || ""),
        ),
      );
    } catch {
      return false;
    }
  }

  function embeddedStorageStatePathForProfileDir(profileDir) {
    if (!normalizeString(profileDir)) {
      return null;
    }
    const candidate = path.join(profileDir, "storage-state.json");
    return existsSync(candidate) && lstatSync(candidate).isFile() ? candidate : null;
  }

  function normalizeEmbeddedStorageStateCookie(cookie) {
    if (!cookie || typeof cookie !== "object") {
      return null;
    }
    const name = normalizeString(cookie.name);
    const value = typeof cookie.value === "string" ? cookie.value : null;
    if (!name || value === null) {
      return null;
    }
    const normalized = {
      name,
      value,
      secure: Boolean(cookie.secure),
      httpOnly: Boolean(cookie.httpOnly),
    };
    const sameSite = normalizeString(cookie.sameSite);
    if (sameSite && /^(Lax|None|Strict)$/i.test(sameSite)) {
      normalized.sameSite = sameSite[0].toUpperCase() + sameSite.slice(1).toLowerCase();
    }
    if (Number.isFinite(cookie.expires)) {
      normalized.expires = Number(cookie.expires);
    }
    const url = normalizeString(cookie.url);
    const domain = normalizeString(cookie.domain);
    // Playwright accepts either a URL or a domain/path pair, never both.
    if (url) {
      normalized.url = url;
      return normalized;
    }
    if (domain) {
      normalized.domain = domain;
      normalized.path = normalizeString(cookie.path) ?? "/";
      return normalized;
    }
    return null;
  }

  async function syncEmbeddedStorageStateIntoContext(context, profileDir) {
    const storageStatePath = embeddedStorageStatePathForProfileDir(profileDir);
    if (!storageStatePath) {
      return 0;
    }
    const parsed = JSON.parse(readFileSync(storageStatePath, "utf8"));
    const cookies = Array.isArray(parsed?.cookies)
      ? parsed.cookies
        .map((cookie) => normalizeEmbeddedStorageStateCookie(cookie))
        .filter(Boolean)
      : [];
    if (!cookies.length) {
      return 0;
    }
    try {
      await context.addCookies(cookies);
      return cookies.length;
    } catch (error) {
      let accepted = 0;
      for (const cookie of cookies) {
        const domain = normalizeString(cookie.domain);
        const pathValue = normalizeString(cookie.path) ?? "/";
        const candidates = [
          cookie,
          domain?.startsWith(".")
            ? {
                ...cookie,
                domain: domain.slice(1),
                path: pathValue,
              }
            : null,
        ].filter(Boolean);
        let synced = false;
        for (const candidate of candidates) {
          try {
            await context.addCookies([candidate]);
            accepted += 1;
            synced = true;
            break;
          } catch {
            // Try the next normalized shape below.
          }
        }
        if (!synced) {
          log(
            "embedded storage-state cookie sync skipped invalid cookie",
            JSON.stringify({
              name: cookie.name,
              domain: cookie.domain ?? null,
              hasUrl: Boolean(cookie.url),
              path: cookie.path ?? null,
            }),
          );
        }
      }
      if (accepted > 0) {
        return accepted;
      }
      throw error;
    }
  }

  return { parseCookieHeader, syncCookieHeaderIntoContext, contextHasGeminiAuthCookies, syncEmbeddedStorageStateIntoContext };
}
