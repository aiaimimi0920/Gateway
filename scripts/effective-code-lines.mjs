import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  countEffectiveLines,
  countPhysicalLines,
  decodeUtf8,
  languageForExtension,
  SUPPORTED_EXTENSIONS,
} from "./effective-code-lines-lexer.mjs";
import { loadRuntimeArtifacts, matchesRuntimeArtifact } from "./effective-code-lines-runtime-artifacts.mjs";

export const CHECKER_VERSION = 2;

const FIXED_THRESHOLDS = Object.freeze({
  target: 150,
  acceptable: 500,
  soft: 700,
  hard: 1500,
});

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function baselineFilesSha256(files) {
  return sha256(Buffer.from(canonicalJson(files), "utf8"));
}

function policyContractSha256(policy) {
  const contract = { ...policy };
  delete contract.baselineFilesSha256;
  return sha256(Buffer.from(canonicalJson(contract), "utf8"));
}

export function normalizeRepoPath(value) {
  const normalized = value.replaceAll("\\", "/").replace(/^\.\//, "").replace(/\/$/, "");
  if (!normalized || normalized.startsWith("/") || /^[A-Za-z]:\//.test(normalized)) {
    throw new Error(`Path must be repository-relative: ${value}`);
  }
  const parts = normalized.split("/");
  if (parts.some((part) => !part || part === "." || part === "..")) {
    throw new Error(`Path is not canonical: ${value}`);
  }
  return normalized;
}

function validatePolicy(policy) {
  assert.equal(policy.schemaVersion, 1, "policy schemaVersion must be 1");
  assert.equal(policy.checkerVersion, CHECKER_VERSION, "policy checkerVersion mismatch");
  assert.deepEqual(policy.thresholds, FIXED_THRESHOLDS, "policy thresholds do not match the Gateway standard");
  assert.match(policy.baselineFilesSha256, /^[0-9a-f]{64}$/, "policy baselineFilesSha256 is invalid");
  assert.ok(Array.isArray(policy.sourceExtensions), "policy sourceExtensions must be an array");
  assert.equal(new Set(policy.sourceExtensions).size, policy.sourceExtensions.length, "duplicate source extension");
  for (const extension of policy.sourceExtensions) {
    assert.ok(SUPPORTED_EXTENSIONS.includes(extension), `unsupported source extension: ${extension}`);
  }
  for (const entry of [
    ...policy.excludedDirectories,
    ...policy.excludedDirectoryPrefixes,
    ...policy.excludedPaths,
    ...policy.excludedFilePrefixes,
  ]) {
    assert.ok(entry.reason?.trim(), "every exclusion requires a reason");
  }
  for (const entry of policy.excludedPaths) normalizeRepoPath(entry.path);
  for (const entry of policy.excludedFilePrefixes) {
    normalizeRepoPath(entry.directory);
    assert.ok(entry.prefix && !/[\\/]/.test(entry.prefix), "excluded file prefix must be a file-name prefix");
  }
}

function exclusionIndex(policy) {
  return {
    names: new Set(policy.excludedDirectories.map((entry) => entry.name.toLowerCase())),
    directoryPrefixes: policy.excludedDirectoryPrefixes.map((entry) => entry.prefix.toLowerCase()),
    paths: policy.excludedPaths.map((entry) => normalizeRepoPath(entry.path)),
    filePrefixes: policy.excludedFilePrefixes.map((entry) => ({
      directory: normalizeRepoPath(entry.directory),
      prefix: entry.prefix,
    })),
  };
}

function isExcluded(relativePath, entryName, isDirectory, exclusions) {
  const normalized = relativePath.replaceAll("\\", "/");
  const lowerName = entryName.toLowerCase();
  if (isDirectory && exclusions.names.has(lowerName)) return true;
  if (isDirectory && exclusions.directoryPrefixes.some((prefix) => lowerName.startsWith(prefix))) return true;
  if (exclusions.paths.some((prefix) => normalized === prefix || normalized.startsWith(`${prefix}/`))) return true;
  if (isDirectory) return false;
  const parent = path.posix.dirname(normalized);
  return exclusions.filePrefixes.some((entry) => parent === entry.directory && entryName.startsWith(entry.prefix));
}

function rowFromBytes(relativePath, bytes) {
  const extension = path.extname(relativePath).toLowerCase();
  const language = languageForExtension(extension);
  const source = decodeUtf8(bytes, relativePath);
  const canonicalSource = source.replace(/\r\n|\r/g, "\n");
  return {
    path: relativePath,
    language,
    effectiveLines: countEffectiveLines(source, language),
    physicalLines: countPhysicalLines(source),
    sourceSha256: sha256(Buffer.from(canonicalSource, "utf8")),
  };
}

export function scanRepository(root, policy) {
  validatePolicy(policy);
  const resolvedRoot = fs.realpathSync(root);
  const exclusions = exclusionIndex(policy);
  const extensions = new Set(policy.sourceExtensions);
  const runtimeArtifacts = loadRuntimeArtifacts(resolvedRoot, policy.immutableRuntimeArtifacts);
  const rows = [];
  const diagnostics = [];

  function visit(directory, relativeDirectory = "") {
    const entries = fs.readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name, "en"));
    for (const entry of entries) {
      const relativePath = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      if (isExcluded(relativePath, entry.name, entry.isDirectory(), exclusions)) continue;
      const absolutePath = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        diagnostics.push(`${relativePath}: symbolic links and junctions are not scanned`);
        continue;
      }
      if (entry.isDirectory()) {
        const realDirectory = fs.realpathSync(absolutePath);
        if (realDirectory !== resolvedRoot && !realDirectory.startsWith(`${resolvedRoot}${path.sep}`)) {
          diagnostics.push(`${relativePath}: directory escapes repository root`);
          continue;
        }
        visit(absolutePath, relativePath);
      } else if (entry.isFile() && extensions.has(path.extname(entry.name).toLowerCase())) {
        const bytes = fs.readFileSync(absolutePath);
        const row = rowFromBytes(normalizeRepoPath(relativePath), bytes);
        const artifact = runtimeArtifacts.get(relativePath);
        if (artifact && matchesRuntimeArtifact(bytes, artifact)) {
          row.runtimeArtifact = { itemId: artifact.itemId, version: artifact.version,
            rawSha256: artifact.sha256, evidencePath: artifact.evidencePath, evidenceSha256: artifact.evidenceSha256 };
        } else if (artifact) {
          diagnostics.push(`${relativePath}: immutable runtime artifact byte drift; provenance review required`);
        }
        rows.push(row);
      }
    }
  }

  visit(resolvedRoot);
  rows.sort((left, right) => left.path.localeCompare(right.path, "en"));
  return { rows, diagnostics };
}

function runGit(root, args, encoding = "utf8") {
  const result = spawnSync("git", ["-C", root, ...args], {
    encoding,
    maxBuffer: 128 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.status !== 0) {
    const stderr = Buffer.isBuffer(result.stderr) ? decodeUtf8(result.stderr, "git stderr") : result.stderr;
    throw new Error(`git ${args.join(" ")} failed: ${stderr?.trim() || `exit ${result.status}`}`);
  }
  return result.stdout;
}

function readJson(filePath, label) {
  const bytes = fs.readFileSync(filePath);
  try {
    return JSON.parse(decodeUtf8(bytes, label));
  } catch (error) {
    throw new Error(`${label} is invalid JSON: ${error.message}`);
  }
}

function canonicalTextSha256(filePath, label) {
  const source = decodeUtf8(fs.readFileSync(filePath), label).replace(/\r\n|\r/g, "\n");
  return sha256(Buffer.from(source, "utf8"));
}

function checkerSha256() {
  return canonicalTextSha256(fileURLToPath(import.meta.url), "effective-line checker");
}

function lexerSha256() {
  return canonicalTextSha256(fileURLToPath(new URL("./effective-code-lines-lexer.mjs", import.meta.url)), "effective-line lexer");
}

export function toolHashes() {
  return { checkerSha256: checkerSha256(), lexerSha256: lexerSha256(),
    runtimeArtifactsSha256: canonicalTextSha256(fileURLToPath(new URL("./effective-code-lines-runtime-artifacts.mjs", import.meta.url)), "runtime artifact classifier") };
}

function policySha256(policyPath) {
  return canonicalTextSha256(policyPath, "effective-line policy");
}

export function createBaseline(root, policy) {
  validatePolicy(policy);
  const { rows, diagnostics } = scanRepository(root, policy);
  if (diagnostics.length > 0) throw new Error(`cannot baseline an incomplete scan: ${diagnostics.join("; ")}`);
  const sourceCommit = runGit(root, ["rev-parse", "HEAD"]).trim();
  const sourceTree = runGit(root, ["rev-parse", "HEAD^{tree}"]).trim();
  const files = rows
    .filter((row) => row.effectiveLines > FIXED_THRESHOLDS.acceptable)
    .map(({ path: filePath, effectiveLines, physicalLines, sourceSha256 }) => ({
      path: filePath,
      effectiveLines,
      physicalLines,
      sourceSha256,
    }));
  const filesSha256 = baselineFilesSha256(files);
  return {
    schemaVersion: 1,
    checkerVersion: CHECKER_VERSION,
    sourceKind: "working-tree-adoption",
    sourceCommit,
    sourceTree,
    ...toolHashes(),
    policyContractSha256: policyContractSha256(policy),
    filesSha256,
    files,
  };
}

export function validateBaseline(root, policy, baseline) {
  assert.equal(baseline.schemaVersion, 1, "baseline schemaVersion must be 1");
  assert.equal(baseline.checkerVersion, CHECKER_VERSION, "baseline checkerVersion mismatch");
  assert.equal(baseline.sourceKind, "working-tree-adoption", "baseline sourceKind mismatch");
  assert.match(baseline.sourceCommit, /^[0-9a-f]{40}$/, "baseline sourceCommit is invalid");
  assert.match(baseline.sourceTree, /^[0-9a-f]{40}$/, "baseline sourceTree is invalid");
  assert.equal(
    runGit(root, ["rev-parse", `${baseline.sourceCommit}^{tree}`]).trim(),
    baseline.sourceTree,
    "baseline source commit no longer resolves to the recorded tree",
  );
  assert.equal(baseline.checkerSha256, checkerSha256(), "checker changed; regenerate and review baseline");
  assert.equal(baseline.lexerSha256, lexerSha256(), "lexer changed; regenerate and review baseline");
  assert.equal(baseline.runtimeArtifactsSha256, toolHashes().runtimeArtifactsSha256, "runtime artifact classifier changed; review baseline provenance");
  assert.equal(
    baseline.policyContractSha256,
    policyContractSha256(policy),
    "policy contract changed; regenerate and review baseline",
  );
  assert.equal(baseline.filesSha256, policy.baselineFilesSha256, "baseline file-list hash differs from policy");
  assert.equal(baseline.filesSha256, baselineFilesSha256(baseline.files), "baseline file-list hash is invalid");
  const extensions = new Set(policy.sourceExtensions);
  const exclusions = exclusionIndex(policy);
  const seen = new Set();
  let previousPath = "";
  for (const entry of baseline.files) {
    const relativePath = normalizeRepoPath(entry.path);
    const parts = relativePath.split("/");
    for (let index = 0; index < parts.length - 1; index += 1) {
      const prefix = parts.slice(0, index + 1).join("/");
      assert.ok(!isExcluded(prefix, parts[index], true, exclusions), `${relativePath}: baseline path is excluded by policy`);
    }
    assert.ok(
      !isExcluded(relativePath, parts.at(-1), false, exclusions),
      `${relativePath}: baseline path is excluded by policy`,
    );
    assert.ok(extensions.has(path.extname(relativePath).toLowerCase()), `${relativePath}: baseline path is not source`);
    assert.ok(!seen.has(relativePath), `duplicate baseline path: ${relativePath}`);
    assert.ok(!previousPath || previousPath.localeCompare(relativePath, "en") < 0, "baseline paths must be sorted");
    assert.ok(Number.isInteger(entry.effectiveLines), `${relativePath}: baseline effectiveLines must be an integer`);
    assert.ok(Number.isInteger(entry.physicalLines), `${relativePath}: baseline physicalLines must be an integer`);
    assert.ok(entry.effectiveLines > FIXED_THRESHOLDS.acceptable, `${relativePath}: baseline entry is not oversized`);
    assert.ok(entry.physicalLines >= entry.effectiveLines, `${relativePath}: baseline physicalLines is invalid`);
    assert.match(entry.sourceSha256, /^[0-9a-f]{64}$/, `${relativePath}: baseline source hash is invalid`);
    seen.add(relativePath);
    previousPath = relativePath;
  }
}

export function validateExceptions(exceptions, rows, policyPath, today) {
  assert.equal(exceptions.schemaVersion, 1, "exceptions schemaVersion must be 1");
  assert.equal(exceptions.checkerVersion, CHECKER_VERSION, "exceptions checkerVersion mismatch");
  assert.equal(exceptions.checkerSha256, checkerSha256(), "exceptions checker hash mismatch");
  assert.equal(exceptions.lexerSha256, lexerSha256(), "exceptions lexer hash mismatch");
  assert.equal(exceptions.runtimeArtifactsSha256, toolHashes().runtimeArtifactsSha256, "exceptions runtime artifact classifier hash mismatch");
  assert.equal(exceptions.policySha256, policySha256(policyPath), "exceptions policy hash mismatch");
  const byPath = new Map(rows.map((row) => [row.path, row]));
  const valid = new Map();
  for (const entry of exceptions.exceptions) {
    const relativePath = normalizeRepoPath(entry.path);
    assert.ok(!valid.has(relativePath), `duplicate exception path: ${relativePath}`);
    const row = byPath.get(relativePath);
    assert.ok(row, `${relativePath}: exception source does not exist`);
    assert.ok(row.effectiveLines > FIXED_THRESHOLDS.acceptable, `${relativePath}: exception is no longer needed`);
    assert.ok(row.effectiveLines <= FIXED_THRESHOLDS.soft, `${relativePath}: exceptions cannot exceed 700 lines`);
    assert.equal(entry.effectiveLines, row.effectiveLines, `${relativePath}: exception line count is stale`);
    assert.equal(entry.sourceSha256, row.sourceSha256, `${relativePath}: exception source hash is stale`);
    for (const field of ["responsibility", "reason", "owner", "approvedBy"]) {
      assert.ok(entry[field]?.trim(), `${relativePath}: exception ${field} is required`);
    }
    assert.notEqual(entry.owner, entry.approvedBy, `${relativePath}: exception requires an independent reviewer`);
    assert.match(entry.reviewBy, /^\d{4}-\d{2}-\d{2}$/, `${relativePath}: exception reviewBy is invalid`);
    assert.ok(entry.reviewBy >= today, `${relativePath}: exception expired on ${entry.reviewBy}`);
    assert.ok(Array.isArray(entry.tests) && entry.tests.length > 0, `${relativePath}: exception tests are required`);
    valid.set(relativePath, entry);
  }
  return valid;
}

export function evaluateRows(rows, baseline, validExceptions, mode) {
  const baselineByPath = new Map(baseline.files.map((entry) => [entry.path, entry]));
  const violations = [];
  const warnings = [];
  for (const row of rows) {
    if (row.runtimeArtifact) continue;
    const old = baselineByPath.get(row.path);
    const unchanged = old
      && old.effectiveLines === row.effectiveLines
      && old.sourceSha256 === row.sourceSha256;
    if (row.effectiveLines > FIXED_THRESHOLDS.soft) {
      if (mode === "strict") {
        violations.push(`${row.path}: ${row.effectiveLines} effective lines exceeds strict limit 700`);
      } else if (!old) {
        violations.push(`${row.path}: oversized file is absent from the adoption baseline`);
      } else if (row.effectiveLines > old.effectiveLines) {
        violations.push(`${row.path}: legacy debt grew from ${old.effectiveLines} to ${row.effectiveLines} lines`);
      } else {
        const severity = row.effectiveLines > FIXED_THRESHOLDS.hard ? "hard-cap" : "mandatory";
        const state = unchanged ? "unchanged" : `non-growing (${old.effectiveLines} -> ${row.effectiveLines})`;
        warnings.push(`${row.path}: ${state} ${severity} migration debt`);
      }
    } else if (row.effectiveLines > FIXED_THRESHOLDS.acceptable) {
      if (!unchanged && !validExceptions.has(row.path)) {
        violations.push(`${row.path}: ${row.effectiveLines} lines requires a current 501-700 exception`);
      } else if (unchanged) {
        warnings.push(`${row.path}: unchanged 501-700 migration debt (${row.effectiveLines})`);
      }
    }
  }
  return { violations, warnings };
}

function parseArguments(argv) {
  const options = { mode: "ratchet", writeBaseline: false, acknowledgeAdoptionSnapshot: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--write-baseline") options.writeBaseline = true;
    else if (argument === "--acknowledge-adoption-snapshot") options.acknowledgeAdoptionSnapshot = true;
    else if (["--mode", "--policy", "--baseline", "--exceptions", "--json", "--root"].includes(argument)) {
      options[argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = argv[++index];
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  if (!["ratchet", "strict", "report"].includes(options.mode)) throw new Error(`Invalid mode: ${options.mode}`);
  return options;
}

export function resolveInsideRoot(root, value, fallback) {
  const relativePath = normalizeRepoPath(value || fallback);
  const resolved = path.resolve(root, ...relativePath.split("/"));
  assert.ok(resolved.startsWith(`${root}${path.sep}`), `${relativePath}: path escapes repository root`);
  return resolved;
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export function summarizeRows(rows) {
  return {
    scanned: rows.length,
    hard: rows.filter((row) => row.effectiveLines > FIXED_THRESHOLDS.hard).length,
    mandatory: rows.filter((row) => row.effectiveLines > FIXED_THRESHOLDS.soft && row.effectiveLines <= FIXED_THRESHOLDS.hard).length,
    soft: rows.filter((row) => row.effectiveLines > FIXED_THRESHOLDS.acceptable && row.effectiveLines <= FIXED_THRESHOLDS.soft).length,
  };
}

export async function main(argv = process.argv.slice(2)) {
  const options = parseArguments(argv);
  const root = fs.realpathSync(options.root || path.resolve(path.dirname(fileURLToPath(import.meta.url)), ".."));
  const policyPath = resolveInsideRoot(root, options.policy, "scripts/effective-code-lines-policy.json");
  const baselinePath = resolveInsideRoot(root, options.baseline, "scripts/effective-code-lines-baseline.json");
  const exceptionsPath = resolveInsideRoot(root, options.exceptions, "scripts/effective-code-lines-exceptions.json");
  const policy = readJson(policyPath, "effective-line policy");
  validatePolicy(policy);

  if (options.writeBaseline) {
    assert.ok(
      options.acknowledgeAdoptionSnapshot,
      "--write-baseline requires --acknowledge-adoption-snapshot and an explicit governance review",
    );
    const baseline = createBaseline(root, policy);
    policy.baselineFilesSha256 = baseline.filesSha256;
    writeJson(policyPath, policy);
    writeJson(baselinePath, baseline);
    console.log(`Wrote ${baseline.files.length} baseline entries to ${baselinePath}`);
    return 0;
  }

  const baseline = readJson(baselinePath, "effective-line baseline");
  validateBaseline(root, policy, baseline);
  const { rows, diagnostics } = scanRepository(root, policy);
  const today = new Date().toISOString().slice(0, 10);
  const exceptions = readJson(exceptionsPath, "effective-line exceptions");
  const validExceptions = validateExceptions(exceptions, rows, policyPath, today);
  const evaluation = evaluateRows(rows, baseline, validExceptions, options.mode);
  const violations = [...diagnostics, ...evaluation.violations];
  const summary = summarizeRows(rows);
  const report = {
    schemaVersion: 1,
    checkerVersion: CHECKER_VERSION,
    mode: options.mode,
    thresholds: FIXED_THRESHOLDS,
    baseline: {
      sourceKind: baseline.sourceKind,
      sourceCommit: baseline.sourceCommit,
      sourceTree: baseline.sourceTree,
      files: baseline.files.length,
    },
    summary,
    sourceSummary: summarizeRows(rows.filter((row) => !row.runtimeArtifact)),
    runtimeArtifacts: rows.filter((row) => row.runtimeArtifact),
    violations,
    warnings: evaluation.warnings,
    files: rows.filter((row) => row.effectiveLines > FIXED_THRESHOLDS.acceptable)
      .sort((left, right) => right.effectiveLines - left.effectiveLines || left.path.localeCompare(right.path, "en")),
  };
  if (options.json) writeJson(resolveInsideRoot(root, options.json, options.json), report);
  console.log(`Effective lines: ${summary.scanned} files; >1500=${summary.hard}, 701-1500=${summary.mandatory}, 501-700=${summary.soft}`);
  console.log(`Classified immutable runtime artifacts: ${report.runtimeArtifacts.length}; governed source >700=${report.sourceSummary.hard + report.sourceSummary.mandatory}`);
  for (const violation of violations) console.error(`ERROR: ${violation}`);
  if (options.mode !== "report" && violations.length > 0) return 1;
  return 0;
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : undefined;
if (invokedPath === import.meta.url) {
  main().then((exitCode) => {
    process.exitCode = exitCode;
  }).catch((error) => {
    console.error(`Effective-line checker failed: ${error.stack || error.message}`);
    process.exitCode = 2;
  });
}
