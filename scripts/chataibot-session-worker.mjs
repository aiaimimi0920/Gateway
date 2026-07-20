import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const DEFAULT_BASE_URL = "https://chataibot.pro";
const DEFAULT_PROFILE_DIRECTORY = "Default";
const DEFAULT_TIMEOUT_MS = 90_000;
const DEFAULT_PREFERRED_MODELS = ["qwen-lora", "google-nano-banana-2", "gpt-image-1.5"];
const DEFAULT_SERVICE_PROVIDER_DIR = "chataibot-platform";
const DEFAULT_SURFACE_DIR = "chataibot-images";
const DEFAULT_MATERIAL_KIND_DIR = "session-auth";
const DEFAULT_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
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
    const baseUrl = normalizeBaseUrl(input.baseUrl ?? DEFAULT_BASE_URL);
    const timeoutMs = normalizeTimeoutMs(input.timeoutMs);
    const preferredModels = normalizePreferredModels(input.preferredModels);
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ??
        process.env.CHATAIBOT_BROWSER_EXECUTABLE_PATH ??
        process.env.QWEN_WEB_BROWSER_EXECUTABLE_PATH ??
        null,
    );
    if (!executablePath) {
      throw createWorkerError(
        500,
        "chataibot_browser_not_found",
        "Unable to locate a Chromium-compatible browser for the ChatAIBot session worker.",
      );
    }

    const profileSource = resolveProfileSource(input);
    clonedUserDataDir = await cloneBrowserProfile(
      profileSource.userDataDir,
      profileSource.profileDirectory,
    );

    context = await chromium.launchPersistentContext(clonedUserDataDir, {
      executablePath,
      headless: parseBoolean(process.env.CHATAIBOT_SESSION_WORKER_HEADLESS, false),
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
    await page.goto(`${baseUrl}/app/chat?chat_id=-2`, {
      waitUntil: "domcontentloaded",
      timeout: Math.min(timeoutMs, 60_000),
    });
    await page.waitForTimeout(2_000);

    const cookies = await context.cookies(baseUrl);
    const cookieHeader = cookies
      .map((cookie) => `${cookie.name}=${cookie.value}`)
      .filter(Boolean)
      .join("; ");
    const cookieToken = cookies.find((cookie) => cookie.name === "token")?.value ?? null;

    const probe = await page.evaluate(async ({ preferredModels, cookieToken }) => {
      const trimString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const JWT_RE = /^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;
      const decodeJwt = (token) => {
        try {
          const [, payload] = token.split(".");
          if (!payload) {
            return null;
          }
          const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
          const jsonText = atob(normalized);
          return JSON.parse(jsonText);
        } catch {
          return null;
        }
      };
      const collectStorageModels = () => {
        const rawCandidates = [];
        for (const key of Object.keys(localStorage)) {
          const lowered = key.toLowerCase();
          if (!lowered.includes("model")) {
            continue;
          }
          const raw = localStorage.getItem(key);
          if (raw) {
            rawCandidates.push(raw);
          }
        }
        const models = new Set();
        const visit = (value) => {
          if (typeof value === "string") {
            if (
              ["qwen-lora", "google-nano-banana-2", "gpt-image-1.5"].includes(value.trim())
            ) {
              models.add(value.trim());
            }
            return;
          }
          if (Array.isArray(value)) {
            value.forEach(visit);
            return;
          }
          if (value && typeof value === "object") {
            Object.values(value).forEach(visit);
          }
        };
        for (const raw of rawCandidates) {
          try {
            visit(JSON.parse(raw));
          } catch {
            visit(raw);
          }
        }
        return Array.from(models);
      };
      const extractTokenFromStorage = () => {
        const preferredKeys = ["token", "active_token", "authToken"];
        for (const key of preferredKeys) {
          const value = trimString(localStorage.getItem(key));
          if (value && JWT_RE.test(value)) {
            return { source: `localStorage:${key}`, token: value };
          }
        }
        for (const key of Object.keys(localStorage)) {
          const value = trimString(localStorage.getItem(key));
          if (value && JWT_RE.test(value)) {
            return { source: `localStorage:${key}`, token: value };
          }
        }
        return null;
      };
      const collectJwtCandidates = (value, sink) => {
        if (!value) {
          return;
        }
        if (typeof value === "string") {
          const trimmed = value.trim();
          if (JWT_RE.test(trimmed)) {
            sink.add(trimmed);
          }
          return;
        }
        if (Array.isArray(value)) {
          value.forEach((entry) => collectJwtCandidates(entry, sink));
          return;
        }
        if (typeof value === "object") {
          Object.values(value).forEach((entry) => collectJwtCandidates(entry, sink));
        }
      };
      const openDatabase = (name, version) =>
        new Promise((resolve, reject) => {
          const request = indexedDB.open(name, version);
          request.onerror = () => reject(request.error);
          request.onsuccess = () => resolve(request.result);
        });
      const readStorePreview = (db, storeName) =>
        new Promise((resolve, reject) => {
          const rows = [];
          const transaction = db.transaction(storeName, "readonly");
          const store = transaction.objectStore(storeName);
          const request = store.openCursor();
          request.onerror = () => reject(request.error);
          request.onsuccess = () => {
            const cursor = request.result;
            if (!cursor || rows.length >= 40) {
              resolve(rows);
              return;
            }
            rows.push(cursor.value);
            cursor.continue();
          };
        });
      const extractTokenFromIndexedDb = async () => {
        if (typeof indexedDB.databases !== "function") {
          return null;
        }
        const databases = await indexedDB.databases();
        const jwtCandidates = new Set();
        for (const info of databases) {
          if (!info?.name) {
            continue;
          }
          try {
            const db = await openDatabase(info.name, info.version);
            try {
              for (const storeName of Array.from(db.objectStoreNames)) {
                const previewRows = await readStorePreview(db, storeName);
                previewRows.forEach((row) => collectJwtCandidates(row, jwtCandidates));
              }
            } finally {
              db.close();
            }
          } catch {
            // Ignore inaccessible IndexedDB stores and keep scanning.
          }
        }
        const token = Array.from(jwtCandidates)[0] ?? null;
        return token ? { source: "indexedDB", token } : null;
      };
      const requestJson = async (url, init) => {
        const response = await fetch(url, init);
        const text = await response.text();
        let json = null;
        try {
          json = text ? JSON.parse(text) : null;
        } catch {
          json = null;
        }
        return {
          status: response.status,
          ok: response.ok,
          text,
          json,
        };
      };

      const tokenFromStorage = extractTokenFromStorage();
      const tokenFromIndexedDb = await extractTokenFromIndexedDb();
      const resolvedToken =
        trimString(cookieToken) ??
        tokenFromStorage?.token ??
        tokenFromIndexedDb?.token ??
        null;
      if (!resolvedToken) {
        return {
          ok: false,
          error: {
            status: 401,
            code: "chataibot_session_missing_token_cookie",
            message:
              "No valid ChatAIBot token cookie was available from the cloned browser profile. The gateway session worker only reuses an existing signed-in browser session; it does not register or sign in accounts.",
          },
          localStorageKeys: Object.keys(localStorage),
        };
      }

      const answersProbe = await requestJson("/api/user/answers-count/v2", {
        method: "GET",
        credentials: "include",
        headers: {
          Accept: "application/json",
        },
      });
      const tokenPayload = decodeJwt(resolvedToken) ?? {};
      const storageModels = collectStorageModels();
      const selectedModel =
        preferredModels.find((candidate) => storageModels.includes(candidate)) ??
        preferredModels[0] ??
        storageModels[0] ??
        null;
      return {
        ok: true,
        authToken: resolvedToken,
        tokenSource:
          (trimString(cookieToken) ? "cookie:token" : null) ??
          tokenFromStorage?.source ??
          tokenFromIndexedDb?.source ??
          null,
        expiresAt:
          typeof tokenPayload.exp === "number"
            ? new Date(tokenPayload.exp * 1000).toISOString()
            : null,
        userId: trimString(tokenPayload.userId) ?? trimString(tokenPayload.sub),
        accountName:
          trimString(tokenPayload.email) ??
          trimString(tokenPayload.username) ??
          trimString(tokenPayload.userId) ??
          null,
        selectedModel,
        availableModels: Array.from(new Set([...storageModels, ...preferredModels])),
        quotaProbe: {
          status: answersProbe.status,
          ok: answersProbe.ok,
          leftAnswersCount:
            answersProbe?.json?.leftAnswersCount ??
            answersProbe?.json?.data?.leftAnswersCount ??
            null,
          message:
            answersProbe?.json?.message ??
            answersProbe?.json?.error ??
            answersProbe?.text ??
            null,
        },
        localStorageKeys: Object.keys(localStorage),
      };
    }, { preferredModels, cookieToken });

    if (!probe.ok) {
      throw createWorkerError(
        probe?.error?.status ?? 500,
        probe?.error?.code ?? "chataibot_session_probe_failed",
        probe?.error?.message ?? "ChatAIBot session probe failed.",
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
      authToken: probe.authToken,
      tokenSource: probe.tokenSource ?? null,
      expiresAt: probe.expiresAt ?? null,
      cookieHeader,
      userId: probe.userId ?? null,
      accountName: probe.accountName ?? null,
      selectedModel: probe.selectedModel ?? null,
      availableModels: Array.isArray(probe.availableModels) ? probe.availableModels : preferredModels,
      quotaProbe: probe.quotaProbe ?? null,
      localStorageKeys: Array.isArray(probe.localStorageKeys) ? probe.localStorageKeys : [],
    };

    const credentialFile = await maybeWriteCredentialFile(input, result);
    if (credentialFile) {
      result.credentialFile = credentialFile;
    }
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

async function maybeWriteCredentialFile(input, result) {
  const shouldWrite =
    input?.writeCredentialFile === true ||
    typeof input?.credentialFilePath === "string" ||
    typeof input?.credentialRootDir === "string" ||
    parseBoolean(process.env.CHATAIBOT_WRITE_CREDENTIAL_FILE, false);
  if (!shouldWrite || !result?.ok) {
    return null;
  }

  const targetPath = resolveCredentialFilePath(input, result);
  await mkdir(path.dirname(targetPath), { recursive: true });
  const payload = {
    authToken: result.authToken,
    cookieHeader: result.cookieHeader || `token=${result.authToken}`,
    supportedModels:
      Array.isArray(result.availableModels) && result.availableModels.length > 0
        ? result.availableModels
        : DEFAULT_PREFERRED_MODELS,
    selectedDisplayModel: result.selectedModel ?? null,
    accountName: result.accountName ?? null,
    userId: result.userId ?? null,
    credentialMaterialKey: result.userId ? `chataibot-user:${result.userId}` : undefined,
    baseUrl: result.baseUrl ?? DEFAULT_BASE_URL,
  };
  if (normalizeString(result.expiresAt)) {
    payload.expiresAt = result.expiresAt;
  }
  await writeFile(targetPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  return targetPath;
}

function resolveCredentialFilePath(input, result) {
  const explicitPath = normalizeString(input?.credentialFilePath);
  if (explicitPath) {
    return ensureJsonExtension(explicitPath);
  }
  const rootDir =
    normalizeString(input?.credentialRootDir) ??
    normalizeString(process.env.NEURO_PROVIDER_CREDENTIAL_ROOT_DIR) ??
    path.join(os.homedir(), ".neuro");
  const serviceProviderDir =
    normalizeString(input?.serviceProviderDir) ?? DEFAULT_SERVICE_PROVIDER_DIR;
  const surfaceDir =
    normalizeString(input?.providerSurfaceDir) ?? DEFAULT_SURFACE_DIR;
  const materialKindDir =
    normalizeString(input?.credentialMaterialKindDir) ?? DEFAULT_MATERIAL_KIND_DIR;
  const explicitFileName = normalizeString(input?.credentialFileName);
  const accountHint =
    normalizeString(result?.accountName) ??
    normalizeString(result?.userId) ??
    normalizeString(result?.selectedModel) ??
    `chataibot-${Date.now()}`;
  const fileName = ensureJsonExtension(
    explicitFileName ?? sanitizeFileNameComponent(accountHint) ?? `chataibot-${Date.now()}.json`,
  );
  return path.join(rootDir, serviceProviderDir, surfaceDir, materialKindDir, fileName);
}

function ensureJsonExtension(filePath) {
  return filePath.toLowerCase().endsWith(".json") ? filePath : `${filePath}.json`;
}

function sanitizeFileNameComponent(value) {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || null;
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

function normalizePreferredModels(value) {
  if (Array.isArray(value)) {
    const normalized = value.map(normalizeString).filter(Boolean);
    if (normalized.length > 0) {
      return normalized;
    }
  }
  return DEFAULT_PREFERRED_MODELS;
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
    normalizeString(process.env.CHATAIBOT_BROWSER_USER_DATA_DIR);
  const profileDirectory =
    normalizeString(input?.profileDirectory) ??
    normalizeString(process.env.CHATAIBOT_BROWSER_PROFILE_DIR) ??
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
    "chataibot_profile_not_found",
    "Unable to locate a Chromium user data directory for the ChatAIBot session worker. Provide an existing signed-in browser profile.",
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
      "chataibot_profile_missing",
      `Browser profile not found at ${sourceProfileDir}. ChatAIBot refresh requires an existing signed-in browser profile.`,
    );
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "chataibot-profile-"));
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
    code: normalizeString(error?.code) ?? "chataibot_session_worker_failed",
    message:
      normalizeString(error?.message) ??
      "ChatAIBot session worker failed without a structured error message.",
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
