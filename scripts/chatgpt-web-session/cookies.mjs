import { normalizeString } from "./configuration.mjs";

export async function primeBrowserWithImportedCookies(context, baseUrl, input) {
  const cookies = [];
  const fromHeader = parseCookieHeader(
    normalizeString(input?.cookieHeader) ?? normalizeString(input?.headers?.Cookie),
    new URL(baseUrl).hostname,
  );
  cookies.push(...fromHeader);
  if (Array.isArray(input?.sessionCookies)) {
    for (const entry of input.sessionCookies) {
      const normalized = normalizeCookie(entry, new URL(baseUrl).hostname);
      if (normalized) {
        cookies.push(normalized);
      }
    }
  }
  if (cookies.length > 0) {
    await context.addCookies(dedupeCookies(cookies)).catch(() => {});
  }
}

export async function collectCookieHeader(context, baseUrl) {
  const cookies = await context.cookies([baseUrl]).catch(() => []);
  if (!Array.isArray(cookies) || cookies.length === 0) {
    return null;
  }
  return cookies
    .filter((cookie) => normalizeString(cookie?.name))
    .map((cookie) => `${cookie.name}=${cookie.value ?? ""}`)
    .join("; ");
}

export function findCookieValue(cookieHeader, name) {
  const target = String(name ?? "").trim();
  if (!cookieHeader || !target) {
    return null;
  }
  for (const entry of String(cookieHeader).split(";")) {
    const [cookieName, ...rest] = entry.split("=");
    if (cookieName?.trim() === target) {
      return rest.join("=").trim() || null;
    }
  }
  return null;
}

export function parseCookieNames(cookieHeader) {
  return String(cookieHeader ?? "")
    .split(";")
    .map((entry) => entry.trim().split("=", 1)[0])
    .filter(Boolean);
}

export function parseCookieHeader(cookieHeader, host) {
  const header = normalizeString(cookieHeader);
  if (!header) {
    return [];
  }
  return header
    .split(";")
    .map((entry) => {
      const [name, ...rest] = entry.split("=");
      const trimmedName = name?.trim();
      if (!trimmedName) {
        return null;
      }
      return {
        name: trimmedName,
        value: rest.join("=").trim(),
        domain: host,
        path: "/",
        secure: true,
        httpOnly: false,
        sameSite: "Lax",
      };
    })
    .filter(Boolean);
}

export function normalizeCookie(cookie, defaultHost) {
  if (!cookie || typeof cookie !== "object") {
    return null;
  }
  const name = normalizeString(cookie.name);
  if (!name) {
    return null;
  }
  const domain =
    normalizeString(cookie.domain)?.replace(/^\./, "") ??
    normalizeString(defaultHost);
  if (!domain) {
    return null;
  }
  return {
    name,
    value: String(cookie.value ?? ""),
    domain,
    path: normalizeString(cookie.path) ?? "/",
    secure: cookie.secure !== false,
    httpOnly: cookie.httpOnly === true,
    expires:
      typeof cookie.expires === "number" && Number.isFinite(cookie.expires)
        ? cookie.expires
        : undefined,
    sameSite:
      cookie.sameSite === "Strict" || cookie.sameSite === "None"
        ? cookie.sameSite
        : "Lax",
  };
}

export function dedupeCookies(cookies) {
  const map = new Map();
  for (const cookie of cookies) {
    const key = `${cookie.name}|${cookie.domain}|${cookie.path}`;
    map.set(key, cookie);
  }
  return [...map.values()];
}

export function decodeJwtExpIso(token) {
  const normalized = normalizeString(token);
  if (!normalized) {
    return null;
  }
  try {
    const [, payload] = normalized.split(".");
    if (!payload) {
      return null;
    }
    const base64 = payload.replace(/-/g, "+").replace(/_/g, "/");
    const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, "=");
    const text = Buffer.from(padded, "base64").toString("utf8");
    const decoded = JSON.parse(text);
    return typeof decoded.exp === "number"
      ? new Date(decoded.exp * 1000).toISOString()
      : null;
  } catch {
    return null;
  }
}
