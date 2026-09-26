import { normalizeString } from "./request.mjs";
import { cookieMatchesOrigin, runtimeStateOrigin } from "./state-scope.mjs";

async function resolveUdioAccessToken(page, baseUrl) {
  const target = runtimeStateOrigin(baseUrl);
  const context = page.context();
  let cookies = await context.cookies([`${baseUrl}/`]).catch(() => []);
  // Some CDP-backed Chromium contexts return an empty URL-filtered cookie set
  // even though the provider cookies are present. Fall back to the complete
  // context cookie jar, then enforce provider domain and exact auth-cookie names.
  if (!cookies.length) {
    cookies = await context.cookies().catch(() => []);
  }
  if (!cookies.length) {
    const session = await context.newCDPSession(page).catch(() => null);
    if (session) {
      try {
        const result = await session.send("Network.getAllCookies");
        cookies = Array.isArray(result?.cookies) ? result.cookies : [];
      } finally {
        await session.detach().catch(() => undefined);
      }
    }
  }
  const tokenFromCookie = extractSupabaseAccessTokenFromCookies(
    cookies.filter((cookie) => cookieMatchesOrigin(cookie, target)),
  );
  if (tokenFromCookie) {
    return tokenFromCookie;
  }

  return await page.evaluate((expectedOrigin) => {
    try {
      if (window.location.origin !== expectedOrigin) return null;
      for (let index = 0; index < localStorage.length; index += 1) {
        const key = localStorage.key(index);
        if (!key) continue;
        const lower = key.toLowerCase();
        if (!lower.includes("supabase") && !lower.includes("auth")) {
          continue;
        }
        const raw = localStorage.getItem(key);
        if (!raw) continue;
        try {
          const parsed = JSON.parse(raw);
          const token =
            parsed?.access_token ??
            parsed?.currentSession?.access_token ??
            parsed?.session?.access_token ??
            parsed?.data?.session?.access_token;
          if (typeof token === "string" && token.trim()) {
            return token.trim();
          }
        } catch {
          // Ignore non-JSON auth cache entries.
        }
      }
    } catch {
      // Ignore localStorage access failures.
    }
    return null;
  }, target.origin);
}

function extractSupabaseAccessTokenFromCookies(cookies) {
  const prefix = "sb-ssr-production-auth-token";
  const parts = cookies
    .filter((cookie) => typeof cookie?.name === "string" && cookie.name.startsWith(prefix))
    .map((cookie) => ({
      index: cookie.name === prefix ? 0 : parseCookiePartIndex(cookie.name),
      value: String(cookie.value ?? ""),
    }))
    .filter((entry) => Number.isInteger(entry.index))
    .sort((left, right) => left.index - right.index);

  if (!parts.length || parts.some((part, index) => part.index !== index)) {
    return null;
  }

  const joined = parts.map((entry) => entry.value).join("");
  return extractAccessTokenFromSupabaseCookie(joined);
}

function parseCookiePartIndex(name) {
  const match = name.match(/^sb-ssr-production-auth-token\.(0|[1-9]\d*)$/);
  const parsed = match ? Number(match[1]) : Number.NaN;
  return Number.isSafeInteger(parsed) ? parsed : Number.NaN;
}

function extractAccessTokenFromSupabaseCookie(rawValue) {
  const normalized = normalizeString(rawValue);
  if (!normalized) {
    return null;
  }

  let payload = normalized;
  if (payload.startsWith("base64-")) {
    payload = payload.slice("base64-".length);
    payload = Buffer.from(normalizeBase64(payload), "base64").toString("utf8");
  }

  try {
    const parsed = JSON.parse(payload);
    const token =
      parsed?.access_token ??
      parsed?.accessToken ??
      parsed?.currentSession?.access_token ??
      parsed?.session?.access_token;
    return typeof token === "string" && token.trim() ? token.trim() : null;
  } catch {
    return null;
  }
}

function normalizeBase64(value) {
  const normalized = String(value ?? "").replace(/-/g, "+").replace(/_/g, "/");
  const remainder = normalized.length % 4;
  if (remainder === 0) {
    return normalized;
  }
  return normalized.padEnd(normalized.length + (4 - remainder), "=");
}


function parseCookieHeader(rawHeader, baseUrl) {
  if (!rawHeader) {
    return [];
  }
  const cookies = [];
  for (const segment of rawHeader.split(";")) {
    const raw = segment.trim();
    if (!raw) {
      continue;
    }
    const separator = raw.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    const name = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim();
    if (!name || !value) {
      continue;
    }
    cookies.push({
      name,
      value,
      url: `${baseUrl}/`,
      secure: true,
      sameSite: "Lax",
      httpOnly: name.startsWith("sb-"),
    });
  }
  return cookies;
}

export { resolveUdioAccessToken, extractSupabaseAccessTokenFromCookies, parseCookieHeader };

