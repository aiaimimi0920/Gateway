import { createHash } from "node:crypto";
import { copyFileSync, lstatSync, mkdirSync, readdirSync } from "node:fs";
import path from "node:path";
import { removeLaunchProfileClonePath } from "./gemini-canvas-browser-pool-resources.mjs";
import { getStorageRoot } from "./gemini-canvas-browser-pool-runtime-state.mjs";

export function createProfileCloneOwner(log) {
  function launchProfileCloneRoot() {
    return path.join(getStorageRoot(), "credential-runtime", "gemini-canvas-browser-launch-clones");
  }

  function profileClonePathForSource(sourcePath) {
    const hash = createHash("sha1").update(String(sourcePath || "")).digest("hex").slice(0, 12);
    return path.join(launchProfileCloneRoot(), `${hash}-${Date.now()}`);
  }

  function copyProfileTreeBestEffort(sourcePath, destinationPath) {
    const sourceStat = lstatSync(sourcePath);
    if (sourceStat.isDirectory()) {
      mkdirSync(destinationPath, { recursive: true });
      for (const entry of readdirSync(sourcePath, { withFileTypes: true })) {
        copyProfileTreeBestEffort(
          path.join(sourcePath, entry.name),
          path.join(destinationPath, entry.name),
        );
      }
      return;
    }
    if (!sourceStat.isFile()) {
      return;
    }
    mkdirSync(path.dirname(destinationPath), { recursive: true });
    try {
      copyFileSync(sourcePath, destinationPath);
    } catch (error) {
      log(
        "skipped locked profile entry during clone",
        JSON.stringify({
          sourcePath,
          destinationPath,
          message: error instanceof Error ? error.message : String(error),
        }),
      );
    }
  }

  function cloneProfileDirectory(sourcePath) {
    const destinationPath = profileClonePathForSource(sourcePath);
    mkdirSync(path.dirname(destinationPath), { recursive: true });
    log(
      "cloning live browser profile for isolated launch",
      JSON.stringify({ sourcePath, destinationPath }),
    );
    copyProfileTreeBestEffort(sourcePath, destinationPath);
    return destinationPath;
  }

  async function removeManagedLaunchProfileClone(clonePath) {
    const removed = await removeLaunchProfileClonePath(clonePath, launchProfileCloneRoot());
    if (!removed) {
      log("refused to remove browser profile outside clone root", clonePath);
    }
    return removed;
  }

  async function cleanupClonedLaunchProfile(entry) {
    if (!entry?.launchClonedProfile) {
      return;
    }

    const clonePath = entry.launchRuntimePath;
    entry.launchClonedProfile = false;
    const removed = await removeManagedLaunchProfileClone(clonePath);
    if (removed) {
      log("removed isolated browser profile clone", clonePath);
    }
  }

  return { cloneProfileDirectory, removeManagedLaunchProfileClone, cleanupClonedLaunchProfile };
}
