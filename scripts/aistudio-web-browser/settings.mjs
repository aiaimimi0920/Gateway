import { existsSync } from "node:fs";

export const DEFAULT_LOCAL_WS_PORT = 9998;
export const DEFAULT_TIMEOUT_MS = 180_000;

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

export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
  if (!normalized) return fallback;
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  return fallback;
}

export function resolveExecutablePath(overridePath) {
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
