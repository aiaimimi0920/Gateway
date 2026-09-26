import { existsSync } from "node:fs";
import net from "node:net";
import { normalizeString } from "./input-text.mjs";

const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const MACOS_EDGE_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];
const LINUX_EDGE_PATHS = [
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];

function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_EDGE_PATHS
      : process.platform === "darwin"
        ? MACOS_EDGE_PATHS
        : LINUX_EDGE_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

function resolveBrowserProxySettings(input) {
  const rawProxy =
    normalizeString(input?.browserProxyUrl) ??
    normalizeString(input?.browser_proxy_url) ??
    normalizeString(input?.browserProxyServer) ??
    normalizeString(input?.browser_proxy_server) ??
    normalizeString(process.env.AISTUDIO_PROBE_BROWSER_PROXY_URL);

  if (!rawProxy || /^none$/i.test(rawProxy)) {
    return null;
  }
  if (/^(direct|no-proxy|no_proxy)$/i.test(rawProxy)) {
    return {
      mode: "direct",
      rawProxy,
      launchProxy: null,
      launchArgs: ["--no-proxy-server"],
    };
  }

  const parsed = new URL(rawProxy);
  const launchProxy = {
    server: `${parsed.protocol}//${parsed.hostname}${parsed.port ? `:${parsed.port}` : ""}`,
  };
  const username = parsed.username ? decodeURIComponent(parsed.username) : null;
  const password = parsed.password ? decodeURIComponent(parsed.password) : null;
  const bypass =
    normalizeString(input?.browserProxyBypass) ??
    normalizeString(input?.browser_proxy_bypass) ??
    normalizeString(input?.browserNoProxy) ??
    normalizeString(input?.browser_no_proxy) ??
    normalizeString(process.env.AISTUDIO_PROBE_BROWSER_PROXY_BYPASS);
  if (username) {
    launchProxy.username = username;
  }
  if (password) {
    launchProxy.password = password;
  }
  if (bypass) {
    launchProxy.bypass = bypass;
  }

  return {
    mode: "proxy",
    rawProxy,
    launchProxy,
    launchArgs: [],
  };
}

function buildBrowserProxyLaunchOptions(browserProxySettings) {
  const options = {
    args: browserProxySettings?.launchArgs ?? [],
  };
  if (browserProxySettings?.launchProxy) {
    options.proxy = browserProxySettings.launchProxy;
  }
  return options;
}

function normalizeProxyHostForReachability(host) {
  const normalized = String(host ?? "").trim().toLowerCase();
  if (normalized.startsWith("[") && normalized.endsWith("]")) {
    return normalized.slice(1, -1);
  }
  return normalized;
}

function isLocalProxyHost(host) {
  return ["127.0.0.1", "localhost", "::1"].includes(
    normalizeProxyHostForReachability(host),
  );
}

function defaultProxyPortForProtocol(protocol) {
  switch (String(protocol ?? "").toLowerCase()) {
    case "http:":
    case "ws:":
      return 80;
    case "https:":
    case "wss:":
      return 443;
    case "socks:":
    case "socks4:":
    case "socks5:":
      return 1080;
    default:
      return null;
  }
}

function buildBrowserProxyUnreachableError({
  host,
  port,
  rawProxy,
  server,
  cause,
}) {
  const causeMessage = cause?.message ? `; cause=${cause.message}` : "";
  const error = new Error(
    `Browser proxy endpoint is not reachable: host=${host}, port=${port}, server=${server}, rawProxy=${rawProxy}${causeMessage}`,
    cause ? { cause } : undefined,
  );
  error.code = "aistudio_browser_proxy_unreachable";
  error.status = 503;
  error.host = host;
  error.port = port;
  error.rawProxy = rawProxy;
  error.server = server;
  return error;
}

async function assertBrowserProxyReachable(
  browserProxySettings,
  timeoutMs = 1500,
) {
  if (
    browserProxySettings?.mode !== "proxy" ||
    !browserProxySettings?.launchProxy?.server
  ) {
    return { ok: true, skipped: true, reason: "browser-proxy-not-configured" };
  }

  const server = browserProxySettings.launchProxy.server;
  const parsed = new URL(server);
  const host = normalizeProxyHostForReachability(parsed.hostname);
  const defaultPort = defaultProxyPortForProtocol(parsed.protocol);
  const port = Number(parsed.port || defaultPort);

  if (!isLocalProxyHost(host)) {
    return { ok: true, skipped: true, reason: "non-local-browser-proxy", host };
  }

  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw buildBrowserProxyUnreachableError({
      host,
      port: parsed.port || null,
      rawProxy: browserProxySettings.rawProxy,
      server,
      cause: new Error("missing or invalid proxy port"),
    });
  }

  const timeout = Math.max(1, Number(timeoutMs) || 1500);
  return await new Promise((resolve, reject) => {
    let settled = false;
    let timer = null;
    const socket = net.createConnection({ host, port });

    const finish = (error) => {
      if (settled) {
        return;
      }
      settled = true;
      if (timer) {
        clearTimeout(timer);
      }
      socket.destroy();
      if (error) {
        reject(
          buildBrowserProxyUnreachableError({
            host,
            port,
            rawProxy: browserProxySettings.rawProxy,
            server,
            cause: error,
          }),
        );
        return;
      }
      resolve({ ok: true, host, port });
    };

    timer = setTimeout(() => {
      finish(new Error(`connect timeout after ${timeout}ms`));
    }, timeout);

    socket.once("connect", () => finish(null));
    socket.once("error", finish);
  });
}

export { resolveExecutablePath, resolveBrowserProxySettings, buildBrowserProxyLaunchOptions, assertBrowserProxyReachable };
