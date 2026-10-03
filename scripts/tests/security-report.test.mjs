import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";
import { validateExit, validateGitleaks, validateNpm, validateOsv } from "../security-report.mjs";
import { lockfiles, runCommand, scan } from "../security-scan.mjs";

const root = process.cwd();
const source = path.resolve(root, lockfiles[0]);
const sarif = (name = "osv-scanner", results = []) => ({ version: "2.1.0",
  runs: [{ tool: { driver: { name, rules: [{ id: "TEST-1" }] } }, results }] });
const finding = () => ({ ruleId: "TEST-1", message: { text: "Package 'example@1' is vulnerable to 'TEST-1'." },
  locations: [{ physicalLocation: { artifactLocation: { uri: pathToFileURL(source).href } } }] });
const json = () => ({ results: lockfiles.map((file) => ({ source: { type: "lockfile", path: path.resolve(root, file) },
  packages: [{ package: { name: "example", version: "1", ecosystem: "crates.io" } }] })) });
function reports(vulnerable = true) {
  const report = json();
  if (vulnerable) Object.assign(report.results[0].packages[0], {
    vulnerabilities: [{ id: "TEST-1", affected: [] }], groups: [{ ids: ["TEST-1"] }],
  });
  return { report, sarif: sarif("osv-scanner", vulnerable ? [finding()] : []) };
}
const check = ({ report, sarif: value }, code = 1, other = code) =>
  validateOsv(report, value, code, other, lockfiles, root);

test("complete clean and vulnerable OSV reports retain findings without failing development", () => {
  assert.equal(check(reports()), 1);
  assert.equal(check(reports(false), 0), 0);
  const alias = reports();
  alias.report.results[0].packages[0].vulnerabilities.push({ id: "TEST-ALIAS", affected: [] });
  alias.report.results[0].packages[0].groups[0].ids.push("TEST-ALIAS");
  alias.sarif.runs[0].results.push(finding());
  assert.equal(check(alias), 2, "Pinned OSV emits an occurrence for each advisory alias");
});

test("OSV rejects partial, malformed, suppressed and inconsistent reports", () => {
  const mutations = [
    (x) => { delete x.report.results; },
    (x) => { x.report.results.pop(); },
    (x) => { x.report.results[0].packages = []; },
    (x) => { x.report.results[0].packages[0].vulnerabilities = {}; },
    (x) => { x.report.results[0].packages[0].groups = []; },
    (x) => { x.report.results[0].packages[0].vulnerabilities[0].affected = null; },
    (x) => { x.report.errors = ["API unavailable"]; },
    (x) => { x.sarif.version = "2.0.0"; },
    (x) => { x.sarif.runs[0].results = []; },
    (x) => { x.sarif.runs[0].results[0].suppressions = [{ kind: "external" }]; },
    (x) => { x.sarif.runs[0].results[0].message.text = "Package 'different@2' is vulnerable to 'TEST-1'."; },
    (x) => { x.sarif.runs[0].results[0].locations[0].physicalLocation.artifactLocation.uri = "wrong.lock"; },
    (x) => { x.sarif.runs[0].invocations = [{ executionSuccessful: false }]; },
  ];
  for (const mutate of mutations) { const value = reports(); mutate(value); assert.throws(() => check(value)); }
  for (const code of [0, 2, 127, 128, 129, 130, null]) assert.throws(() => check(reports(), code));
  assert.throws(() => validateExit(undefined, 1));
  assert.throws(() => check(reports(), 1, 127));
  assert.throws(() => check(reports(false), 1));
});

test("duplicate package version cannot hide a different version with the same advisory", () => {
  const value = reports();
  const other = structuredClone(value.report.results[0].packages[0]);
  other.package.version = "2";
  value.report.results[0].packages.push(other);
  value.sarif.runs[0].results.push(finding());
  assert.throws(() => check(value));
  value.sarif.runs[0].results[1].message.text = "Package 'example@2' is vulnerable to 'TEST-1'.";
  assert.equal(check(value), 2);
});

test("scanner runner rejects missing or corrupt output, execution errors and stale report directories", () => {
  const temporary = mkdtempSync(path.join(tmpdir(), "gateway-scan-test-"));
  let index = 0;
  try {
    const invoke = (_binary, args) => {
      const output = args.find((arg) => arg.startsWith("--output-file=")).slice(14);
      writeFileSync(output, JSON.stringify(args.includes("--format=json") ? json() : sarif()));
      assert.ok(args.includes("--verbosity=error"));
      return { status: 0, stdout: "", stderr: "" };
    };
    const good = path.join(temporary, "good");
    assert.equal(scan("osv", "fake", good, root, invoke), 0);
    assert.equal(JSON.parse(readFileSync(path.join(good, "status.json"))).findings_free, true);
    assert.throws(() => scan("osv", "fake", good, root, invoke));
    for (const bad of [
      () => ({ status: 1, stderr: "" }),
      (_binary, args) => { writeFileSync(args.find((arg) => arg.startsWith("--output-file=")).slice(14), "{"); return { status: 1 }; },
      (...args) => ({ ...invoke(...args), status: 127 }),
      (...args) => ({ ...invoke(...args), stderr: "failed to parse one input" }),
      () => ({ status: null, signal: "SIGTERM" }),
      () => ({ status: null, error: new Error("ENOENT") }),
    ]) assert.throws(() => scan("osv", "fake", path.join(temporary, `bad-${index++}`), root, bad));
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});

test("Gitleaks reserves exit 2 for findings and strips secret-bearing metadata", () => {
  const value = sarif("gitleaks", [finding()]);
  const result = value.runs[0].results[0];
  result.partialFingerprints = { commitSha: "a".repeat(40), commitMessage: "private", email: "private" };
  result.locations[0].physicalLocation.region = { startLine: 1, endLine: 1, snippet: { text: "REDACTED" } };
  assert.equal(validateGitleaks(structuredClone(value), 2), 1);
  assert.throws(() => validateGitleaks(structuredClone(value), 1));
  const exposed = structuredClone(value);
  exposed.runs[0].results[0].locations[0].physicalLocation.region.snippet.text = "unredacted";
  assert.throws(() => validateGitleaks(exposed, 2));
  validateGitleaks(value, 2);
  assert.doesNotMatch(JSON.stringify(value), /private|snippet/);
  assert.equal(validateGitleaks(sarif("gitleaks"), 0), 0);
});

export function npmReport(severity = "high") {
  const counts = { info: 0, low: 0, moderate: 0, high: 0, critical: 0, total: severity ? 1 : 0 };
  if (severity) counts[severity] = 1;
  return { auditReportVersion: 2, vulnerabilities: severity ? {
    example: { name: "example", severity, via: ["dependency"], nodes: [], effects: [] },
  } : {}, metadata: { vulnerabilities: counts, dependencies: { total: 1 } } };
}

test("npm distinguishes high-severity audit exit from low-severity findings and API errors", () => {
  assert.equal(validateNpm(npmReport(), 1), 1);
  assert.equal(validateNpm(npmReport("low"), 0), 1);
  assert.equal(validateNpm(npmReport(null), 0), 0);
  for (const status of [0, 2, 127, null]) assert.throws(() => validateNpm(npmReport(), status));
  for (const report of [{}, { error: { code: "EAUDITNOLOCK" } },
    { ...npmReport(), error: { code: "ENETUNREACH" } },
    { ...npmReport(), vulnerabilities: {} }]) assert.throws(() => validateNpm(report, 1));
  assert.throws(() => validateExit(1, 0));
  assert.throws(() => runCommand("missing", [], root, () => ({ status: null, error: new Error("ENOENT") })));
});
