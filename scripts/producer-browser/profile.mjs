import { existsSync, lstatSync, realpathSync } from "node:fs";
import { copyFile, lstat, mkdtemp, mkdir, readdir, realpath, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { normalizeString } from "./request-fields.mjs";

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const MACOS_BROWSER_PATHS = [
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
];
const LINUX_BROWSER_PATHS = [
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
];
const WINDOWS_CHROME_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Google",
  "Chrome",
  "User Data",
);
const WINDOWS_EDGE_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Microsoft",
  "Edge",
  "User Data",
);
const PROFILE_FILES_TO_COPY = ["Preferences", "Secure Preferences"];
const PROFILE_DIRS_TO_COPY = [
  "Network",
  "Local Storage",
  "Session Storage",
  "IndexedDB",
  "WebStorage",
  "Service Worker",
];

export function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_BROWSER_PATHS
      : process.platform === "darwin"
        ? MACOS_BROWSER_PATHS
        : LINUX_BROWSER_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

export function resolveBrowserProfileSource(input) {
  const userDataDir =
    normalizeString(input.browserUserDataDir) ??
    normalizeString(process.env.PRODUCER_BROWSER_USER_DATA_DIR) ??
    (existsSync(WINDOWS_CHROME_USER_DATA_DIR)
      ? WINDOWS_CHROME_USER_DATA_DIR
      : existsSync(WINDOWS_EDGE_USER_DATA_DIR)
        ? WINDOWS_EDGE_USER_DATA_DIR
        : null);
  if (!userDataDir || !existsSync(userDataDir)) {
    return null;
  }

  const profileDirectory =
    normalizeString(input.browserProfileDirectory) ??
    normalizeString(process.env.PRODUCER_BROWSER_PROFILE_DIRECTORY) ??
    "Default";
  validateProfileDirectory(profileDirectory);
  // The operator-selected root may be a NAS link; descendants must not be links.
  const canonicalRoot = realpathSync(userDataDir);
  const profilePath = path.join(canonicalRoot, profileDirectory);
  let info;
  try {
    info = lstatSync(profilePath);
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
  validateProfileEntry(info);
  if (!info.isDirectory()) {
    return null;
  }

  return {
    userDataDir: canonicalRoot,
    profileDirectory,
  };
}

export async function cloneBrowserProfile(userDataDir, profileDirectory) {
  validateProfileDirectory(profileDirectory);
  const canonicalRoot = await realpath(userDataDir);
  const profileSource = path.join(canonicalRoot, profileDirectory);
  const info = await inspectProfileEntry(profileSource);
  if (!info?.isDirectory()) {
    throw profileError("producer_browser_profile_invalid", "Browser profile must be an existing directory.");
  }
  const cloneRoot = await mkdtemp(path.join(os.tmpdir(), "producer-browser-profile-"));
  const profileTarget = path.join(cloneRoot, profileDirectory);
  try {
    await mkdir(profileTarget, { recursive: true, mode: 0o700 });
    await copyProfileFile(path.join(canonicalRoot, "Local State"), path.join(cloneRoot, "Local State"));
    for (const fileName of PROFILE_FILES_TO_COPY) {
      await copyProfileFile(path.join(profileSource, fileName), path.join(profileTarget, fileName));
    }
    for (const dirName of PROFILE_DIRS_TO_COPY) {
      await copyProfileTree(path.join(profileSource, dirName), path.join(profileTarget, dirName));
    }
    return cloneRoot;
  } catch (error) {
    try {
      await rm(cloneRoot, { recursive: true, force: true });
    } catch (cleanupError) {
      throw new AggregateError([error, cleanupError], "Producer profile cloning and cleanup failed.");
    }
    throw error;
  }
}

async function copyProfileTree(source, target) {
  const info = await inspectProfileEntry(source);
  if (!info) {
    return;
  }

  if (info.isDirectory()) {
    await mkdir(target, { recursive: true, mode: 0o700 });
    const entries = await readdir(source);
    for (const entry of entries) {
      await copyProfileTree(path.join(source, entry), path.join(target, entry));
    }
    return;
  }

  await mkdir(path.dirname(target), { recursive: true, mode: 0o700 });
  await copyFile(source, target);
}

async function copyProfileFile(source, target) {
  const info = await inspectProfileEntry(source);
  if (!info) return;
  if (!info.isFile()) {
    throw profileError("producer_browser_profile_entry_invalid", "Browser profile state must be a regular file.");
  }
  await copyFile(source, target);
}

function profileError(code, message) {
  return Object.assign(new Error(message), { code, status: 400 });
}

function validateProfileDirectory(value) {
  if (
    typeof value !== "string" || !value || value === "." || value === ".." ||
    value !== value.trim() || /[. ]$/.test(value) || /[<>:"/\\|?*\x00-\x1f]/.test(value) ||
    /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value)
  ) {
    throw profileError("producer_browser_profile_invalid", "Browser profile must be a single valid directory name.");
  }
}

function validateProfileEntry(info) {
  if (info.isSymbolicLink()) {
    throw profileError("producer_browser_profile_link", "Browser profile links are not copied.");
  }
  if (!info.isDirectory() && !info.isFile()) {
    throw profileError("producer_browser_profile_entry_invalid", "Browser profile contains an unsupported file type.");
  }
}

async function inspectProfileEntry(source) {
  let info;
  try {
    info = await lstat(source);
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
  validateProfileEntry(info);
  return info;
}
