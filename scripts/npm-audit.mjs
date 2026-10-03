import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { validateNpm } from "./security-report.mjs";

export function auditNpm(mode, cwd, reportPath, invoke = spawnSync) {
  assert.ok(["strict", "advisory"].includes(mode), "Unknown audit policy");
  const result = invoke("npm", ["audit", "--omit=dev", "--audit-level=high", "--json"], {
    cwd, encoding: "utf8", windowsHide: true, maxBuffer: 64 * 1024 * 1024, timeout: 10 * 60 * 1000,
    env: { ...process.env, NO_COLOR: "1", NPM_CONFIG_COLOR: "false" },
  });
  assert.ok(!result.error && !result.signal && Number.isInteger(result.status), "Audit could not complete");
  assert.doesNotMatch(result.stderr ?? "", /^npm (?:ERR!|error)(?:\s|$)/m, "npm reported an execution error");
  const report = JSON.parse(result.stdout);
  const count = validateNpm(report, result.status);
  mkdirSync(path.dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  const exitCode = mode === "strict" ? result.status : 0;
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY,
    `### npm production audit (${mode})\n\nValidated report: **${count} findings**. ` +
    "Complete JSON is retained; execution and report errors remain failures.\n\n");
  return { report, count, exitCode };
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  try {
    const [mode = "strict", cwd = ".", reportPath = "artifacts/npm-audit.json"] = process.argv.slice(2);
    const result = auditNpm(mode, path.resolve(cwd), path.resolve(reportPath));
    console.log(`Validated npm audit (${mode}): ${result.count} findings.`);
    process.exitCode = result.exitCode;
  } catch {
    console.error("npm audit execution or complete-report validation failed.");
    process.exitCode = 2;
  }
}
