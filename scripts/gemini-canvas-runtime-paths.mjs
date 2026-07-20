import { existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

export function resolveGeminiCanvasStorageRoot() {
  return path.resolve(
    process.cwd(),
    process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR ?? ".runtime/ai-gateway-objects",
  );
}

function byNewestMtimeThenNameDesc(left, right) {
  const leftTime = safeMtime(left);
  const rightTime = safeMtime(right);
  if (rightTime !== leftTime) {
    return rightTime - leftTime;
  }
  return path.basename(right).localeCompare(path.basename(left));
}

function safeMtime(targetPath) {
  try {
    return statSync(targetPath).mtimeMs;
  } catch {
    return 0;
  }
}

function newestMatchingPath(baseDir, predicate) {
  if (!existsSync(baseDir)) {
    return null;
  }
  const candidates = readdirSync(baseDir, { withFileTypes: true })
    .filter((entry) => predicate(entry))
    .map((entry) => path.join(baseDir, entry.name))
    .sort(byNewestMtimeThenNameDesc);
  return candidates[0] ?? null;
}

function newestMatchingDirWithChild(baseDir, prefix, childRelativePath, suffix = "") {
  return newestMatchingPath(
    baseDir,
    (entry) =>
      entry.isDirectory()
      && entry.name.startsWith(prefix)
      && (!suffix || entry.name.endsWith(suffix))
      && existsSync(path.join(baseDir, entry.name, childRelativePath)),
  );
}

function newestMatchingFile(baseDir, prefix, suffix = "") {
  return newestMatchingPath(
    baseDir,
    (entry) =>
      entry.isFile()
      && entry.name.startsWith(prefix)
      && (!suffix || entry.name.endsWith(suffix)),
  );
}

export function resolveGeminiCanvasManualTestStorageStatePath(storageRoot = resolveGeminiCanvasStorageRoot()) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas");
  const preferred = path.join(baseDir, "manual-test", "storage-state.json");
  if (existsSync(preferred)) {
    return preferred;
  }
  const latest = newestMatchingDirWithChild(baseDir, "manual-test", "storage-state.json");
  return latest ? path.join(latest, "storage-state.json") : preferred;
}

export function resolveGeminiCanvasLiveProbeUserDataDir(storageRoot = resolveGeminiCanvasStorageRoot()) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas-profile");
  const preferred = path.join(baseDir, "live-probe", "user-data");
  if (existsSync(preferred)) {
    return preferred;
  }
  return (
    newestMatchingDirWithChild(baseDir, "live-probe", "user-data")
    ?? preferred
  );
}

export function resolveGeminiCanvasManualLiveVendorProfileDir(
  storageRoot = resolveGeminiCanvasStorageRoot(),
) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas");
  const preferred = path.join(baseDir, "manual-live-vendor-profile");
  if (existsSync(preferred)) {
    return preferred;
  }
  return (
    newestMatchingDirWithChild(baseDir, "manual-live-vendor", ".", "-profile")
    ?? preferred
  );
}

export function resolveGeminiCanvasManualLiveVendorStorageStatePath(
  storageRoot = resolveGeminiCanvasStorageRoot(),
) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas");
  const preferred = path.join(baseDir, "manual-live-vendor", "storage-state.json");
  if (existsSync(preferred)) {
    return preferred;
  }
  const latestDir = newestMatchingDirWithChild(baseDir, "manual-live-vendor", "storage-state.json");
  if (latestDir) {
    return path.join(latestDir, "storage-state.json");
  }
  const latestFile =
    newestMatchingFile(baseDir, "manual-live-vendor", "-storage-state.json")
    ?? newestMatchingFile(baseDir, "manual-live-vendor", "-storage-state-from-profile.json");
  return latestFile ?? preferred;
}

export function resolveGeminiCanvasHostExportStorageStatePath(
  storageRoot = resolveGeminiCanvasStorageRoot(),
) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas");
  const preferred = path.join(baseDir, "host-export", "storage-state.json");
  if (existsSync(preferred)) {
    return preferred;
  }
  const latest = newestMatchingDirWithChild(baseDir, "host-export", "storage-state.json");
  return latest ? path.join(latest, "storage-state.json") : preferred;
}

export function resolveGeminiCanvasManualLiveVendorProfileObjectKey(
  storageRoot = resolveGeminiCanvasStorageRoot(),
) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas");
  const preferred = path.join(baseDir, "manual-live-vendor-profile");
  if (existsSync(preferred)) {
    return "credential-runtime/gemini-canvas/manual-live-vendor-profile";
  }
  const latest = newestMatchingDirWithChild(baseDir, "manual-live-vendor", ".", "-profile");
  if (!latest) {
    return "credential-runtime/gemini-canvas/manual-live-vendor-profile";
  }
  return path.relative(storageRoot, latest).split(path.sep).join("/");
}

export function resolveGeminiCanvasHostExportDir(storageRoot = resolveGeminiCanvasStorageRoot()) {
  const baseDir = path.join(storageRoot, "credential-runtime", "gemini-canvas");
  const preferred = path.join(baseDir, "host-export");
  if (existsSync(preferred)) {
    return preferred;
  }
  return newestMatchingDirWithChild(baseDir, "host-export", "storage-state.json") ?? preferred;
}
