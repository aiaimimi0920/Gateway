// Incremental mtime+size cached effective-line inner-loop checker.
// Preserves scripts/effective-code-lines.mjs unchanged for authoritative baseline ratification.
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import {
  countEffectiveLines,
  countPhysicalLines,
  decodeUtf8,
  languageForExtension,
} from "./effective-code-lines-lexer.mjs";
import { loadRuntimeArtifacts, matchesRuntimeArtifact } from "./effective-code-lines-runtime-artifacts.mjs";
import { normalizeRepoPath } from "./effective-code-lines.mjs";

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptsDir, "..");
const policyPath = path.join(scriptsDir, "effective-code-lines-policy.json");
const baselinePath = path.join(scriptsDir, "effective-code-lines-baseline.json");
const exceptionsPath = path.join(scriptsDir, "effective-code-lines-exceptions.json");
const cachePath = path.join(repoRoot, "artifacts", ".effective-lines-fast-cache.json");

function sha256Text(text) {
  return crypto.createHash("sha256").update(Buffer.from(text, "utf8")).digest("hex");
}

function loadJsonOrNull(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function isExcluded(relativePath, entryName, isDirectory, exclusions) {
  const normalized = relativePath.replaceAll("\\", "/");
  const lower = entryName.toLowerCase();
  if (isDirectory && exclusions.names.has(lower)) return true;
  if (isDirectory && exclusions.dirPrefixes.some((p) => lower.startsWith(p))) return true;
  if (exclusions.paths.some((p) => normalized === p || normalized.startsWith(`${p}/`))) return true;
  if (isDirectory) return false;
  const parent = path.posix.dirname(normalized);
  return exclusions.filePrefixes.some((e) => parent === e.directory && entryName.startsWith(e.prefix));
}

function runFastRatchet() {
  const startedAt = Date.now();
  const policy = JSON.parse(fs.readFileSync(policyPath, "utf8"));
  const baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
  const exceptions = JSON.parse(fs.readFileSync(exceptionsPath, "utf8"));
  const prevCache = loadJsonOrNull(cachePath)?.entries ?? {};
  const nextCache = {};

  const exclusions = {
    names: new Set(policy.excludedDirectories.map((e) => e.name.toLowerCase())),
    dirPrefixes: policy.excludedDirectoryPrefixes.map((e) => e.prefix.toLowerCase()),
    paths: policy.excludedPaths.map((e) => normalizeRepoPath(e.path)),
    filePrefixes: policy.excludedFilePrefixes.map((e) => ({
      directory: normalizeRepoPath(e.directory),
      prefix: e.prefix,
    })),
  };
  const extensions = new Set(policy.sourceExtensions);
  const resolvedRoot = fs.realpathSync(repoRoot);
  const runtimeArtifacts = loadRuntimeArtifacts(resolvedRoot, policy.immutableRuntimeArtifacts);
  const baselineByPath = new Map(baseline.files.map((f) => [f.path, f]));
  const exceptionByPath = new Map((exceptions.exceptions ?? []).map((e) => [e.path, e]));

  let scannedCount = 0;
  let cachedHits = 0;
  const violations = [];

  function visit(dir, relDir = "") {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const relPath = relDir ? `${relDir}/${entry.name}` : entry.name;
      if (isExcluded(relPath, entry.name, entry.isDirectory(), exclusions)) continue;
      if (entry.isSymbolicLink()) continue;
      const absPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        visit(absPath, relPath);
      } else if (entry.isFile() && extensions.has(path.extname(entry.name).toLowerCase())) {
        const normPath = normalizeRepoPath(relPath);
        const stat = fs.statSync(absPath);
        const prev = prevCache[normPath];
        let row;
        if (prev && prev.mtimeMs === stat.mtimeMs && prev.size === stat.size) {
          row = prev;
          cachedHits += 1;
        } else {
          const bytes = fs.readFileSync(absPath);
          const lang = languageForExtension(path.extname(normPath).toLowerCase());
          const src = decodeUtf8(bytes, normPath);
          const isArtifact = runtimeArtifacts.has(relPath) && matchesRuntimeArtifact(bytes, runtimeArtifacts.get(relPath));
          row = {
            mtimeMs: stat.mtimeMs,
            size: stat.size,
            effectiveLines: countEffectiveLines(src, lang),
            physicalLines: countPhysicalLines(src),
            sourceSha256: sha256Text(src.replace(/\r\n|\r/g, "\n")),
            isArtifact,
          };
          scannedCount += 1;
        }
        nextCache[normPath] = row;
        if (row.isArtifact) continue;

        const baseEntry = baselineByPath.get(normPath);
        const excEntry = exceptionByPath.get(normPath);
        if (row.effectiveLines > policy.thresholds.hard && (!baseEntry || row.effectiveLines > baseEntry.effectiveLines)) {
          violations.push(`${normPath}: ${row.effectiveLines} effective lines exceeds hard limit ${policy.thresholds.hard}`);
        } else if (baseEntry) {
          if (baseEntry.effectiveLines > policy.thresholds.soft && row.effectiveLines > baseEntry.effectiveLines) {
            violations.push(`${normPath}: legacy >700 file grew from ${baseEntry.effectiveLines} to ${row.effectiveLines}`);
          } else if (baseEntry.effectiveLines <= policy.thresholds.soft && row.effectiveLines > policy.thresholds.soft) {
            violations.push(`${normPath}: crossed 700 effective lines (${row.effectiveLines})`);
          }
        } else if (row.effectiveLines > policy.thresholds.acceptable && !excEntry) {
          violations.push(`${normPath}: new/migrated file has ${row.effectiveLines} effective lines (> ${policy.thresholds.acceptable})`);
        }
      }
    }
  }

  visit(resolvedRoot);
  if (scannedCount > 0 || Object.keys(prevCache).length !== Object.keys(nextCache).length) {
    fs.mkdirSync(path.dirname(cachePath), { recursive: true });
    fs.writeFileSync(cachePath, JSON.stringify({ version: 1, entries: nextCache }), "utf8");
  }

  const elapsedMs = Date.now() - startedAt;
  console.log(
    `[effective-lines:fast] ${Object.keys(nextCache).length} files (cached=${cachedHits}, lexed=${scannedCount}) in ${elapsedMs}ms`
  );
  if (violations.length > 0) {
    for (const v of violations) console.error(`  VIOLATION: ${v}`);
    process.exit(1);
  }
}

runFastRatchet();
