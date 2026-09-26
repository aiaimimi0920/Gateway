import { lstat } from "node:fs/promises";
import { createWorkerError } from "./errors.mjs";

export function validateProfileDirectory(value) {
  // A browser profile is one child name, not a caller-selected output path.
  if (
    typeof value !== "string" ||
    !value || value === "." || value === ".." ||
    value !== value.trim() || /[. ]$/.test(value) ||
    /[<>:"/\\|?*\x00-\x1f]/.test(value) ||
    /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value)
  ) {
    throw createWorkerError(400, "chatgpt_web_profile_invalid", "Browser profile must be a single valid directory name.");
  }
}

export async function inspectProfileEntry(source) {
  let entry;
  try {
    entry = await lstat(source);
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
  if (entry.isSymbolicLink()) {
    throw createWorkerError(400, "chatgpt_web_profile_link", "Browser profile links are not copied.");
  }
  if (!entry.isDirectory() && !entry.isFile()) {
    throw createWorkerError(400, "chatgpt_web_profile_entry_invalid", "Browser profile contains an unsupported file type.");
  }
  return entry;
}
