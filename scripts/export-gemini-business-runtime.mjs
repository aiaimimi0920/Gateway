/**
 * Manual helper for Gemini Business credential bootstrap.
 *
 * The helper opens a visible Chromium/Edge browser on business.gemini.google,
 * waits for the operator to sign in and trigger one real Gemini Business
 * request, then extracts the runtime material directly from the intercepted
 * widget API request:
 *   - Authorization bearer JWT
 *   - configId
 *   - session
 */

import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const DEFAULT_BASE_URL = "https://business.gemini.google";
const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;

const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
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

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

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

function normalizeBearerHeader(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    return null;
  }
  return normalized.replace(/^Bearer\s+/i, "").trim() || null;
}

function normalizeSessionName(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    return null;
  }
  return normalized;
}

function extractRuntimeFromRequest(request) {
  const url = new URL(request.url());
  if (!url.hostname.includes("biz-discoveryengine.googleapis.com")) {
    return null;
  }
  if (
    !url.pathname.endsWith("/widgetStreamAssist") &&
    !url.pathname.endsWith("/widgetAddContextFile")
  ) {
    return null;
  }
  const headers = request.headers();
  const jwt = normalizeBearerHeader(headers.authorization ?? headers.Authorization);
  if (!jwt) {
    return null;
  }

  let body = null;
  try {
    body = request.postDataJSON();
  } catch {
    body = null;
  }
  if (!body || typeof body !== "object") {
    return null;
  }

  const configId = normalizeString(body.configId);
  const session =
    normalizeSessionName(body?.streamAssistRequest?.session) ??
    normalizeSessionName(body?.addContextFileRequest?.name) ??
    normalizeSessionName(body?.sessionInfo?.session);
  if (!configId || !session) {
    return null;
  }

  return {
    jwt,
    configId,
    session,
    sourceUrl: request.url(),
    operation: url.pathname.endsWith("/widgetStreamAssist")
      ? "widgetStreamAssist"
      : "widgetAddContextFile",
  };
}

function summarizeResponseStructure(bodyText) {
  const summary = {
    format: "text",
    rootKind: null,
    topLevelKeys: [],
    itemCount: null,
  };
  try {
    const parsed = JSON.parse(bodyText);
    summary.format = "json";
    summary.rootKind = Array.isArray(parsed) ? "array" : typeof parsed;
    summary.itemCount = Array.isArray(parsed) ? parsed.length : null;
    summary.topLevelKeys =
      parsed && !Array.isArray(parsed) && typeof parsed === "object" ? Object.keys(parsed) : [];
    return summary;
  } catch {}

  const nonEmptyLines = String(bodyText || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const parsedLines = nonEmptyLines
    .map((line) => {
      try {
        return JSON.parse(line.replace(/^data:\s*/, ""));
      } catch {
        return null;
      }
    })
    .filter((value) => value !== null);
  if (parsedLines.length > 0) {
    summary.format = nonEmptyLines.some((line) => line.startsWith("data:")) ? "sse" : "ndjson";
    summary.rootKind = "stream";
    summary.itemCount = parsedLines.length;
  }
  return summary;
}

async function main() {
  const executablePath = resolveExecutablePath(
    process.env.GEMINI_BUSINESS_CAPTURE_BROWSER_EXECUTABLE_PATH ?? null,
  );
  if (!executablePath) {
    throw new Error(
      "Unable to locate Edge/Chrome automatically. Set GEMINI_BUSINESS_CAPTURE_BROWSER_EXECUTABLE_PATH.",
    );
  }
  const baseUrl = normalizeString(process.env.GEMINI_BUSINESS_CAPTURE_BASE_URL) ?? DEFAULT_BASE_URL;
  const timeoutMs = Number(process.env.GEMINI_BUSINESS_CAPTURE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS);
  const targetUrl = `${baseUrl.replace(/\/+$/, "")}/`;

  console.log(`[gemini-business-export] Opening browser at ${targetUrl}`);
  console.log(
    "[gemini-business-export] Complete Gemini Business login, then send one text prompt so the widgetStreamAssist response can be captured.",
  );

  const browser = await chromium.launch({
    executablePath,
    headless: false,
    args: [
      "--disable-blink-features=AutomationControlled",
      "--disable-dev-shm-usage",
      "--no-first-run",
      "--no-default-browser-check",
      "--start-maximized",
    ],
  });

  try {
    const context = await browser.newContext({
      viewport: null,
      locale: "en-US",
      ignoreHTTPSErrors: true,
      bypassCSP: true,
    });
    const capturePromise = new Promise((resolve, reject) => {
      let settled = false;
      const finish = (callback, value) => {
        if (settled) {
          return;
        }
        settled = true;
        clearTimeout(timeout);
        context.off("response", handleResponse);
        callback(value);
      };
      const timeout = setTimeout(() => {
        finish(
          reject,
          new Error(
            "Timed out waiting for a Gemini Business widgetStreamAssist response. After login, submit one text prompt in the opened browser window.",
          ),
        );
      }, timeoutMs);
      const handleResponse = async (response) => {
        const runtime = extractRuntimeFromRequest(response.request());
        if (!runtime || runtime.operation !== "widgetStreamAssist") {
          return;
        }
        try {
          const bodyText = await response.text();
          finish(resolve, {
            ...runtime,
            responseStatus: response.status(),
            responseContentType: response.headers()["content-type"] ?? null,
            responseBodyText: bodyText,
          });
        } catch (error) {
          finish(
            reject,
            new Error(
              `Failed to capture the Gemini Business widget response: ${error instanceof Error ? error.message : String(error)}`,
            ),
          );
        }
      };
      context.on("response", handleResponse);
    });

    const page = await context.newPage();
    await page.goto(targetUrl, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });

    const runtime = await capturePromise;
    const captureDir = path.resolve(process.cwd(), ".runtime", "gemini-business-captures");
    await mkdir(captureDir, { recursive: true });
    const responseCapturePath = path.join(captureDir, `widget-stream-assist-${Date.now()}.txt`);
    await writeFile(responseCapturePath, runtime.responseBodyText, "utf8");
    const responseSummary = summarizeResponseStructure(runtime.responseBodyText);
    console.log(
      JSON.stringify(
        {
          ok: true,
          jwt: runtime.jwt,
          configId: runtime.configId,
          session: runtime.session,
          sourceUrl: runtime.sourceUrl,
          currentUrl: page.url(),
          responseStatus: runtime.responseStatus,
          responseContentType: runtime.responseContentType,
          responseBytes: Buffer.byteLength(runtime.responseBodyText, "utf8"),
          responseCapturePath,
          responseSummary,
          note: "Captured runtime material and a local response fixture from an intercepted Gemini Business widget request.",
        },
        null,
        2,
      ),
    );
  } finally {
    await browser.close().catch(() => undefined);
  }
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
