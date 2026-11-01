/** Trusted host overrides take precedence; bundled binaries never depend on the caller's cwd. */
import { statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

function isFile(value) {
  try { return path.isAbsolute(value) && statSync(value).isFile(); } catch { return false; }
}

export function runtimeExecutable(name, configured, missingCode, options = {}) {
  if (configured != null) {
    if (typeof configured !== "string" || !isFile(configured)) throw new Error(missingCode);
    return configured;
  }
  const root = options.root ?? fileURLToPath(new URL("./runtime/", import.meta.url));
  const bundled = path.join(root, name);
  if (isFile(bundled)) return bundled;
  // PATH is host configuration, not request input. Never resolve its relative/cwd entries.
  for (const entry of (options.searchPath ?? "").split(path.delimiter)) {
    const directory = entry.replace(/^"|"$/g, "");
    if (!path.isAbsolute(directory)) continue;
    const candidate = path.join(directory, name);
    if (isFile(candidate)) return candidate;
  }
  throw new Error(missingCode);
}
