import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

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
    const probe = await page.evaluate(
      async ({ baseUrl, preferredModels }) => {
        const trimString = (value) =>
          typeof value === "string" && value.trim() ? value.trim() : null;
        const decodeJwtExp = (token) => {
          try {
            const [, payload] = token.split(".");
            if (!payload) {
              return null;
            }
            const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
            const jsonText = atob(normalized);
            const decoded = JSON.parse(jsonText);
            return typeof decoded.exp === "number"
              ? new Date(decoded.exp * 1000).toISOString()
              : null;
          } catch {
            return null;
          }
        };
        const headersFor = (token) => ({
          Authorization: `Bearer ${token}`,
          Accept: "application/json, text/plain, */*",
          "Content-Type": "application/json",
        });
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
        const availableTokens = [
          { source: "active_token", token: trimString(localStorage.getItem("active_token")) },
          { source: "token", token: trimString(localStorage.getItem("token")) },
        ].filter((entry) => entry.token);

        let resolvedToken = null;
        let tokenSource = null;
        let authProbe = null;
        for (const entry of availableTokens) {
          const probe = await requestJson(`${baseUrl}/api/v1/auths/`, {
            method: "GET",
            headers: headersFor(entry.token),
          });
          if (probe.ok) {
            resolvedToken = entry.token;
            tokenSource = entry.source;
            authProbe = probe;
            break;
          }
          if (authProbe === null) {
            authProbe = probe;
          }
        }
        if (!resolvedToken) {
          return {
            ok: false,
            error: {
              code: "qwen_web_session_missing_active_token",
              message:
                "No valid Qwen Web browser token was available from localStorage. The gateway refresh worker only reuses an existing signed-in Qwen Web session; it does not register or sign in accounts.",
              status: authProbe?.status ?? 401,
            },
            authProbe,
            localStorageKeys: Object.keys(localStorage),
          };
        }

        const modelProbe = await requestJson(`${baseUrl}/api/models`, {
          method: "GET",
          headers: headersFor(resolvedToken),
        });
        const rawModels = Array.isArray(modelProbe?.json?.data)
          ? modelProbe.json.data
          : Array.isArray(modelProbe?.json)
            ? modelProbe.json
            : [];
        const normalizedModels = rawModels
          .map((entry) => {
            if (typeof entry === "string") {
              return { id: entry, label: entry };
            }
            if (!entry || typeof entry !== "object") {
              return null;
            }
            const id =
              trimString(entry.code) ??
              trimString(entry.id) ??
              trimString(entry.model) ??
              trimString(entry.name);
            if (!id) {
              return null;
            }
            return {
              id,
              label:
                trimString(entry.name) ??
                trimString(entry.display_name) ??
                trimString(entry.label) ??
                id,
            };
          })
          .filter(Boolean);
        const selectedModel =
          preferredModels.find((candidate) =>
            normalizedModels.some((entry) => entry.id === candidate),
          ) ?? normalizedModels[0]?.id ?? null;
        const selectedDisplayModel =
          normalizedModels.find((entry) => entry.id === selectedModel)?.label ?? selectedModel;

        let createChatProbe = null;
        if (selectedModel) {
          createChatProbe = await requestJson(`${baseUrl}/api/v2/chats/new`, {
            method: "POST",
            headers: headersFor(resolvedToken),
            body: JSON.stringify({
              title: `api_${Math.floor(Date.now() / 1000)}`,
              models: [selectedModel],
              chat_mode: "normal",
              chat_type: "t2t",
              timestamp: Math.floor(Date.now() / 1000),
            }),
          });
        }

        const expiresAt =
          trimString(authProbe?.json?.expires_at) ??
          trimString(authProbe?.json?.data?.expires_at) ??
          decodeJwtExp(resolvedToken);

        return {
          ok: true,
          authToken: resolvedToken,
          tokenSource,
          expiresAt,
          selectedModel,
          selectedDisplayModel,
          availableModels: normalizedModels,
          authProbe: {
            status: authProbe?.status ?? null,
            ok: authProbe?.ok ?? false,
            userId:
              trimString(authProbe?.json?.id) ??
              trimString(authProbe?.json?.data?.id) ??
              null,
            email:
              trimString(authProbe?.json?.email) ??
              trimString(authProbe?.json?.data?.email) ??
              null,
          },
          modelProbe: {
            status: modelProbe?.status ?? null,
            ok: modelProbe?.ok ?? false,
            count: normalizedModels.length,
          },
          createChatProbe: {
            status: createChatProbe?.status ?? null,
            ok: createChatProbe?.ok ?? false,
            chatId:
              trimString(createChatProbe?.json?.id) ??
              trimString(createChatProbe?.json?.data?.id) ??
              null,
          },
          localStorageKeys: Object.keys(localStorage),
        };
      },
      { baseUrl, preferredModels },
    );

    let effectiveProbe = probe;
    if (!effectiveProbe.ok && authSeed?.password) {
      await loginExistingAccount(page, baseUrl, authSeed, timeoutMs);
      await page.waitForTimeout(2_000);
      effectiveProbe = await page.evaluate(
        async ({ baseUrl, preferredModels }) => {
          const trimString = (value) =>
            typeof value === "string" && value.trim() ? value.trim() : null;
          const decodeJwtExp = (token) => {
            try {
              const [, payload] = token.split(".");
              if (!payload) {
                return null;
              }
              const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
              const jsonText = atob(normalized);
              const decoded = JSON.parse(jsonText);
              return typeof decoded.exp === "number"
                ? new Date(decoded.exp * 1000).toISOString()
                : null;
            } catch {
              return null;
            }
          };
          const headersFor = (token) => ({
            Authorization: `Bearer ${token}`,
            Accept: "application/json, text/plain, */*",
            "Content-Type": "application/json",
          });
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
          const availableTokens = [
            { source: "active_token", token: trimString(localStorage.getItem("active_token")) },
            { source: "token", token: trimString(localStorage.getItem("token")) },
          ].filter((entry) => entry.token);

          let resolvedToken = null;
          let tokenSource = null;
          let authProbe = null;
          for (const entry of availableTokens) {
            const probe = await requestJson(`${baseUrl}/api/v1/auths/`, {
              method: "GET",
              headers: headersFor(entry.token),
            });
            if (probe.ok) {
              resolvedToken = entry.token;
              tokenSource = entry.source;
              authProbe = probe;
              break;
            }
            if (authProbe === null) {
              authProbe = probe;
            }
          }
          if (!resolvedToken) {
            return {
              ok: false,
              error: {
                code: "qwen_web_session_missing_active_token",
                message:
                  "No valid Qwen Web browser token was available from localStorage. The gateway refresh worker only reuses an existing signed-in Qwen Web session; it does not register or sign in accounts.",
                status: authProbe?.status ?? 401,
              },
              authProbe,
              localStorageKeys: Object.keys(localStorage),
            };
          }

          const modelProbe = await requestJson(`${baseUrl}/api/models`, {
            method: "GET",
            headers: headersFor(resolvedToken),
          });
          const rawModels = Array.isArray(modelProbe?.json?.data)
            ? modelProbe.json.data
            : Array.isArray(modelProbe?.json)
              ? modelProbe.json
              : [];
          const normalizedModels = rawModels
            .map((entry) => {
              if (typeof entry === "string") {
                return { id: entry, label: entry };
              }
              if (!entry || typeof entry !== "object") {
                return null;
              }
              const id =
                trimString(entry.code) ??
                trimString(entry.id) ??
                trimString(entry.model) ??
                trimString(entry.name);
              if (!id) {
                return null;
              }
              return {
                id,
                label:
                  trimString(entry.name) ??
                  trimString(entry.display_name) ??
                  trimString(entry.label) ??
                  id,
              };
            })
            .filter(Boolean);
          const selectedModel =
            preferredModels.find((candidate) =>
              normalizedModels.some((entry) => entry.id === candidate),
            ) ?? normalizedModels[0]?.id ?? null;
          const selectedDisplayModel =
            normalizedModels.find((entry) => entry.id === selectedModel)?.label ?? selectedModel;

          let createChatProbe = null;
          if (selectedModel) {
            createChatProbe = await requestJson(`${baseUrl}/api/v2/chats/new`, {
              method: "POST",
              headers: headersFor(resolvedToken),
              body: JSON.stringify({
                title: `api_${Math.floor(Date.now() / 1000)}`,
                models: [selectedModel],
                chat_mode: "normal",
                chat_type: "t2t",
                timestamp: Math.floor(Date.now() / 1000),
              }),
            });
          }

          const expiresAt =
            trimString(authProbe?.json?.expires_at) ??
            trimString(authProbe?.json?.data?.expires_at) ??
            decodeJwtExp(resolvedToken);

          return {
            ok: true,
            authToken: resolvedToken,
            tokenSource,
            expiresAt,
            selectedModel,
            selectedDisplayModel,
            availableModels: normalizedModels,
            authProbe: {
              status: authProbe?.status ?? null,
              ok: authProbe?.ok ?? false,
              userId:
                trimString(authProbe?.json?.id) ??
                trimString(authProbe?.json?.data?.id) ??
                null,
              email:
                trimString(authProbe?.json?.email) ??
                trimString(authProbe?.json?.data?.email) ??
                null,
            },
            modelProbe: {
              status: modelProbe?.status ?? null,
              ok: modelProbe?.ok ?? false,
              count: normalizedModels.length,
            },
            createChatProbe: {
              status: createChatProbe?.status ?? null,
              ok: createChatProbe?.ok ?? false,
              chatId:
                trimString(createChatProbe?.json?.id) ??
                trimString(createChatProbe?.json?.data?.id) ??
                null,
            },
            localStorageKeys: Object.keys(localStorage),
          };
        },
        { baseUrl, preferredModels },
      );
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
    const credentialFile = await maybeWriteCredentialFile(input, result);
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

async function maybeWriteCredentialFile(input, result) {
  const shouldWrite =
    input?.writeCredentialFile === true ||
    typeof input?.credentialFilePath === "string" ||
    typeof input?.credentialRootDir === "string" ||
    parseBoolean(process.env.QWEN_WEB_WRITE_CREDENTIAL_FILE, false);
  if (!shouldWrite || !result?.ok) {
    return null;
  }

  const targetPath = resolveCredentialFilePath(input, result);
  await mkdir(path.dirname(targetPath), { recursive: true });
  const payload = buildCredentialPayload(result);
  await writeFile(targetPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  return targetPath;
}

function buildCredentialPayload(result) {
  const payload = {
    apiKey: result.authToken,
  };
  if (normalizeString(result.expiresAt)) {
    payload.expiresAt = result.expiresAt;
  }
  if (normalizeString(result.cookieHeader)) {
    payload.headers = {
      Cookie: result.cookieHeader,
    };
  }
  if (normalizeString(result.selectedModel)) {
    payload.supportedModels = [result.selectedModel];
  }
  if (normalizeString(result.selectedDisplayModel)) {
    payload.selectedDisplayModel = result.selectedDisplayModel;
  }
  const accountName =
    normalizeString(result?.authProbe?.email) ?? normalizeString(result?.authProbe?.userId);
  if (accountName) {
    payload.accountName = accountName;
  }
  const materialKey = normalizeString(result?.authProbe?.userId)
    ? `qwen-web-user:${result.authProbe.userId}`
    : null;
  if (materialKey) {
    payload.credentialMaterialKey = materialKey;
  }
  payload.rawSource = {
    tokenSource: result.tokenSource ?? null,
    authProbe: result.authProbe ?? null,
    modelProbe: result.modelProbe ?? null,
    createChatProbe: result.createChatProbe ?? null,
    profileSource: result.profileSource ?? null,
    localStorageKeys: Array.isArray(result.localStorageKeys) ? result.localStorageKeys : [],
  };
  return payload;
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
  const familyDir =
    normalizeString(input?.credentialFamilyDir) ?? DEFAULT_CREDENTIAL_FAMILY_DIR;
  const explicitFileName = normalizeString(input?.credentialFileName);
  const accountHint =
    normalizeString(result?.authProbe?.email) ??
    normalizeString(result?.authProbe?.userId) ??
    normalizeString(result?.selectedModel) ??
    `qwen-web-${Date.now()}`;
  const inferredFileName =
    explicitFileName ??
    sanitizeFileNameComponent(accountHint) ??
    `qwen-web-${Date.now()}.json`;
  const fileName = ensureJsonExtension(
    inferredFileName,
  );
  return path.join(rootDir, familyDir, fileName);
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

async function loginExistingAccount(page, baseUrl, authSeed, timeoutMs) {
  if (!authSeed?.email || !authSeed?.password) {
    throw createWorkerError(
      401,
      "qwen_web_browser_login_seed_missing",
      "Browser-backed Qwen Web login requires an existing account email and plain password.",
    );
  }

  await page.goto(`${baseUrl}/auth`, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  });
  await page.waitForTimeout(2_000);

  let emailInput = await page.$('input[placeholder*=\"Email\"]');
  let passwordInput = await page.$('input[type=\"password\"]');
  if (!emailInput || !passwordInput) {
    const inputs = await page.$$('input');
    emailInput = emailInput ?? inputs[0] ?? null;
    if (!passwordInput && inputs.length >= 2) {
      passwordInput = inputs[1];
    }
  }

  if (!emailInput || !passwordInput) {
    throw createWorkerError(
      500,
      "qwen_web_browser_login_form_missing",
      "Could not locate the Qwen Web login form while refreshing an existing account session.",
    );
  }

  await emailInput.click();
  await emailInput.fill(authSeed.email);
  await passwordInput.click();
  await passwordInput.fill(authSeed.password);

  const submitButton =
    (await page.$('button:has-text(\"Log in\")')) ??
    (await page.$('button:has-text(\"登录\")')) ??
    (await page.$('button[type=\"submit\"]'));
  if (!submitButton) {
    throw createWorkerError(
      500,
      "qwen_web_browser_login_submit_missing",
      "Could not locate the Qwen Web login submit button while refreshing an existing account session.",
    );
  }

  await submitButton.click();
  await page.waitForTimeout(8_000);
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
