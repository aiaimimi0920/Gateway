import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { probeQwenPage } from "./qwen-web-session/page-probe.mjs";
import { loginExistingAccount as loginAccount } from "./qwen-web-session/login.mjs";
import { maybeWriteCredentialFile as writeCredential } from "./qwen-web-session/credentials.mjs";

// This worker only refreshes/imports an existing signed-in Qwen Web session.
// It does not register accounts, solve activation flows, or orchestrate login/signup.

const DEFAULT_BASE_URL = "https://chat.qwen.ai";
const DEFAULT_PROFILE_DIRECTORY = "Default";
const DEFAULT_TIMEOUT_MS = 90_000;
const DEFAULT_ACCEPT_LANGUAGE = "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7";
const DEFAULT_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
const DEFAULT_PREFERRED_MODELS = [
  "qwen3-coder-plus",
  "qwen3-plus",
  "qwen-plus",
  "qwen3-max-preview",
  "qwen3-max",
];
const DEFAULT_CREDENTIAL_FAMILY_DIR = "qwen-web-chat";
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
const PROFILE_DIRS_TO_COPY = [
  "Network",
  "Local Storage",
  "Session Storage",
  "IndexedDB",
  "WebStorage",
  "Service Worker",
  "Shared Dictionary",
];

async function main() {
  let clonedUserDataDir = null;
  let context = null;

  try {
    const input = JSON.parse(await readStdin());
    const authSeed = normalizeAuthSeed(input?.authSeed);
    const baseUrl = normalizeBaseUrl(input.baseUrl ?? DEFAULT_BASE_URL);
    const timeoutMs = normalizeTimeoutMs(input.timeoutMs);
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ??
        process.env.QWEN_WEB_BROWSER_EXECUTABLE_PATH ??
        process.env.PRODUCER_BROWSER_EXECUTABLE_PATH ??
        null,
    );
    if (!executablePath) {
      throw createWorkerError(
        500,
        "qwen_web_browser_not_found",
        "Unable to locate a Chromium-compatible browser for the Qwen Web session worker.",
      );
    }

    const profileSource = resolveProfileSource(input, authSeed);
    if (profileSource.mode === "clone") {
      clonedUserDataDir = await cloneBrowserProfile(
        profileSource.userDataDir,
        profileSource.profileDirectory,
      );
    } else {
      clonedUserDataDir = await createFreshBrowserProfile(profileSource.profileDirectory);
    }

    context = await chromium.launchPersistentContext(clonedUserDataDir, {
      executablePath,
      headless: parseBoolean(process.env.QWEN_WEB_SESSION_WORKER_HEADLESS, false),
      locale: DEFAULT_ACCEPT_LANGUAGE.split(",")[0] ?? "zh-CN",
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
    const referer = normalizeString(input.referer) ?? `${baseUrl}/c/guest`;
    await page.goto(referer, {
      waitUntil: "domcontentloaded",
      timeout: Math.min(timeoutMs, 60_000),
    });
    await page.waitForTimeout(1_500);

    const preferredModels = normalizePreferredModels(input.preferredModels);
    const probe = await probeQwenPage(page, baseUrl, preferredModels);

    let effectiveProbe = probe;
    if (!effectiveProbe.ok && authSeed?.password) {
      await loginAccount(page, baseUrl, authSeed, timeoutMs, createWorkerError);
      await page.waitForTimeout(2_000);
      effectiveProbe = await probeQwenPage(page, baseUrl, preferredModels);
    }

    if (!effectiveProbe.ok) {
      throw createWorkerError(
        effectiveProbe?.error?.status ?? 500,
        effectiveProbe?.error?.code ?? "qwen_web_session_probe_failed",
        effectiveProbe?.error?.message ?? "Qwen Web session probe failed.",
      );
    }

    const cookies = await context.cookies(baseUrl);
    const cookieHeader = cookies
      .map((cookie) => `${cookie.name}=${cookie.value}`)
      .filter(Boolean)
      .join("; ");

    const result = {
      ok: true,
      baseUrl,
      browserExecutablePath: executablePath,
      profileSource: {
        browser: profileSource.browser,
        userDataDir: profileSource.userDataDir,
        profileDirectory: profileSource.profileDirectory,
      },
      authToken: effectiveProbe.authToken,
      tokenSource: effectiveProbe.tokenSource,
      expiresAt: effectiveProbe.expiresAt ?? null,
      cookieHeader,
      selectedModel: effectiveProbe.selectedModel,
      selectedDisplayModel: effectiveProbe.selectedDisplayModel,
      availableModels: effectiveProbe.availableModels ?? [],
      authProbe: effectiveProbe.authProbe ?? null,
      modelProbe: effectiveProbe.modelProbe ?? null,
      createChatProbe: effectiveProbe.createChatProbe ?? null,
      localStorageKeys: effectiveProbe.localStorageKeys ?? [],
    };
    const credentialFile = await writeCredential(input, result, {
      parseBoolean,
      defaultFamilyDir: DEFAULT_CREDENTIAL_FAMILY_DIR,
    });
    if (credentialFile) {
      result.credentialFile = credentialFile;
    }
    printAndExit(result);
  } catch (error) {
    printAndExit({
      ok: false,
      error: serializeError(error),
    }, 1);
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

function normalizePreferredModels(value) {
  if (Array.isArray(value)) {
    const normalized = value.map(normalizeString).filter(Boolean);
    if (normalized.length > 0) {
      return normalized;
    }
  }
  const envValue = normalizeString(process.env.GATEWAY_QWEN_WEB_PREFERRED_MODELS);
  if (envValue) {
    const normalized = envValue
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean);
    if (normalized.length > 0) {
      return normalized;
    }
  }
  return DEFAULT_PREFERRED_MODELS;
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

function normalizeAuthSeed(value) {
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

function resolveProfileSource(input, authSeed) {
  if (input?.freshProfile === true && authSeed?.password) {
    const profileDirectory =
      normalizeString(input.profileDirectory) ??
      normalizeString(process.env.QWEN_WEB_BROWSER_PROFILE_DIR) ??
      DEFAULT_PROFILE_DIRECTORY;
    return {
      mode: "fresh",
      browser: "chromium",
      userDataDir: null,
      profileDirectory,
    };
  }
  const configuredUserDataDir =
    normalizeString(input.userDataDir) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_USER_DATA_DIR);
  const profileDirectory =
    normalizeString(input.profileDirectory) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_PROFILE_DIR) ??
    DEFAULT_PROFILE_DIRECTORY;
  if (configuredUserDataDir && existsSync(configuredUserDataDir)) {
    return {
      mode: "clone",
      browser: inferBrowserName(configuredUserDataDir),
      userDataDir: configuredUserDataDir,
      profileDirectory,
    };
  }
  if (process.platform === "win32" && existsSync(WINDOWS_EDGE_USER_DATA_DIR)) {
    return {
      mode: "clone",
      browser: "edge",
      userDataDir: WINDOWS_EDGE_USER_DATA_DIR,
      profileDirectory,
    };
  }
  if (process.platform === "win32" && existsSync(WINDOWS_CHROME_USER_DATA_DIR)) {
    return {
      mode: "clone",
      browser: "chrome",
      userDataDir: WINDOWS_CHROME_USER_DATA_DIR,
      profileDirectory,
    };
  }
  if (authSeed?.password) {
    return {
      mode: "fresh",
      browser: "chromium",
      userDataDir: null,
      profileDirectory,
    };
  }
  throw createWorkerError(
    500,
    "qwen_web_profile_not_found",
    "Unable to locate a Chromium user data directory for the Qwen Web session worker. Provide an existing signed-in browser profile or import a refreshed session from external account tooling.",
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
      "qwen_web_profile_missing",
      `Browser profile not found at ${sourceProfileDir}. Qwen Web refresh requires an existing signed-in browser profile; the gateway does not register or bootstrap accounts here.`,
    );
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "qwen-web-profile-"));
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

async function createFreshBrowserProfile(profileDirectory) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "qwen-web-profile-fresh-"));
  await mkdir(path.join(tempRoot, profileDirectory), { recursive: true });
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
    code: normalizeString(error?.code) ?? "qwen_web_session_worker_failed",
    message:
      normalizeString(error?.message) ??
      "Qwen Web session worker failed without a structured error message.",
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
