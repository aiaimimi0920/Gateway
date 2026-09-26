import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, realpath, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { normalizeString, parseBoolean } from "./configuration.mjs";
import { createWorkerError } from "./errors.mjs";
import { inspectProfileEntry, validateProfileDirectory } from "./profile-paths.mjs";

const DEFAULT_PROFILE_DIRECTORY = "Default";

const WINDOWS_EDGE_EXECUTABLES = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];

const WINDOWS_CHROME_EXECUTABLES = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
];

const LINUX_CHROMIUM_EXECUTABLES = [
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
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

export function resolveExecutablePath(configured) {
  const candidates = [];
  if (normalizeString(configured)) {
    candidates.push(configured);
  }
  if (process.platform === "win32") {
    candidates.push(...WINDOWS_EDGE_EXECUTABLES, ...WINDOWS_CHROME_EXECUTABLES);
  } else {
    candidates.push(...LINUX_CHROMIUM_EXECUTABLES);
  }
  return candidates.find((candidate) => candidate && existsSync(candidate)) ?? null;
}

export function resolveProfileSource(input, authSeed) {
  const configuredUserDataDir =
    normalizeString(input?.userDataDir) ??
    normalizeString(process.env.CHATGPT_WEB_BROWSER_USER_DATA_DIR);
  const autoCloneLocalProfile = parseBoolean(
    input?.autoCloneLocalProfile ??
      process.env.CHATGPT_WEB_BROWSER_AUTO_CLONE_LOCAL_PROFILE,
    false,
  );
  const profileDirectory =
    normalizeString(input?.profileDirectory) ??
    normalizeString(process.env.CHATGPT_WEB_BROWSER_PROFILE_DIR) ??
    DEFAULT_PROFILE_DIRECTORY;

  if (configuredUserDataDir && existsSync(configuredUserDataDir)) {
    return {
      mode: "clone",
      browser: inferBrowserName(configuredUserDataDir),
      userDataDir: configuredUserDataDir,
      profileDirectory,
    };
  }
  if (autoCloneLocalProfile && process.platform === "win32" && existsSync(WINDOWS_EDGE_USER_DATA_DIR)) {
    return {
      mode: "clone",
      browser: "edge",
      userDataDir: WINDOWS_EDGE_USER_DATA_DIR,
      profileDirectory,
    };
  }
  if (autoCloneLocalProfile && process.platform === "win32" && existsSync(WINDOWS_CHROME_USER_DATA_DIR)) {
    return {
      mode: "clone",
      browser: "chrome",
      userDataDir: WINDOWS_CHROME_USER_DATA_DIR,
      profileDirectory,
    };
  }
  return {
    mode: "fresh",
    browser: authSeed?.password ? "chromium-login" : "chromium",
    userDataDir: null,
    profileDirectory,
  };
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

export async function cloneBrowserProfile(userDataDir, profileDirectory) {
  validateProfileDirectory(profileDirectory);
  const sourceProfileDir = path.join(userDataDir, profileDirectory);
  if (!existsSync(userDataDir) || !existsSync(sourceProfileDir)) {
    throw createWorkerError(
      500,
      "chatgpt_web_profile_missing",
      `Browser profile not found at ${sourceProfileDir}. ChatGPT Web refresh requires an existing profile or a browser login seed.`,
    );
  }

  // The operator-selected root may itself be a NAS junction. Descendants may not.
  const sourceRoot = await realpath(userDataDir);
  const selectedProfile = path.join(sourceRoot, profileDirectory);
  await inspectProfileEntry(selectedProfile);
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "chatgpt-web-profile-"));
  let complete = false;
  try {
    for (const fileName of ROOT_FILES_TO_COPY) {
      await copyIfExists(path.join(sourceRoot, fileName), path.join(tempRoot, fileName));
    }

    const clonedProfileDir = path.join(tempRoot, profileDirectory);
    await mkdir(clonedProfileDir, { recursive: true });
    for (const fileName of PROFILE_FILES_TO_COPY) {
      await copyIfExists(path.join(selectedProfile, fileName), path.join(clonedProfileDir, fileName));
    }
    for (const directoryName of PROFILE_DIRS_TO_COPY) {
      await copyRecursive(
        path.join(selectedProfile, directoryName),
        path.join(clonedProfileDir, directoryName),
      );
    }
    complete = true;
    return tempRoot;
  } finally {
    if (!complete) await rm(tempRoot, { recursive: true, force: true });
  }
}

export async function createFreshBrowserProfile(profileDirectory) {
  validateProfileDirectory(profileDirectory);
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "chatgpt-web-profile-fresh-"));
  let complete = false;
  try {
    await mkdir(path.join(tempRoot, profileDirectory), { recursive: true });
    complete = true;
    return tempRoot;
  } finally {
    if (!complete) await rm(tempRoot, { recursive: true, force: true });
  }
}

async function copyIfExists(source, destination) {
  if (!(await inspectProfileEntry(source))) {
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
  const sourceStat = await inspectProfileEntry(source);
  if (!sourceStat) {
    return;
  }
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
