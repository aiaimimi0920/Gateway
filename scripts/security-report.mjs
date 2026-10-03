import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

const object = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const text = (value) => typeof value === "string" && value.length > 0;
const list = (value) => { assert.ok(Array.isArray(value), "Expected report array"); return value; };

export function validateExit(status, count, findingExit = 1) {
  assert.ok(status === 0 || status === findingExit, "Scanner execution failed");
  assert.equal(status === findingExit, count > 0, "Exit status and report disagree");
}

export function validateSarif(report, tool) {
  assert.equal(report?.version, "2.1.0", "Invalid SARIF version");
  assert.equal(list(report.runs).length, 1, "Expected one complete SARIF run");
  const run = report.runs[0];
  assert.equal(run.tool?.driver?.name.toLowerCase(), tool);
  const rules = list(run.tool.driver.rules);
  assert.ok(rules.every((rule) => text(rule.id)), "Invalid SARIF rules");
  const ids = new Set(rules.map((rule) => rule.id));
  for (const invocation of run.invocations ?? []) {
    assert.notEqual(invocation.executionSuccessful, false, "SARIF records execution failure");
    assert.ok(!(invocation.toolExecutionNotifications ?? []).some((item) => item.level === "error"));
  }
  for (const result of list(run.results)) {
    assert.ok(ids.has(result.ruleId) && text(result.message?.text), "Invalid SARIF result");
    assert.ok(list(result.locations).length > 0, "Missing finding location");
    for (const location of result.locations) {
      assert.ok(text(location.physicalLocation?.artifactLocation?.uri), "Missing source URI");
    }
    assert.ok(!result.suppressions?.length, "Unexpected suppressed findings");
  }
  return run;
}

export function validateOsv(report, sarif, status, sarifStatus, locks, root) {
  assert.ok(object(report), "Invalid OSV report");
  assert.ok(!report.error && !report.errors, "OSV report records an error");
  const expected = new Set(locks.map((lock) => path.resolve(root, lock)));
  const seen = new Set();
  const occurrences = [];
  for (const source of list(report.results)) {
    assert.equal(source.source?.type, "lockfile");
    const file = path.resolve(source.source.path);
    assert.ok(expected.has(file) && !seen.has(file), "Unexpected or duplicate lock source");
    seen.add(file);
    assert.ok(list(source.packages).length > 0, "Empty lock scan");
    for (const pkg of source.packages) {
      assert.ok(text(pkg.package?.name) && text(pkg.package.version) && text(pkg.package.ecosystem));
      const vulnerabilities = list(pkg.vulnerabilities ?? []);
      assert.ok(vulnerabilities.every((vuln) => text(vuln.id) && Array.isArray(vuln.affected)));
      const ids = vulnerabilities.map((vuln) => vuln.id);
      const groups = list(pkg.groups ?? []);
      assert.deepEqual(groups.flatMap((group) => list(group.ids)).sort(), [...ids].sort(), "Missing vulnerability group");
      for (const group of groups) {
        for (const id of group.ids) occurrences.push({ file, id,
          ids: [...group.ids, ...(group.aliases ?? [])], package: `${pkg.package.name}@${pkg.package.version}` });
      }
    }
  }
  assert.deepEqual(seen, expected, "Incomplete lock coverage");
  const run = validateSarif(sarif, "osv-scanner");
  const remaining = [...occurrences];
  for (const result of run.results) {
    const rule = run.tool.driver.rules.find((item) => item.id === result.ruleId);
    const ids = [rule.id, ...(rule.deprecatedIds ?? [])];
    for (const location of result.locations) {
      const uri = location.physicalLocation.artifactLocation.uri;
      const file = uri.startsWith("file:") ? fileURLToPath(uri) : path.resolve(root, decodeURIComponent(uri));
      const index = remaining.findIndex((item) => item.file === file && item.ids.some((id) => ids.includes(id)) &&
        result.message.text.startsWith(`Package '${item.package}' is vulnerable to `));
      assert.notEqual(index, -1, "SARIF and JSON findings differ");
      remaining.splice(index, 1);
    }
  }
  assert.equal(remaining.length, 0, "SARIF omitted JSON findings");
  validateExit(status, occurrences.length);
  validateExit(sarifStatus, occurrences.length);
  return occurrences.length;
}

export function validateGitleaks(sarif, status) {
  const run = validateSarif(sarif, "gitleaks");
  validateExit(status, run.results.length, 2);
  // Publish locations and rule identities, never source snippets or commit messages.
  run.results = run.results.map((result) => {
    const commit = result.partialFingerprints?.commitSha;
    assert.match(commit, /^[a-f0-9]{40}$/);
    const locations = result.locations.map(({ physicalLocation: location }) => {
      const region = location.region;
      assert.ok(Number.isInteger(region?.startLine) && region.startLine > 0);
      assert.ok(!region.snippet?.text || region.snippet.text === "REDACTED", "Secret was not redacted");
      return { physicalLocation: { artifactLocation: location.artifactLocation,
        region: { startLine: region.startLine, endLine: region.endLine } } };
    });
    return { ruleId: result.ruleId, message: { text: "Potential secret; rotate and review the recorded location." },
      locations, partialFingerprints: { commitSha: commit } };
  });
  return run.results.length;
}

export function validateNpm(report, status) {
  assert.equal(report?.auditReportVersion, 2);
  assert.ok(!report.error && object(report.vulnerabilities) && object(report.metadata?.vulnerabilities));
  const counts = report.metadata.vulnerabilities;
  const levels = ["info", "low", "moderate", "high", "critical"];
  for (const key of [...levels, "total"]) assert.ok(Number.isInteger(counts[key]) && counts[key] >= 0);
  const entries = Object.values(report.vulnerabilities);
  for (const entry of entries) {
    assert.ok(text(entry.name) && levels.includes(entry.severity));
    assert.ok(list(entry.via).length > 0 && Array.isArray(entry.nodes) && Array.isArray(entry.effects));
  }
  assert.equal(entries.length, counts.total);
  for (const level of levels) assert.equal(entries.filter((entry) => entry.severity === level).length, counts[level]);
  assert.ok(Number.isInteger(report.metadata.dependencies?.total) && report.metadata.dependencies.total > 0);
  validateExit(status, counts.high + counts.critical);
  return counts.total;
}
