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
const windowsBuildGuard = "${{ needs.security.result == 'success' && " +
  "needs.security.outputs.findings_free == 'true' && " +
  "((github.event_name == 'push' && github.ref == 'refs/heads/main') || " +
  "(github.event_name == 'workflow_dispatch' && toJSON(inputs.scan_only) == 'false')) }}";
const dockerBuildGuard = "${{ needs.security.result == 'success' && " +
  "(github.event_name == 'pull_request' || needs.security.outputs.findings_free == 'true') }}";

function job(source, name) {
  const marker = `  ${name}:\n`;
  const start = source.indexOf(marker, source.indexOf("\njobs:\n"));
  assert.notEqual(start, -1, `Missing ${name} job`);
  return source.slice(start + marker.length).split(/\n  [\w-]+:\n/, 1)[0];
}

function assertWindowsBuildGuard(build) {
  const guards = [...build.matchAll(/^    if: >-\n((?:      .+\n)+)/gm)];
  assert.equal(guards.length, 1, "Expected one explicit Windows build guard");
  assert.equal((build.match(/^    if:/gm) ?? []).length, 1);
  assert.equal(guards[0][1].trim().replace(/\s+/g, " "), windowsBuildGuard);
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
  assert.ok(job(security, "dependencies").includes("enforce_findings: ${{ toJSON(inputs.enforce_findings) == 'true' }}"));
});

test("standalone and caller security runs cannot share a cancellation group", () => {
  assert.ok(security.includes("group: gateway-security-${{ github.workflow }}-"));
});

test("Docker builder and metadata actions remain pinned to immutable commits", () => {
  const workflow = readWorkflow("docker.yml");
  for (const action of ["setup-buildx-action", "metadata-action"]) {
    const uses = [...workflow.matchAll(new RegExp(`uses: docker/${action}@([^\\s]+)`, "g"))];
    assert.ok(uses.length > 0, `Missing Docker action: ${action}`);
    for (const [, ref] of uses) {
      assert.match(ref, /^[a-f0-9]{40}$/, `Docker action ${action} must use a full commit SHA`);
    }
  }
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
    if (filename === "build-windows.yml") assertWindowsBuildGuard(build);
    else if (filename === "docker.yml") {
      assert.ok(gate.includes("enforce_findings: ${{ github.event_name != 'pull_request' }}"));
      assert.equal(build.match(/^    if: (.+)$/m)?.[1], dockerBuildGuard);
    } else {
      assert.ok(gate.includes("enforce_findings: true"));
      assert.match(build, /^    if: \$\{\{ needs\.security\.result == 'success' && needs\.security\.outputs\.findings_free == 'true' \}\}$/m);
    }
  });
}

test("Windows scans every main PR with the existing caller and no path filters", () => {
  const workflow = readWorkflow("build-windows.yml");
  const triggers = workflow.split("\non:\n")[1].split("\npermissions:\n")[0];
  assert.match(triggers, /^  pull_request:\n    branches:\n      - main\n/m);
  assert.doesNotMatch(triggers, /paths(?:-ignore)?:|pull_request_target:|types:/);
  assert.match(triggers, /scan_only:\n        description: [^\n]+\n        type: boolean\n        required: false\n        default: true/);
  assert.deepEqual([...workflow.matchAll(/^  ([\w-]+):\n/gm)]
    .map((match) => match[1]), ["push", "pull_request", "workflow_dispatch", "security", "build"]);
  assert.doesNotMatch(job(workflow, "security"), /^    if:/m);
  assert.doesNotMatch(job(workflow, "security"), /continue-on-error|secrets:/);
});

test("Windows guard rejects bypasses and weakened input checks", () => {
  const build = job(readWorkflow("build-windows.yml"), "build");
  for (const unsafe of [
    build.replace("needs.security.result == 'success' &&", "always() &&"),
    build.replace("needs.security.result == 'success' &&", ""),
    build.replace("needs.security.outputs.findings_free == 'true' &&", ""),
    build.replace("github.event_name == 'push'", "github.event_name == 'pull_request'"),
    build.replace(" && github.ref == 'refs/heads/main'", ""),
    build.replace("toJSON(inputs.scan_only) == 'false'", "inputs.scan_only == false"),
    build.replace("toJSON(inputs.scan_only) == 'false'", "!inputs.scan_only"),
  ]) assert.throws(() => assertWindowsBuildGuard(unsafe));
});

test("Windows build guard permits only successful main pushes or explicit build dispatches", () => {
  const build = job(readWorkflow("build-windows.yml"), "build");
  assertWindowsBuildGuard(build);
  // Evaluate the actual expression after the exact allowlist check above. All
  // equality operands are strings, including toJSON's type-preserving output.
  const expression = build.match(/^    if: >-\n((?:      .+\n)+)/m)[1]
    .trim().slice(3, -2).trim().replace(/==/g, "===");
  const evaluate = new Function("needs", "github", "inputs", "toJSON", `return (${expression});`);
  for (const result of ["success", "failure", "cancelled", "skipped", ""]) {
    for (const findings_free of ["true", "false", "", null, undefined]) {
      for (const event_name of ["pull_request", "pull_request_target", "push", "workflow_dispatch", "schedule"]) {
        for (const ref of ["refs/heads/main", "refs/heads/topic", "refs/tags/V1.2.3", "refs/pull/19/merge"]) {
          for (const scan_only of [true, false, undefined, null, "false", "true", "", 0, 1]) {
            const expected = result === "success" && findings_free === "true" &&
              ((event_name === "push" && ref === "refs/heads/main") ||
               (event_name === "workflow_dispatch" && scan_only === false));
            const inputs = scan_only === undefined ? {} : { scan_only };
            assert.equal(evaluate({ security: { result, outputs: { findings_free } } }, { event_name, ref }, inputs,
              (value) => JSON.stringify(value === undefined ? "" : value)), expected,
            JSON.stringify({ result, event_name, ref, inputs }));
          }
        }
      }
    }
  }
});

test("manual tag releases resolve the requested tag while other callers use event SHA", () => {
  assert.ok(job(readWorkflow("release-tag.yml"), "security").includes(
    "ref: ${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.sha }}",
  ));
  for (const filename of ["build-windows.yml", "docker.yml"]) {
    assert.ok(job(readWorkflow(filename), "security").includes("ref: ${{ github.sha }}"));
  }
});

function assertDockerPolicy(workflow) {
  assert.ok(workflow.includes("PUBLISH_IMAGE: ${{ (github.event_name == 'push' || github.event_name == 'workflow_dispatch') && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/V')) }}"));
  assert.equal((workflow.match(/if: env.PUBLISH_IMAGE == 'true'/g) ?? []).length, 2);
  assert.equal((workflow.match(/push: true/g) ?? []).length, 1);
  assert.ok(job(workflow, "security").includes("enforce_findings: ${{ github.event_name != 'pull_request' }}"));
  assert.ok(workflow.includes("NPM_AUDIT_MODE: ${{ github.event_name == 'pull_request' && 'advisory' || 'strict' }}"));
  assert.ok(workflow.includes("GATEWAY_AUDIT_MODE=${{ env.NPM_AUDIT_MODE }}"));
  assert.equal(job(workflow, "docker").match(/^    if: (.+)$/m)?.[1], dockerBuildGuard);
  for (const root of ["scripts", "apps/desktop"]) {
    assert.ok(workflow.includes(`run: node scripts/npm-audit.mjs "$NPM_AUDIT_MODE" ${root} `));
  }
  const publishing = workflow.split("      - name: Build and optionally publish Gateway image")[1];
  assert.match(publishing, /GATEWAY_AUDIT_MODE=strict/);
  assert.doesNotMatch(publishing, /GATEWAY_AUDIT_MODE=\$|advisory/);
}

test("Docker main publication remains explicit and never enabled for pull requests", () => {
  assertDockerPolicy(readWorkflow("docker.yml"));
});

test("Docker contract rejects missing audits, release bypasses and advisory publishing", () => {
  const workflow = readWorkflow("docker.yml");
  for (const changed of [
    workflow.replace('run: node scripts/npm-audit.mjs "$NPM_AUDIT_MODE" scripts ', "run: echo "),
    workflow.replace('run: node scripts/npm-audit.mjs "$NPM_AUDIT_MODE" apps/desktop ', "run: echo "),
    workflow.replace("GATEWAY_AUDIT_MODE=strict", "GATEWAY_AUDIT_MODE=advisory"),
    workflow.replace("needs.security.result == 'success' && ", ""),
    workflow.replace(" || needs.security.outputs.findings_free == 'true'", " || true"),
    workflow.replace("enforce_findings: ${{ github.event_name != 'pull_request' }}", "enforce_findings: false"),
    workflow.replace("if: env.PUBLISH_IMAGE == 'true'", "if: always()"),
  ]) assert.throws(() => assertDockerPolicy(changed));
});

test("Docker findings permit PR verification only; publishing events require a clean result", () => {
  const actual = job(readWorkflow("docker.yml"), "docker").match(/^    if: (.+)$/m)[1];
  assert.equal(actual, dockerBuildGuard);
  const evaluate = new Function("needs", "github", `return (${actual.slice(3, -2).replace(/==/g, "===")});`);
  for (const result of ["success", "failure", "cancelled", "skipped", ""]) {
    for (const event_name of ["pull_request", "pull_request_target", "push", "workflow_dispatch", "schedule"]) {
      for (const findings_free of ["true", "false", "", null, undefined]) {
        assert.equal(evaluate({ security: { result, outputs: { findings_free } } }, { event_name }),
          result === "success" && (event_name === "pull_request" || findings_free === "true"));
      }
    }
  }
});

test("the reusable policy job runs both coverage and publication contracts", () => {
  assert.ok(job(security, "policy").includes(
    "node --test scripts/tests/security-ci-policy.test.mjs scripts/tests/security-release-gates.test.mjs",
  ));
});
