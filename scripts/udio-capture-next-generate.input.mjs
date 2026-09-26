import { writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

export const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
export const UDIO_DEFAULT_ORIGIN = "https://www.udio.com";

export function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function normalizeTimeoutMs(value, fallback, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(Math.max(Math.trunc(parsed), min), max);
}

export function normalizeUrlPath(rawUrl, baseUrl = UDIO_DEFAULT_ORIGIN) {
  const value = normalizeString(rawUrl);
  if (!value) {
    return null;
  }
  try {
    return new URL(value, baseUrl).pathname;
  } catch {
    return null;
  }
}

export function isGenerateProxySubmit(rawUrl, method) {
  return (
    String(method ?? "").trim().toUpperCase() === "POST" &&
    normalizeUrlPath(rawUrl) === "/api/generate-proxy"
  );
}

async function ensureParentDir(filePath) {
  const fs = await import("node:fs/promises");
  await fs.mkdir(path.dirname(filePath), { recursive: true });
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

export function parseCliArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const entry = argv[index];
    if (!entry?.startsWith("--")) {
      continue;
    }
    const key = entry.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

export async function resolveInput() {
  const raw = (await readStdin()).replace(/^\uFEFF/, "").trim();
  if (raw) {
    return JSON.parse(raw);
  }

  const cli = parseCliArgs(process.argv.slice(2));
  return {
    browserCdpUrl: cli["browser-cdp-url"],
    browserCdpTargetUrl: cli["browser-cdp-target-url"],
    outputPath: cli["output-file"],
    readyPath: cli["ready-file"],
    statusPath: cli["status-file"],
    timeoutMs: cli["timeout-ms"],
    browserCdpConnectTimeoutMs: cli["browser-cdp-connect-timeout-ms"],
  };
}

export async function writeJson(filePath, value) {
  const fs = await import("node:fs/promises");
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, JSON.stringify(value, null, 2), "utf8");
}

export async function writeOptionalJson(filePath, value) {
  const normalized = normalizeString(filePath);
  if (!normalized) {
    return;
  }
  await ensureParentDir(normalized);
  await writeJson(normalized, value);
}

export function withTimeout(promise, timeoutMs, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}
