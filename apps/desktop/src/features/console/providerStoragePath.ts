/** Filesystem paths only: an HTTP URL needs a defined transport/authentication contract. */
export function providerStoragePathError(value: string): "cloud" | "invalid" | null {
  const path = value.trim();
  if (/^https?:/i.test(path)) return "cloud";
  if (!path || /[\u0000-\u001f\u007f]/.test(path) || path.length > 4096) return "invalid";
  if (/^[a-z][a-z0-9+.-]*:/i.test(path) && !/^[a-z]:[\\/]/i.test(path)) return "invalid";
  if (!/^(?:\/|[a-z]:[\\/]|\\\\)/i.test(path) || path.split(/[\\/]/).includes("..")) return "invalid";
  return null;
}
