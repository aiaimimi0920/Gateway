import { rm } from "node:fs/promises";
import path from "node:path";

const DEFAULT_MAX_CONTEXTS = 4;

export function normalizeMaxContexts(value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_MAX_CONTEXTS;
  }
  return Math.max(1, Math.floor(parsed));
}

export function selectContextCapacityVictims(
  contextEntries,
  initializingCount,
  maxContexts,
) {
  const evictionCount = contextEntries.size + initializingCount - maxContexts + 1;
  if (evictionCount <= 0) {
    return [];
  }

  const candidates = [...contextEntries.entries()]
    .filter(([, entry]) => !entry.busy)
    .sort((left, right) => left[1].lastUsedAt - right[1].lastUsedAt);
  if (candidates.length < evictionCount) {
    throw Object.assign(new Error("Gemini Canvas browser context capacity is busy."), {
      code: "gemini_canvas_context_busy",
      status: 429,
    });
  }
  return candidates.slice(0, evictionCount).map(([key]) => key);
}

export async function removeLaunchProfileClonePath(clonePath, cloneRoot) {
  if (typeof clonePath !== "string" || clonePath.trim().length === 0) {
    return false;
  }

  const resolvedRoot = path.resolve(cloneRoot);
  const resolvedClonePath = path.resolve(clonePath);
  const relativePath = path.relative(resolvedRoot, resolvedClonePath);
  const isManagedClone =
    relativePath.length > 0 &&
    relativePath !== ".." &&
    !relativePath.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relativePath);
  if (!isManagedClone) {
    return false;
  }

  await rm(resolvedClonePath, {
    recursive: true,
    force: true,
    maxRetries: 3,
    retryDelay: 100,
  });
  return true;
}
