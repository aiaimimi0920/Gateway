import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { normalizeString } from "./settings.mjs";

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
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
const DEFAULT_PROFILE_DIRECTORY = "Default";
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
// Keep the clone scope intentionally narrow. For the manual helper we only
// need a browser environment close to the user's real profile plus cookies in
// the Network store; copying the full web app storage tree makes startup slow
// and increases popup/service-worker churn during OAuth/captcha flows.
const PROFILE_DIRS_TO_COPY = ["Network"];

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

export function resolveProfileSource() {
  const configuredUserDataDir =
    normalizeString(process.env.UDIO_MANUAL_HELPER_BROWSER_USER_DATA_DIR) ??
    normalizeString(process.env.QWEN_WEB_BROWSER_USER_DATA_DIR) ??
    null;
  const profileDirectory =
    normalizeString(process.env.UDIO_MANUAL_HELPER_BROWSER_PROFILE_DIR) ??
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
  return null;
}

export function inferBrowserName(userDataDir) {
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
  const sourceProfileDir = path.join(userDataDir, profileDirectory);
  if (!existsSync(userDataDir) || !existsSync(sourceProfileDir)) {
    throw new Error(
      `Browser profile not found at ${sourceProfileDir}. Provide an existing signed-in Edge/Chrome profile.`,
    );
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "udio-manual-profile-"));
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

export async function copyIfExists(source, destination) {
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

export async function copyRecursive(source, destination) {
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

export function shouldIgnoreLockedProfileFile(error) {
  const code = normalizeString(error?.code)?.toUpperCase();
  return code === "EBUSY" || code === "EPERM";
}
