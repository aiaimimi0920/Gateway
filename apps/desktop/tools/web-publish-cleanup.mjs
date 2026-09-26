import { rm } from "node:fs/promises";

const CLEANUP_MAX_ATTEMPTS = 5;
const CLEANUP_RETRY_MS = 100;
const CLEANUP_RETRY_CODES = new Set([
  "EACCES",
  "EBUSY",
  "EMFILE",
  "ENFILE",
  "ENOTEMPTY",
  "EPERM",
]);

export function sleep(durationMs) {
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}

export async function removePathWithRetry(targetPath, options) {
  for (let attempt = 1; attempt <= CLEANUP_MAX_ATTEMPTS; attempt += 1) {
    try {
      await rm(targetPath, options);
      return;
    } catch (error) {
      if (
        !CLEANUP_RETRY_CODES.has(error?.code) ||
        attempt === CLEANUP_MAX_ATTEMPTS
      ) {
        throw error;
      }
      await sleep(CLEANUP_RETRY_MS * attempt);
    }
  }
}
