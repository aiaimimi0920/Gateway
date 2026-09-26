export const DEFAULT_TIMEOUT_MS = 120_000;
export const DEFAULT_BASE_URL = "https://chatgpt.com";

export function normalizeBaseUrl(value) {
  const text = normalizeString(value) ?? DEFAULT_BASE_URL;
  return text.replace(/\/+$/, "");
}

export function dedupeStrings(values) {
  const unique = [];
  const seen = new Set();
  for (const value of values) {
    const normalized = normalizeString(value);
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    unique.push(normalized);
  }
  return unique;
}

export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function normalizeTimeoutMs(value) {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.min(value, 10 * 60 * 1000);
  }
  return DEFAULT_TIMEOUT_MS;
}

export function parseBoolean(value, fallback) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }
  return fallback;
}

export function resolveHeadlessMode(input) {
  const explicitHeadless = input?.headless ?? process.env.CHATGPT_WEB_SESSION_WORKER_HEADLESS;
  if (typeof explicitHeadless === "boolean") {
    return explicitHeadless;
  }
  if (typeof explicitHeadless === "string") {
    return parseBoolean(explicitHeadless, false);
  }

  // Containerized Linux workers commonly run without an X server. In that case
  // the browser refresh path must default to headless instead of crashing on a
  // headed launch before the retry can even start.
  if (
    process.platform !== "win32" &&
    !normalizeString(process.env.DISPLAY) &&
    !normalizeString(process.env.WAYLAND_DISPLAY)
  ) {
    return true;
  }

  return false;
}

export function resolveProxySettings(input) {
  const rawProxy =
    normalizeString(input?.proxyUrl) ??
    normalizeString(input?.proxy_url) ??
    normalizeString(input?.proxyServer) ??
    normalizeString(input?.proxy_server) ??
    normalizeString(input?.credentialProxyUrl) ??
    normalizeString(input?.credential_proxy_url) ??
    normalizeString(input?.outboundProxy) ??
    normalizeString(input?.outbound_proxy) ??
    normalizeString(process.env.CHATGPT_WEB_BROWSER_PROXY_URL) ??
    normalizeString(process.env.ALL_PROXY) ??
    normalizeString(process.env.all_proxy) ??
    normalizeString(process.env.HTTPS_PROXY) ??
    normalizeString(process.env.https_proxy) ??
    normalizeString(process.env.HTTP_PROXY) ??
    normalizeString(process.env.http_proxy);

  if (!rawProxy || rawProxy.toLowerCase() === "direct") {
    return null;
  }

  try {
    const parsed = new URL(rawProxy);
    const launchOptions = {
      server: `${parsed.protocol}//${parsed.hostname}${parsed.port ? `:${parsed.port}` : ""}`,
    };
    const username = parsed.username ? decodeURIComponent(parsed.username) : null;
    const password = parsed.password ? decodeURIComponent(parsed.password) : null;
    const bypass =
      normalizeString(input?.proxyBypass) ??
      normalizeString(input?.proxy_bypass) ??
      normalizeString(input?.noProxy) ??
      normalizeString(input?.no_proxy) ??
      normalizeString(process.env.CHATGPT_WEB_BROWSER_PROXY_BYPASS) ??
      normalizeString(process.env.NO_PROXY) ??
      normalizeString(process.env.no_proxy);
    if (username) {
      launchOptions.username = username;
    }
    if (password) {
      launchOptions.password = password;
    }
    if (bypass) {
      launchOptions.bypass = bypass;
    }
    return {
      launchOptions,
      rawProxy,
    };
  } catch {
    return null;
  }
}

export function normalizeAuthSeed(value) {
  if (!value || typeof value !== "object") {
    return null;
  }
  const email = normalizeString(value.email ?? value.loginEmail ?? value.login_email);
  const password = normalizeString(value.password ?? value.loginPassword ?? value.login_password);
  const passwordSha256 = normalizeString(
    value.passwordSha256 ?? value.password_sha256 ?? value.passwordHash ?? value.password_hash,
  );
  if (!email || (!password && !passwordSha256)) {
    return null;
  }
  return {
    email,
    password,
    passwordSha256,
  };
}
