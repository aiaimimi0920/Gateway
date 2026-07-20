import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const DEFAULT_BASE_URL = "https://app.lumalabs.ai";
const DEFAULT_PROFILE_DIRECTORY = "Default";
const DEFAULT_TIMEOUT_MS = 90_000;
const DEFAULT_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
const WINDOWS_EDGE_EXECUTABLES = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const WINDOWS_CHROME_EXECUTABLES = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
];
const WINDOWS_EDGE_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Microsoft",
  "Edge",
  "User Data",
);
const WINDOWS_CHROME_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Google",
  "Chrome",
  "User Data",
);
const ROOT_FILES_TO_COPY = ["Local State"];
const PROFILE_FILES_TO_COPY = ["Preferences", "Secure Preferences"];
// Luma live probing only needs an existing `wos-session` cookie. Copying the
// full Chromium profile makes the worker unreasonably slow on larger real
// profiles, so keep the clone scope to the cookie store plus lightweight prefs.
const PROFILE_DIRS_TO_COPY = ["Network"];

async function main() {
  let clonedUserDataDir = null;
  let context = null;

  try {
    const input = JSON.parse(await readStdin());
    const baseUrl = normalizeBaseUrl(input.baseUrl ?? DEFAULT_BASE_URL);
    const realmId = normalizeString(input.realmId);
    if (!realmId) {
      throw createWorkerError(
        400,
        "lumalabs_realm_id_missing",
        "LumaLabs session worker requires a realmId so it can probe an existing board.",
      );
    }
    const timeoutMs = normalizeTimeoutMs(input.timeoutMs);
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ??
        process.env.LUMALABS_BROWSER_EXECUTABLE_PATH ??
        process.env.QWEN_WEB_BROWSER_EXECUTABLE_PATH ??
        process.env.PRODUCER_BROWSER_EXECUTABLE_PATH ??
        null,
    );
    if (!executablePath) {
      throw createWorkerError(
        500,
        "lumalabs_browser_not_found",
        "Unable to locate a Chromium-compatible browser for the LumaLabs session worker.",
      );
    }

    const profileSource = resolveProfileSource(input);
    clonedUserDataDir = await cloneBrowserProfile(
      profileSource.userDataDir,
      profileSource.profileDirectory,
    );

    context = await chromium.launchPersistentContext(clonedUserDataDir, {
      executablePath,
      headless: parseBoolean(process.env.LUMALABS_SESSION_WORKER_HEADLESS, false),
      locale: "zh-CN",
      userAgent: normalizeString(input.userAgent) ?? DEFAULT_USER_AGENT,
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--no-first-run",
        "--no-default-browser-check",
        `--profile-directory=${profileSource.profileDirectory}`,
      ],
    });

    const page = context.pages()[0] ?? (await context.newPage());
    const boardUrl = `${baseUrl}/board/${realmId}`;
    await page.goto(boardUrl, {
      waitUntil: "domcontentloaded",
      timeout: Math.min(timeoutMs, 60_000),
    });
    await page.waitForTimeout(2_500);

    const finalUrl = page.url();
    const cookies = await context.cookies(baseUrl);
    const cookieHeader = cookies
      .map((cookie) => `${cookie.name}=${cookie.value}`)
      .filter(Boolean)
      .join("; ");
    const sessionCookie = cookies.find((cookie) => cookie.name === "wos-session") ?? null;
    const pageProbe = await page.evaluate(() => {
      const trim = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const title = trim(document.title);
      const heading =
        trim(document.querySelector("h1")?.textContent ?? null) ??
        trim(document.querySelector("[role='heading']")?.textContent ?? null);
      const bodySnippet = trim(document.body?.innerText ?? "")?.slice(0, 400) ?? null;
      const storageKeys = Object.keys(localStorage);
      let accountName = null;
      for (const key of storageKeys) {
        const lowered = key.toLowerCase();
        if (!lowered.includes("user") && !lowered.includes("account")) {
          continue;
        }
        const raw = trim(localStorage.getItem(key));
        if (!raw) {
          continue;
        }
        try {
          const parsed = JSON.parse(raw);
          if (parsed && typeof parsed === "object") {
            accountName =
              trim(parsed.name) ??
              trim(parsed.email) ??
              trim(parsed.username) ??
              trim(parsed.id) ??
              accountName;
          }
        } catch {
          accountName ??= raw;
        }
      }
      return {
        title,
        heading,
        bodySnippet,
        accountName,
        storageKeys,
      };
    });

    if (!finalUrl.includes(`/board/${realmId}`)) {
      throw createWorkerError(
        401,
        "lumalabs_board_auth_redirect",
        `LumaLabs board navigation redirected to ${finalUrl}. The worker only reuses an existing signed-in browser session; it does not sign in or register accounts.`,
      );
    }
    if (!sessionCookie?.value?.trim()) {
      throw createWorkerError(
        401,
        "lumalabs_session_missing_cookie",
        "The cloned browser profile did not expose a usable `wos-session` cookie for LumaLabs.",
      );
    }

    const result = {
      ok: true,
      baseUrl,
      browserExecutablePath: executablePath,
      profileSource: {
        browser: profileSource.browser,
        userDataDir: profileSource.userDataDir,
        profileDirectory: profileSource.profileDirectory,
      },
      realmId,
      boardUrl,
      finalUrl,
      sessionToken: sessionCookie.value.trim(),
      cookieHeader,
      expiresAt:
        typeof sessionCookie.expires === "number" && sessionCookie.expires > 0
          ? new Date(sessionCookie.expires * 1000).toISOString()
          : null,
      accountName: pageProbe.accountName ?? null,
      title: pageProbe.title ?? null,
      heading: pageProbe.heading ?? null,
      bodySnippet: pageProbe.bodySnippet ?? null,
      localStorageKeys: Array.isArray(pageProbe.storageKeys) ? pageProbe.storageKeys : [],
    };
    printAndExit(result);
  } catch (error) {
    printAndExit(
      {
        ok: false,
        error: serializeError(error),
      },
      1,
    );
  } finally {
    await context?.close().catch(() => {});
    if (clonedUserDataDir) {
      await rm(clonedUserDataDir, { recursive: true, force: true }).catch(() => {});
    }
  }
}

function normalizeBaseUrl(value) {
  const text = normalizeString(value) ?? DEFAULT_BASE_URL;
  return text.replace(/\/+$/, "");
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeTimeoutMs(value) {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.min(value, 10 * 60 * 1000);
  }
  return DEFAULT_TIMEOUT_MS;
}

function parseBoolean(value, fallback) {
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

function resolveExecutablePath(configured) {
  const candidates = [];
  if (normalizeString(configured)) {
    candidates.push(configured);
  }
  if (process.platform === "win32") {
    candidates.push(...WINDOWS_EDGE_EXECUTABLES, ...WINDOWS_CHROME_EXECUTABLES);
  }
  return candidates.find((candidate) => candidate && existsSync(candidate)) ?? null;
}

function resolveProfileSource(input) {
  const configuredUserDataDir =
    normalizeString(input?.userDataDir) ??
    normalizeString(process.env.LUMALABS_BROWSER_USER_DATA_DIR) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_USER_DATA_DIR);
  const profileDirectory =
    normalizeString(input?.profileDirectory) ??
    normalizeString(process.env.LUMALABS_BROWSER_PROFILE_DIR) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_PROFILE_DIR) ??
    DEFAULT_PROFILE_DIRECTORY;
  if (configuredUserDataDir && existsSync(configuredUserDataDir)) {
    return {
      browser: inferBrowserName(configuredUserDataDir),
      userDataDir: configuredUserDataDir,
      profileDirectory,
    };
  }
  if (process.platform === "win32" && existsSync(WINDOWS_EDGE_USER_DATA_DIR)) {
    return {
      browser: "edge",
      userDataDir: WINDOWS_EDGE_USER_DATA_DIR,
      profileDirectory,
    };
  }
  if (process.platform === "win32" && existsSync(WINDOWS_CHROME_USER_DATA_DIR)) {
    return {
      browser: "chrome",
      userDataDir: WINDOWS_CHROME_USER_DATA_DIR,
      profileDirectory,
    };
  }
  throw createWorkerError(
    500,
    "lumalabs_profile_not_found",
    "Unable to locate a Chromium user data directory for the LumaLabs session worker. Provide an existing signed-in browser profile.",
  );
}

function inferBrowserName(userDataDir) {
  const lower = userDataDir.toLowerCase();
  if (lower.includes("\\edge\\") || lower.includes("/edge/")) {
    return "edge";
  }
  if (lower.includes("\\chrome\\") || lower.includes("/chrome/")) {
    return "chrome";
  }
  return "chromium";
}

async function cloneBrowserProfile(userDataDir, profileDirectory) {
  const sourceProfileDir = path.join(userDataDir, profileDirectory);
  if (!existsSync(userDataDir) || !existsSync(sourceProfileDir)) {
    throw createWorkerError(
      500,
      "lumalabs_profile_missing",
      `Browser profile not found at ${sourceProfileDir}. LumaLabs refresh requires an existing signed-in browser profile.`,
    );
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "lumalabs-profile-"));
  await mkdir(tempRoot, { recursive: true });

  for (const fileName of ROOT_FILES_TO_COPY) {
    await copyIfExists(path.join(userDataDir, fileName), path.join(tempRoot, fileName));
  }

  const clonedProfileDir = path.join(tempRoot, profileDirectory);
  await mkdir(clonedProfileDir, { recursive: true });
  for (const fileName of PROFILE_FILES_TO_COPY) {
    await copyIfExists(path.join(sourceProfileDir, fileName), path.join(clonedProfileDir, fileName));
  }
  for (const directoryName of PROFILE_DIRS_TO_COPY) {
    await copyRecursive(
      path.join(sourceProfileDir, directoryName),
      path.join(clonedProfileDir, directoryName),
    );
  }

  return tempRoot;
}

async function copyIfExists(source, destination) {
  if (!existsSync(source)) {
    return;
  }
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    if (shouldIgnoreLockedProfileFile(error)) {
      return;
    }
    throw error;
  }
}

async function copyRecursive(source, destination) {
  if (!existsSync(source)) {
    return;
  }
  const sourceStat = await stat(source);
  if (sourceStat.isDirectory()) {
    await mkdir(destination, { recursive: true });
    const entries = await readdir(source, { withFileTypes: true });
    for (const entry of entries) {
      await copyRecursive(path.join(source, entry.name), path.join(destination, entry.name));
    }
    return;
  }
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(source, destination);
  } catch (error) {
    if (shouldIgnoreLockedProfileFile(error)) {
      return;
    }
    throw error;
  }
}

function shouldIgnoreLockedProfileFile(error) {
  const code = normalizeString(error?.code)?.toUpperCase();
  return code === "EBUSY" || code === "EPERM";
}

function createWorkerError(status, code, message) {
  const error = new Error(message);
  error.status = status;
  error.code = code;
  return error;
}

function serializeError(error) {
  return {
    status: Number.isFinite(error?.status) ? error.status : 500,
    code: normalizeString(error?.code) ?? "lumalabs_session_worker_failed",
    message:
      normalizeString(error?.message) ??
      "LumaLabs session worker failed without a structured error message.",
  };
}

async function readStdin() {
  return new Promise((resolve, reject) => {
    let buffer = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      buffer += chunk;
    });
    process.stdin.on("end", () => resolve(buffer.trim() || "{}"));
    process.stdin.on("error", reject);
  });
}

function printAndExit(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exit(exitCode);
}

await main();
