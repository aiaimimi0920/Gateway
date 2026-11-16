import { chromium, errors } from "playwright-core";
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
  let browser = null;
  async function generate() {
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
    const deadline = Date.now() + timeoutMs;
    const remaining = () => {
      if (Date.now() >= deadline) throw Object.assign(new Error("Luma Web deadline expired."),
        { code: "lumalabs_browser_timeout" });
      return deadline - Date.now();
    };
    browser = await chromium.launch({
      executablePath,
      timeout: remaining(),
      headless: true,
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--no-first-run",
        "--no-default-browser-check",
      ],
    });

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
      timeout: remaining(),
    });

    const finalUrl = new URL(page.url());
    const expectedUrl = new URL(boardUrl);
    if (finalUrl.origin !== expectedUrl.origin || finalUrl.pathname !== expectedUrl.pathname) {
      return {
        ok: false,
        error: {
          code: "lumalabs_browser_auth_redirect",
          message: "LumaLabs board navigation did not reach the authenticated board.",
          stage: "board",
          status: 401,
        },
      };
    }

    return await page.evaluate(executeLumaOperation, {
      realmId,
      mediaOperation: normalizeString(input.mediaOperation),
      artifactField: normalizeString(input.artifactField) ?? "image",
      locale,
      actionBody: input.actionBody,
      autoDiscoverActionType: Boolean(input.autoDiscoverActionType),
      timeoutMs: remaining(),
      clientCapabilities:
        normalizeString(input.clientCapabilities) ?? DEFAULT_CLIENT_CAPABILITIES,
    });
  }
  let result;
  try { result = await generate(); }
  catch (error) {
    if (error?.code === "lumalabs_browser_timeout" || error instanceof errors.TimeoutError) {
      result = workerFailure("lumalabs_browser_timeout", "LumaLabs browser request deadline expired.", 504);
    } else {
      result = workerFailure(error?.code === "lumalabs_browser_worker_input_too_large"
        ? error.code : "lumalabs_browser_worker_failed", "LumaLabs browser worker execution failed; details omitted.");
    }
  }
  // JSON business failures use exit 0 too: a nonzero OS exit is an execution failure.
  // Never emit success until the owned browser has actually finished closing.
  if (browser) {
    try { await browser.close(); }
    catch { result = workerFailure("lumalabs_browser_cleanup_failed", "LumaLabs browser cleanup failed."); }
  }
  await new Promise((resolve, reject) => process.stdout.write(`${JSON.stringify(result)}\n`,
    (error) => error ? reject(error) : resolve()));
}

function workerFailure(code, message, status = 500) {
  return { ok: false, error: { code, message, stage: "worker", status } };
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
  return Math.max(1, Math.min(parsed, 30 * 60 * 1000));
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

async function readStdin() {
  const chunks = [];
  let total = 0;
  for await (const chunk of process.stdin) {
    total += chunk.length;
    if (total > 1024 * 1024) {
      throw Object.assign(new Error("Worker input exceeded 1 MiB."),
        { code: "lumalabs_browser_worker_input_too_large" });
    }
    chunks.push(chunk);
  }
  return new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks, total));
}

main().catch(() => { process.exitCode = 1; });
