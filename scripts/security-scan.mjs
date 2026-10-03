import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { validateGitleaks, validateOsv } from "./security-report.mjs";

export const lockfiles = ["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock",
  "crates/gateway-local-data/Cargo.lock", "apps/desktop/package-lock.json", "scripts/package-lock.json"];

export function runCommand(command, args, cwd, invoke = spawnSync) {
  const result = invoke(command, args, { cwd, encoding: "utf8", windowsHide: true,
    maxBuffer: 64 * 1024 * 1024, timeout: 15 * 60 * 1000, env: { ...process.env, NO_COLOR: "1" } });
  assert.ok(!result.error && !result.signal && Number.isInteger(result.status), "Tool could not complete");
  return result;
}

function scannerCommand(binary, args, root, invoke) {
  const result = runCommand(binary, args, root, invoke);
  // Both tools run at error-only verbosity. OSV may return 1 before checking
  // its logger flag, so even a valid finding report cannot override diagnostics.
  assert.equal((result.stderr ?? "").trim(), "", "Scanner reported an execution error");
  return result;
}

export function summary(name, count, directory) {
  const report = { tool: name, findings: count, findings_free: count === 0 };
  writeFileSync(path.join(directory, "status.json"), `${JSON.stringify(report, null, 2)}\n`);
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `findings_free=${count === 0}\n`);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY,
    `### ${name}\n\nValidated report: **${count} finding occurrences**. ` +
    "Development findings are advisory; release callers remain strict. " +
    "See the complete uploaded report and repository security alerts.\n\n");
}

export function scan(kind, binary, directory, root = process.cwd(), invoke) {
  assert.ok(["osv", "gitleaks"].includes(kind), "Unknown scanner");
  mkdirSync(directory, { recursive: false });
  const sarifPath = path.join(directory, "results.sarif");
  let count;
  if (kind === "osv") {
    const jsonPath = path.join(directory, "results.json");
    const args = ["scan", "source", "--verbosity=error", "--all-packages", "--all-vulns",
      ...lockfiles.map((file) => `--lockfile=./${file}`)];
    const jsonRun = scannerCommand(binary, [...args, "--format=json", `--output-file=${jsonPath}`], root, invoke);
    const report = JSON.parse(readFileSync(jsonPath, "utf8"));
    // No reporter fallback: both scanner runs must complete and agree exactly.
    const sarifRun = scannerCommand(binary, [...args, "--format=sarif", `--output-file=${sarifPath}`], root, invoke);
    count = validateOsv(report, JSON.parse(readFileSync(sarifPath, "utf8")),
      jsonRun.status, sarifRun.status, lockfiles, root);
  } else {
    const result = scannerCommand(binary, ["git", "--redact=100", "--no-banner", "--exit-code=2",
      "--log-level=error", "--report-format=sarif", `--report-path=${sarifPath}`, "--log-opts=HEAD"], root, invoke);
    const report = JSON.parse(readFileSync(sarifPath, "utf8"));
    count = validateGitleaks(report, result.status);
    writeFileSync(sarifPath, `${JSON.stringify(report, null, 2)}\n`);
  }
  summary(kind, count, directory);
  return count;
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  try {
    const [kind, binary, directory] = process.argv.slice(2);
    assert.ok(binary && directory);
    scan(kind, path.resolve(binary), path.resolve(directory));
  } catch {
    // Errors may embed scanner input or a secret; never print raw tool/assertion output.
    console.error("Security scan failed: execution, coverage, exit status or report validation error.");
    process.exitCode = 2;
  }
}
