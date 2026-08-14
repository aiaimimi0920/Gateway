/**
 * Captures the authenticated Gemini Web runtime for a selected Google account.
 *
 * The operator signs in and sends the validation prompt in a visible browser.
 * The script captures the matching StreamGenerate request and returns only the
 * runtime material required by the Gateway's Gemini Web reverse adapter.
 */

import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "playwright-core";

export const DEFAULT_TARGET_URL = "https://gemini.google.com/u/1/";
export const VALIDATION_PROMPT = "法国的首都在哪里";
export const STREAM_GENERATE_PATH =
  "/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate";

const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const PRIMARY_COOKIE_NAME = "__Secure-1PSID";
const SECONDARY_COOKIE_NAME = "__Secure-1PSIDTS";
const MODEL_HEADER_NAMES = [
  "x-goog-ext-525001261-jspb",
  "x-goog-ext-73010989-jspb",
  "x-goog-ext-73010990-jspb",
];
const REQUEST_CONTEXT_HEADER_NAME = "x-goog-ext-525005358-jspb";
const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
];
const MACOS_BROWSER_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];
const LINUX_BROWSER_PATHS = [
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
  const candidates =
    process.platform === "win32"
      ? WINDOWS_BROWSER_PATHS
      : process.platform === "darwin"
        ? MACOS_BROWSER_PATHS
        : LINUX_BROWSER_PATHS;
  return candidates.find((entry) => existsSync(entry)) ?? null;
}

export function isGeminiWebStreamGenerateUrl(value) {
  try {
    const url = new URL(value);
    return url.hostname === "gemini.google.com" && url.pathname.endsWith(STREAM_GENERATE_PATH);
  } catch {
    return false;
  }
}

export function requestContainsValidationPrompt(postData, prompt = VALIDATION_PROMPT) {
  const normalized = normalizeString(postData);
  if (!normalized) {
    return false;
  }
  const form = new URLSearchParams(normalized);
  return String(form.get("f.req") ?? "").includes(prompt);
}

export function extractAccountIndex(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    return null;
  }
  try {
    const url = new URL(normalized);
    const queryAccount = normalizeString(url.searchParams.get("authuser"));
    if (queryAccount) {
      return queryAccount;
    }
    const match = /^\/u\/(\d+)(?:\/|$)/.exec(url.pathname);
    return match?.[1] ?? null;
  } catch {
    return null;
  }
}

export function extractGeminiWebRuntime({ requestUrl, postData, headers, cookies, currentUrl }) {
  if (!isGeminiWebStreamGenerateUrl(requestUrl)) {
    throw new Error("Captured request is not a Gemini Web StreamGenerate request.");
  }
  const normalizedHeaders = Object.fromEntries(
    Object.entries(headers ?? {}).map(([name, value]) => [name.toLowerCase(), String(value)]),
  );
  const request = new URL(requestUrl);
  const form = new URLSearchParams(postData ?? "");
  const primaryCookie = (cookies ?? []).find(
    (cookie) => cookie?.name === PRIMARY_COOKIE_NAME && /(^|\.)google\.com$/i.test(cookie.domain ?? ""),
  );
  const secondaryCookie = (cookies ?? []).find(
    (cookie) => cookie?.name === SECONDARY_COOKIE_NAME && /(^|\.)google\.com$/i.test(cookie.domain ?? ""),
  );
  if (!normalizeString(primaryCookie?.value)) {
    throw new Error(`Gemini Web login did not expose ${PRIMARY_COOKIE_NAME}.`);
  }

  const referer = normalizeString(normalizedHeaders.referer) ?? normalizeString(currentUrl);
  const accountIndex =
    normalizeString(normalizedHeaders["x-goog-authuser"])
    ?? extractAccountIndex(requestUrl)
    ?? extractAccountIndex(referer)
    ?? extractAccountIndex(currentUrl);
  const modelHeaders = Object.fromEntries(
    MODEL_HEADER_NAMES.flatMap((name) => {
      const value = normalizeString(normalizedHeaders[name]);
      return value ? [[name, value]] : [];
    }),
  );

  return {
    apiKey: primaryCookie.value,
    authToken: normalizeString(secondaryCookie?.value),
    accountIndex,
    accessToken: normalizeString(form.get("at")),
    buildLabel: normalizeString(request.searchParams.get("bl")),
    sessionId: normalizeString(request.searchParams.get("f.sid")),
    language: normalizeString(request.searchParams.get("hl")) ?? "zh-CN",
    appPagePath: referer ? new URL(referer).pathname : new URL(currentUrl).pathname,
    endpointPath: request.pathname,
    referer,
    modelHeaders,
    requestContextHeader: normalizeString(normalizedHeaders[REQUEST_CONTEXT_HEADER_NAME]),
  };
}

async function main() {
  const executablePath = resolveExecutablePath(
    process.env.GEMINI_WEB_CAPTURE_BROWSER_EXECUTABLE_PATH
      ?? process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH,
  );
  if (!executablePath) {
    throw new Error(
      "Unable to locate Edge/Chrome automatically. Set GEMINI_WEB_CAPTURE_BROWSER_EXECUTABLE_PATH.",
    );
  }
  const targetUrl = normalizeString(process.env.GEMINI_WEB_CAPTURE_TARGET_URL) ?? DEFAULT_TARGET_URL;
  const timeoutMs = Number(process.env.GEMINI_WEB_CAPTURE_TIMEOUT_MS || DEFAULT_TIMEOUT_MS);

  console.log(`[gemini-web-export] Opening browser at ${targetUrl}`);
  console.log(
    `[gemini-web-export] Sign in to account /u/1/, then send exactly: ${VALIDATION_PROMPT}`,
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
      locale: "zh-CN",
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
            `Timed out waiting for the exact prompt '${VALIDATION_PROMPT}' on ${targetUrl}.`,
          ),
        );
      }, timeoutMs);
      const handleResponse = async (response) => {
        const request = response.request();
        if (
          !isGeminiWebStreamGenerateUrl(request.url())
          || !requestContainsValidationPrompt(request.postData())
        ) {
          return;
        }
        try {
          const [headers, cookies, responseBody] = await Promise.all([
            request.allHeaders(),
            context.cookies(),
            response.text().catch(() => ""),
          ]);
          const runtime = extractGeminiWebRuntime({
            requestUrl: request.url(),
            postData: request.postData(),
            headers,
            cookies,
            currentUrl: response.frame()?.url() ?? targetUrl,
          });
          finish(resolve, {
            ...runtime,
            responseStatus: response.status(),
            responseContainsParis: responseBody.includes("巴黎"),
          });
        } catch (error) {
          finish(reject, error);
        }
      };
      context.on("response", handleResponse);
    });

    const page = await context.newPage();
    await page.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: 60_000 });
    const runtime = await capturePromise;
    process.stdout.write(
      `${JSON.stringify(
        {
          ok: true,
          targetFamily: "gemini-web",
          currentUrl: page.url(),
          ...runtime,
          note: "Captured Gemini Web /u/1/ runtime after the exact validation prompt completed.",
        },
        null,
        2,
      )}\n`,
    );
  } finally {
    await browser.close();
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : null;
if (invokedPath && pathToFileURL(invokedPath).href === import.meta.url) {
  main().catch((error) => {
    process.stdout.write(
      `${JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error) }, null, 2)}\n`,
    );
    process.exitCode = 1;
  });
}

