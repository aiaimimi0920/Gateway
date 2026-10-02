import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const readWorkflow = (name) => readFileSync(
  new URL(`../../.github/workflows/${name}`, import.meta.url), "utf8",
).replace(/\r\n/g, "\n");
const security = readWorkflow("security.yml");
const callers = [
  ["build-windows.yml", "build"],
  ["release-tag.yml", "release"],
  ["docker.yml", "docker"],
];

function job(source, name) {
  const marker = `  ${name}:\n`;
  const start = source.indexOf(marker, source.indexOf("\njobs:\n"));
  assert.notEqual(start, -1, `Missing ${name} job`);
  return source.slice(start + marker.length).split(/\n  [\w-]+:\n/, 1)[0];
}

test("Security can be called with a ref and returns the checked commit", () => {
  assert.match(security, /  workflow_call:\n    inputs:\n      ref:/);
  assert.match(security, /outputs:\n      source_sha:/);
  assert.ok(security.includes("value: ${{ jobs.policy.outputs.source_sha }}"));
  const policy = job(security, "policy");
  assert.ok(policy.includes("source_sha: ${{ steps.revision.outputs.source_sha }}"));
  assert.ok(policy.includes("ref: ${{ inputs.ref || github.sha }}"));
  assert.ok(policy.includes("id: revision"));
  assert.ok(policy.includes('source_sha="$(git rev-parse --verify HEAD)"'));
  assert.ok(policy.includes('echo "source_sha=$source_sha" >> "$GITHUB_OUTPUT"'));
});

test("dependency and secret scans use the resolved commit, not a mutable ref", () => {
  for (const name of ["dependencies", "secrets"]) {
    const block = job(security, name);
    assert.match(block, /^    needs: policy$/m);
    assert.ok(block.includes("ref: ${{ needs.policy.outputs.source_sha }}"));
    assert.ok(!block.includes("ref: ${{ github.ref }}"));
  }
  assert.match(job(security, "dependencies"), /fail-on-vuln: true/);
});

test("standalone and caller security runs cannot share a cancellation group", () => {
  assert.ok(security.includes("group: gateway-security-${{ github.workflow }}-"));
});

for (const [filename, publisher] of callers) {
  test(`${filename} waits for Security and builds exactly its checked commit`, () => {
    const workflow = readWorkflow(filename);
    const gate = job(workflow, "security");
    assert.ok(gate.includes("uses: ./.github/workflows/security.yml"));
    for (const permission of ["actions: read", "contents: read", "pull-requests: read", "security-events: write"]) {
      assert.ok(gate.includes(permission), `Missing ${permission}`);
    }
    assert.doesNotMatch(gate, /contents: write|packages: write|secrets: inherit/);
    const build = job(workflow, publisher);
    assert.match(build, /^    needs: security$/m);
    assert.ok(build.includes("ref: ${{ needs.security.outputs.source_sha }}"));
    assert.doesNotMatch(build, /^    if:/m, "Do not bypass failed or cancelled scan dependencies");
  });
}

test("manual tag releases resolve the requested tag while other callers use event SHA", () => {
  assert.ok(job(readWorkflow("release-tag.yml"), "security").includes(
    "ref: ${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.sha }}",
  ));
  for (const filename of ["build-windows.yml", "docker.yml"]) {
    assert.ok(job(readWorkflow(filename), "security").includes("ref: ${{ github.sha }}"));
  }
});

test("the reusable policy job runs both coverage and publication contracts", () => {
  assert.ok(job(security, "policy").includes(
    "node --test scripts/tests/security-ci-policy.test.mjs scripts/tests/security-release-gates.test.mjs",
  ));
});
