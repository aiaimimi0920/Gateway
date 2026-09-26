import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readdir, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

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

/** Owns profile cloning until a complete temporary directory reaches the caller. */
export async function cloneBrowserProfile(userDataDir, profileDirectory) {
  const sourceProfileDir = path.join(userDataDir, profileDirectory);
  if (!existsSync(userDataDir) || !existsSync(sourceProfileDir)) {
    throw Object.assign(new Error(
      `Browser profile not found at ${sourceProfileDir}. ChatAIBot refresh requires an existing signed-in browser profile.`,
    ), { status: 500, code: "chataibot_profile_missing" });
  }

  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "chataibot-profile-"));
  // Until the clone is returned, this owner must reclaim partial copies.
  try {
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
  } catch (error) {
    await rm(tempRoot, { recursive: true, force: true }).catch(() => {});
    throw error;
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
  const code = (typeof error?.code === "string" ? error.code.trim().toUpperCase() : undefined);
  return code === "EBUSY" || code === "EPERM";
}
