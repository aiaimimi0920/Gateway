import { lstatSync } from "node:fs";
import path from "node:path";

function invalidPath() {
  return Object.assign(new Error("AI Studio object key must remain inside its configured storage root."), {
    status: 400, code: "aistudio_runtime_state_invalid_path",
  });
}

export function resolveStorageObjectPath(root, objectKey) {
  if (typeof objectKey !== "string" || !objectKey.length || objectKey.length > 4096) throw invalidPath();
  const parts = objectKey.replace(/\\/g, "/").split("/");
  if (parts.length > 128 || parts.some((part) => !part || part === "." || part === ".."
    || /[<>:"|?*\x00-\x1f]/.test(part) || /[. ]$/.test(part)
    || /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part))) throw invalidPath();
  const base = path.resolve(root);
  const destination = path.resolve(base, ...parts);
  const relative = path.relative(base, destination);
  if (!relative || relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw invalidPath();
  }
  let current = base;
  for (const part of parts) {
    current = path.join(current, part);
    try {
      if (lstatSync(current).isSymbolicLink()) throw invalidPath();
    } catch (error) {
      if (error?.code === "ENOENT") break;
      throw error;
    }
  }
  return destination;
}
