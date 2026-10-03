import assert from "node:assert/strict";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { evaluateRows, scanRepository, summarizeRows, validateBaseline,
  validateExceptions } from "./effective-code-lines.mjs";
import { runCommand } from "./security-scan.mjs";
import { validateExit, validateNpm } from "./security-report.mjs";

export function formattingReport(invoke, root) {
  for (const args of [["fmt", "--all"], ["fmt", "--manifest-path", "apps/desktop/src-tauri/Cargo.toml"]]) {
    const result = runCommand("cargo", args, root, invoke);
    // Successful formatting produces the evidence. Syntax/execution failures never qualify.
    assert.equal(result.status, 0, "Formatter failed");
  }
  const diff = runCommand("git", ["diff", "--numstat", "-z", "--no-renames", "--", "*.rs"], root, invoke);
  assert.equal(diff.status, 0, "Unable to read formatting changes");
  return diff.stdout.split("\0").filter(Boolean).map((row) => {
    const match = row.match(/^(\d+)\t(\d+)\t([^\0]+)$/);
    assert.ok(match, "Malformed formatting diff");
    return { file: match[3], added: Number(match[1]), removed: Number(match[2]) };
  });
}

export function validateLines(report, status, scan, baseline, policy, exceptions) {
  assert.equal(report?.schemaVersion, 1);
  assert.equal(report.checkerVersion, 2);
  assert.equal(report.mode, "ratchet");
  assert.ok(Array.isArray(report.violations) && report.violations.every((item) => typeof item === "string"));
  assert.ok(Array.isArray(report.warnings) && Array.isArray(report.files));
  assert.equal(scan.diagnostics.length, 0, "Incomplete source scan or artifact integrity error");
  assert.ok(scan.rows.length > 0 && report.summary?.scanned === scan.rows.length);
  assert.deepEqual(report.thresholds, policy.thresholds);
  assert.deepEqual(report.baseline, { sourceKind: baseline.sourceKind, sourceCommit: baseline.sourceCommit,
    sourceTree: baseline.sourceTree, files: baseline.files.length });
  assert.deepEqual(report.summary, summarizeRows(scan.rows));
  assert.deepEqual(report.sourceSummary, summarizeRows(scan.rows.filter((row) => !row.runtimeArtifact)));
  assert.deepEqual(report.runtimeArtifacts, scan.rows.filter((row) => row.runtimeArtifact));
  assert.deepEqual(report.files, scan.rows.filter((row) => row.effectiveLines > policy.thresholds.acceptable)
    .sort((left, right) => right.effectiveLines - left.effectiveLines || left.path.localeCompare(right.path, "en")));
  const expected = evaluateRows(scan.rows, baseline, exceptions, "ratchet");
  assert.deepEqual(report.violations, expected.violations);
  assert.deepEqual(report.warnings, expected.warnings);
  validateExit(status, report.violations.length);
  return report.violations.length;
}

export function quality(kind, root = process.cwd(), invoke, sourceScan = scanRepository) {
  assert.ok(["npm-scripts", "npm-desktop", "format", "lines"].includes(kind));
  const directory = path.join(root, "artifacts", `quality-${kind}`);
  mkdirSync(directory, { recursive: true });
  const reportPath = path.join(directory, "report.json");
  let report;
  let count;
  if (kind.startsWith("npm-")) {
    const cwd = path.join(root, kind === "npm-scripts" ? "scripts" : "apps/desktop");
    const result = runCommand("npm", ["audit", "--omit=dev", "--audit-level=high", "--json"], cwd, invoke);
    assert.doesNotMatch(result.stderr ?? "", /^npm (?:ERR!|error)\b/m, "npm reported an execution error");
    report = JSON.parse(result.stdout);
    count = validateNpm(report, result.status);
  } else if (kind === "format") {
    report = { files: formattingReport(invoke, root) };
    count = report.files.length;
  } else {
    const relative = path.relative(root, reportPath).replaceAll("\\", "/");
    const result = runCommand(process.execPath,
      ["scripts/effective-code-lines.mjs", "--mode", "ratchet", "--json", relative], root, invoke);
    report = JSON.parse(readFileSync(reportPath, "utf8"));
    const policyPath = path.join(root, "scripts/effective-code-lines-policy.json");
    const policy = JSON.parse(readFileSync(policyPath, "utf8"));
    const baseline = JSON.parse(readFileSync(path.join(root, "scripts/effective-code-lines-baseline.json"), "utf8"));
    validateBaseline(root, policy, baseline);
    const scan = sourceScan(root, policy);
    const exceptions = validateExceptions(JSON.parse(readFileSync(
      path.join(root, "scripts/effective-code-lines-exceptions.json"), "utf8")),
    scan.rows, policyPath, new Date().toISOString().slice(0, 10));
    count = validateLines(report, result.status, scan, baseline, policy, exceptions);
  }
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY,
    `### Development quality: ${kind}\n\nValidated report: **${count} findings**. ` +
    "Findings are advisory; see the complete report artifact. Execution/report/upload errors remain failures.\n\n");
  console.log(`Validated ${kind}: ${count} advisory findings.`);
  return count;
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  try { quality(process.argv[2]); } catch {
    console.error("Development quality tool failed to execute or produce a valid complete report.");
    process.exitCode = 2;
  }
}
