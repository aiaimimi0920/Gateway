import { existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

export function createBrowserlessMaterialOwner({ normalizeString, readJson }) {
  function fileExists(filePath) {
    try {
      return existsSync(filePath);
    } catch {
      return false;
    }
  }

  function pathStatMtime(filePath) {
    try {
      return statSync(filePath).mtimeMs;
    } catch {
      return 0;
    }
  }

  function findLatestBrowserState(repoRoot) {
    const runtimeRoot = path.join(repoRoot, ".runtime", "gemini-canvas-program-runtime");
    if (!fileExists(runtimeRoot)) {
      return null;
    }
    const candidates = [];
    const queue = [runtimeRoot];
    while (queue.length) {
      const currentDir = queue.shift();
      let entries = [];
      try {
        entries = requireDirEntries(currentDir);
      } catch {
        continue;
      }
      for (const entry of entries) {
        const fullPath = path.join(currentDir, entry.name);
        if (entry.isDirectory()) {
          queue.push(fullPath);
          continue;
        }
        if (entry.isFile() && entry.name === "browser-state.json") {
          candidates.push(fullPath);
        }
      }
    }
    if (!candidates.length) {
      return null;
    }
    candidates.sort((left, right) => pathStatMtime(right) - pathStatMtime(left));
    return candidates[0];
  }

  function requireDirEntries(dirPath) {
    return readdirSync(dirPath, { withFileTypes: true });
  }

  function uniqueStrings(values) {
    return Array.from(
      new Set(
        (values || [])
          .map((value) => String(value ?? "").trim())
          .filter(Boolean),
      ),
    );
  }

  function resolveProgramHandlePath(browserState, repoRoot) {
    const explicit = normalizeString(browserState?.sourceProgramHandlePath);
    if (explicit && fileExists(explicit)) {
      return explicit;
    }
    const browserStateDir = path.dirname(browserState.__path);
    const sibling = path.join(browserStateDir, "program-handle.json");
    if (fileExists(sibling)) {
      return sibling;
    }
    const probeRoot = path.join(repoRoot, ".runtime", "gemini-canvas-program-handle-probe");
    if (!fileExists(probeRoot)) {
      return null;
    }
    let newest = null;
    const entries = requireDirEntries(probeRoot)
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(probeRoot, entry.name, "program-handle.json"))
      .filter((candidate) => fileExists(candidate));
    for (const candidate of entries) {
      if (!newest || pathStatMtime(candidate) > pathStatMtime(newest)) {
        newest = candidate;
      }
    }
    return newest;
  }

  async function resolveProgramHandle(browserState, repoRoot) {
    const handlePath = resolveProgramHandlePath(browserState, repoRoot);
    if (!handlePath) {
      return { path: null, json: null };
    }
    return {
      path: handlePath,
      json: await readJson(handlePath),
    };
  }

  function resolveProfileDir(browserState, programHandle, repoRoot, args) {
    const explicit = normalizeString(args["profile-dir"]);
    if (explicit && fileExists(explicit)) {
      return explicit;
    }
    const handleProfileDir = normalizeString(programHandle?.json?.runtimeProfileDir);
    if (handleProfileDir && fileExists(handleProfileDir)) {
      return handleProfileDir;
    }
    const browserProfileDir = normalizeString(browserState?.runtimeProfileDir);
    if (browserProfileDir && fileExists(browserProfileDir)) {
      return browserProfileDir;
    }
    const runtimeStateObjectKey = normalizeString(browserState?.runtimeStateObjectKey);
    if (runtimeStateObjectKey) {
      const candidate = path.join(
        repoRoot,
        ".runtime",
        "ai-gateway-objects",
        ...runtimeStateObjectKey.split("/"),
      );
      if (fileExists(candidate)) {
        return candidate;
      }
    }
    return null;
  }

  function walkFiles(rootDir, options = {}) {
    const {
      maxDepth = 6,
      fileName = null,
      includePath = null,
    } = options;
    const results = [];
    const queue = [{ dir: rootDir, depth: 0 }];
    while (queue.length) {
      const current = queue.shift();
      let entries = [];
      try {
        entries = requireDirEntries(current.dir);
      } catch {
        continue;
      }
      for (const entry of entries) {
        const fullPath = path.join(current.dir, entry.name);
        if (entry.isDirectory()) {
          if (current.depth < maxDepth) {
            queue.push({ dir: fullPath, depth: current.depth + 1 });
          }
          continue;
        }
        if (!entry.isFile()) {
          continue;
        }
        if (fileName && entry.name !== fileName) {
          continue;
        }
        if (includePath && !fullPath.toLowerCase().includes(includePath.toLowerCase())) {
          continue;
        }
        results.push(fullPath);
      }
    }
    return results;
  }

  function resolveStorageStatePath(repoRoot, profileDir, args) {
    const explicit = normalizeString(args["storage-state"]);
    const candidatePool = [];
    if (explicit) {
      candidatePool.push(path.resolve(explicit));
    }
    if (profileDir) {
      candidatePool.push(path.join(profileDir, "storage-state.json"));
      candidatePool.push(path.join(path.dirname(profileDir), "storage-state.json"));
    }
    const runtimeObjectRoot = path.join(repoRoot, ".runtime", "ai-gateway-objects");
    if (fileExists(runtimeObjectRoot)) {
      candidatePool.push(
        ...walkFiles(runtimeObjectRoot, {
          maxDepth: 7,
          fileName: "storage-state.json",
          includePath: "gemini-canvas",
        }),
      );
    }
    const runtimeRoot = path.join(repoRoot, ".runtime");
    if (fileExists(runtimeRoot)) {
      candidatePool.push(
        ...walkFiles(runtimeRoot, {
          maxDepth: 6,
          fileName: "storage-state.json",
          includePath: "gemini-canvas",
        }),
      );
    }

    const uniqueCandidates = uniqueStrings(candidatePool).filter((candidate) => fileExists(candidate));
    if (!uniqueCandidates.length) {
      return null;
    }
    uniqueCandidates.sort((left, right) => pathStatMtime(right) - pathStatMtime(left));
    return uniqueCandidates[0];
  }

  return {
    fileExists,
    pathStatMtime,
    findLatestBrowserState,
    uniqueStrings,
    resolveProgramHandlePath,
    resolveProgramHandle,
    resolveProfileDir,
    walkFiles,
    resolveStorageStatePath,
  };
}
