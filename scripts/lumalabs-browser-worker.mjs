import { chromium } from "playwright-core";
import { executeLumaOperation } from "./lumalabs-session/browser-operation.mjs";
import { existsSync } from "node:fs";

const DEFAULT_TIMEOUT_MS = 120_000;
const DEFAULT_GEO = "SG";
const DEFAULT_LOCALE = "zh-CN";
const DEFAULT_CLIENT_CAPABILITIES = "retry,upgrade_plan";
const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const MACOS_EDGE_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
];
const LINUX_EDGE_PATHS = [
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];

async function main() {
  try {
    const raw = await readStdin();
    const input = JSON.parse(raw);
    validateInput(input);

    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ?? process.env.LUMALABS_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!executablePath) {
      throw new Error(
        "Unable to locate a Chromium-compatible browser. Set LUMALABS_BROWSER_EXECUTABLE_PATH to a local Edge/Chromium executable.",
      );
    }

    const timeoutMs = normalizeTimeoutMs(input.timeoutMs);
    const browser = await chromium.launch({
      executablePath,
      headless: true,
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--no-first-run",
        "--no-default-browser-check",
      ],
    });

    try {
      const locale = normalizeString(input.locale) ?? DEFAULT_LOCALE;
      const baseUrl = normalizeBaseUrl(input.baseUrl);
      const realmId = input.realmId.trim();
      const boardUrl = `${baseUrl}/board/${realmId}`;
      const context = await browser.newContext({
        locale,
        userAgent: normalizeString(input.userAgent) ?? undefined,
        extraHTTPHeaders: {
          "x-client-capabilities":
            normalizeString(input.clientCapabilities) ?? DEFAULT_CLIENT_CAPABILITIES,
        },
      });

      await context.addCookies([
        {
          name: "wos-session",
          value: input.sessionToken,
          url: `${baseUrl}/`,
          httpOnly: true,
          secure: true,
          sameSite: "Lax",
        },
        {
          name: "_geo",
          value: normalizeString(input.geo) ?? DEFAULT_GEO,
          url: `${baseUrl}/`,
          secure: true,
          sameSite: "Lax",
        },
        {
          name: "user-logged-in",
          value: "true",
          url: `${baseUrl}/`,
          secure: true,
          sameSite: "Lax",
        },
      ]);

      const page = await context.newPage();
      await page.goto(boardUrl, {
        waitUntil: "domcontentloaded",
        timeout: timeoutMs,
      });

      const finalUrl = page.url();
      if (!finalUrl.includes(`/board/${realmId}`)) {
        printJsonAndExit({
          ok: false,
          error: {
            code: "lumalabs_browser_auth_redirect",
            message: `LumaLabs board navigation redirected to ${finalUrl}.`,
            stage: "board",
            status: 401,
          },
        });
      }

      const result = await page.evaluate(
        executeLumaOperation,
        {
          realmId,
          mediaOperation: normalizeString(input.mediaOperation),
          artifactField: normalizeString(input.artifactField) ?? "image",
          locale,
          actionBody: input.actionBody,
          autoDiscoverActionType: Boolean(input.autoDiscoverActionType),
          timeoutMs,
          clientCapabilities:
            normalizeString(input.clientCapabilities) ?? DEFAULT_CLIENT_CAPABILITIES,
        },
      );

      printJsonAndExit(result);
    } finally {
      await browser.close().catch(() => {});
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    printJsonAndExit({
      ok: false,
      error: {
        code: "lumalabs_browser_worker_failed",
        message,
        stage: "worker",
        status: 500,
      },
    });
  }
}

function validateInput(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new Error("Worker input must be a JSON object.");
  }
  for (const field of ["baseUrl", "realmId", "sessionToken", "actionBody"]) {
    if (!(field in input)) {
      throw new Error(`Missing required worker input field '${field}'.`);
    }
  }
  if (typeof input.sessionToken !== "string" || !input.sessionToken.trim()) {
    throw new Error("sessionToken must be a non-empty string.");
  }
  if (typeof input.realmId !== "string" || !input.realmId.trim()) {
    throw new Error("realmId must be a non-empty string.");
  }
  if (!input.actionBody || typeof input.actionBody !== "object" || Array.isArray(input.actionBody)) {
    throw new Error("actionBody must be a JSON object.");
  }
}

function normalizeTimeoutMs(value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return DEFAULT_TIMEOUT_MS;
  }
  return Math.max(5_000, Math.min(parsed, 300_000));
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeBaseUrl(value) {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error("baseUrl must be a non-empty string.");
  }
  return value.trim().replace(/\/+$/, "");
}

function resolveExecutablePath(explicitPath) {
  const candidate = normalizeString(explicitPath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }

  const paths =
    process.platform === "win32"
      ? WINDOWS_EDGE_PATHS
      : process.platform === "darwin"
        ? MACOS_EDGE_PATHS
        : LINUX_EDGE_PATHS;
  return paths.find((entry) => existsSync(entry)) ?? null;
}

function readStdin() {
  return new Promise((resolve, reject) => {
    let buffer = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      buffer += chunk;
    });
    process.stdin.on("end", () => resolve(buffer));
    process.stdin.on("error", reject);
  });
}

function printJsonAndExit(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exit(payload?.ok ? 0 : 1);
}

main();
