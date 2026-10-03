import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { formattingReport, validateLines } from "../development-quality.mjs";
import { evaluateRows, summarizeRows } from "../effective-code-lines.mjs";

const read = (name) => readFileSync(new URL(`../../${name}`, import.meta.url), "utf8").replace(/\r\n/g, "\n");
test("product validation is independent of advisory checks and preserves functional failures", () => {
  const workflow = read(".github/workflows/ci.yml");
  const products = workflow.split("\n  quality:\n")[0];
  assert.doesNotMatch(products, /needs:|continue-on-error|cargo fmt|run audit:prod|run check:effective-lines/);
  for (const command of ["cargo check --locked --all-targets", "cargo test --locked",
    "npm run typecheck", "npm run build", "npm test -- --run", "node --test scripts/tests/*.test.mjs",
    "cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml"]) {
    assert.equal(products.split(command).length - 1, 2, command);
  }
  assert.match(workflow, /check: \[npm-scripts, npm-desktop, format, lines\]/);
  assert.match(workflow, /fail-fast: false/);
  assert.match(workflow, /if-no-files-found: error/);
  assert.doesNotMatch(workflow, /continue-on-error/);
});

test("release paths keep npm audits and formatting gates strict", () => {
  for (const file of ["build-windows.yml", "release-tag.yml", "docker.yml"]) {
    const workflow = read(`.github/workflows/${file}`);
    assert.match(workflow, /run: npm run audit:prod --prefix scripts/);
    assert.match(workflow, /run: npm run audit:prod --prefix apps\/desktop/);
    assert.doesNotMatch(workflow, /continue-on-error|development-quality/);
  }
  const release = read(".github/workflows/release-tag.yml");
  assert.match(release, /cargo fmt --all -- --check/);
  assert.match(release, /cargo fmt --manifest-path apps\/desktop\/src-tauri\/Cargo.toml -- --check/);
  assert.equal((read("Dockerfile").match(/npm run audit:prod/g) ?? []).length, 2);
});

test("format differences require successful formatters and a valid diff; parse errors stay failures", () => {
  const invoke = (command) => ({ status: 0, stderr: "", stdout: command === "git" ? "2\t3\tsrc/file.rs\0" : "" });
  assert.deepEqual(formattingReport(invoke, process.cwd()), [{ file: "src/file.rs", added: 2, removed: 3 }]);
  assert.deepEqual(formattingReport(() => ({ status: 0, stdout: "" }), process.cwd()), []);
  for (const status of [1, 2, 127, null]) {
    assert.throws(() => formattingReport(() => ({ status, stdout: "" }), process.cwd()));
  }
  assert.throws(() => formattingReport((command) => ({ status: 0, stdout: command === "git" ? "invalid" : "" }), process.cwd()));
});

test("line audit requires a complete report agreeing with an independent source scan", () => {
  const scan = { diagnostics: [], rows: [{ path: "src/file.rs", effectiveLines: 800, physicalLines: 810, sourceSha256: "x" }] };
  const baseline = { files: [], sourceKind: "working-tree-adoption", sourceCommit: "a".repeat(40), sourceTree: "b".repeat(40) };
  const policy = { thresholds: { target: 150, acceptable: 500, soft: 700, hard: 1500 } };
  const exceptions = new Map();
  const report = { schemaVersion: 1, checkerVersion: 2, mode: "ratchet", thresholds: policy.thresholds,
    baseline: { ...baseline, files: 0 }, summary: summarizeRows(scan.rows), sourceSummary: summarizeRows(scan.rows),
    runtimeArtifacts: [], files: scan.rows, ...evaluateRows(scan.rows, baseline, exceptions, "ratchet") };
  const check = (value, status = 1, source = scan) => validateLines(value, status, source, baseline, policy, exceptions);
  assert.equal(check(report), 1);
  assert.throws(() => check(report, 2));
  assert.throws(() => check(report, 0));
  assert.throws(() => check(report, 1, { ...scan, diagnostics: ["incomplete scan"] }));
  for (const field of ["files", "summary", "sourceSummary", "runtimeArtifacts", "baseline", "thresholds", "violations", "warnings"]) {
    const corrupt = structuredClone(report); delete corrupt[field]; assert.throws(() => check(corrupt));
  }
  for (const field of ["files", "violations"]) {
    const corrupt = structuredClone(report); corrupt[field] = []; assert.throws(() => check(corrupt));
  }
});

test("advisory scanners upload evidence before evaluating strict release findings", () => {
  for (const file of ["security.yml", "osv-scan.yml"]) {
    const workflow = read(`.github/workflows/${file}`);
    assert.ok(workflow.indexOf("uses: actions/upload-artifact@") < workflow.indexOf("name: Enforce findings"));
    assert.match(workflow, /enforce_findings:[\s\S]*type: boolean\n        default: true/);
    assert.doesNotMatch(workflow, /continue-on-error/);
  }
  const osv = read(".github/workflows/osv-scan.yml");
  assert.ok(osv.indexOf("uses: github/codeql-action/upload-sarif@") < osv.indexOf("name: Enforce findings"));
  assert.match(osv, /wait-for-processing: true/);
  for (const match of osv.matchAll(/uses: (\S+)/g)) assert.match(match[1], /@[a-f0-9]{40}$/);
});
