import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));
const read = (name) => readFileSync(new URL(`../../${name}`, import.meta.url), "utf8").replace(/\r\n/g, "\n");
const security = read(".github/workflows/security.yml");
const osv = read(".github/workflows/osv-scan.yml");
const runner = read("scripts/security-scan.mjs");
const codeql = read(".github/workflows/codeql.yml");
const dependabot = read(".github/dependabot.yml");

test("OSV covers every committed first-party lockfile with validated reports", () => {
  const tracked = execFileSync("git", ["ls-files"], { cwd: root, encoding: "utf8" }).split("\n")
    .filter((name) => /(^|\/)(Cargo\.lock|package-lock\.json)$/.test(name) && !name.includes("node_modules/"));
  const covered = [...runner.matchAll(/"([^"\n]*(?:Cargo\.lock|package-lock\.json))"/g)].map((match) => match[1]);
  assert.deepEqual(covered.sort(), tracked.sort());
  assert.equal(covered.length, 5);
  assert.ok(security.includes("uses: ./.github/workflows/osv-scan.yml"));
  assert.match(osv, /if: inputs.enforce_findings && steps.scan.outputs.findings_free != 'true'/);
  assert.match(osv, /wait-for-processing: true/);
  assert.doesNotMatch(osv, /continue-on-error|if: always|if:.*cancelled/);
  assert.doesNotMatch(security, /continue-on-error: true|allow-no-lockfiles|--offline/);
});

test("Cargo locks do not restore withdrawn spin 0.9 patch releases", () => {
  const locks = ["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock", "crates/gateway-local-data/Cargo.lock"];
  for (const lock of locks) {
    for (const entry of read(lock).split(/^\[\[package\]\]$/m)) {
      if (!/^name = "spin"$/m.test(entry)) continue;
      const version = entry.match(/^version = "([^"]+)"$/m)?.[1];
      assert.ok(version, `Missing spin version in ${lock}`);
      // 撤回状态不等同于漏洞公告；单独防止回退，不替代 OSV 的完整扫描。
      assert.doesNotMatch(version, /^0\.9\.[0-8]($|[-+])/, `${lock}: use spin 0.9.9 or a reviewed newer release`);
    }
  }
});

test("browser TLS uses the async selfsigned API without node-forge in its lock graph", () => {
  const manifest = JSON.parse(read("scripts/package.json"));
  const lock = JSON.parse(read("scripts/package-lock.json"));
  assert.match(manifest.dependencies.selfsigned, /^5\./);
  assert.equal(lock.packages["node_modules/selfsigned"].version, manifest.dependencies.selfsigned);
  assert.equal(lock.packages[""].dependencies.selfsigned, manifest.dependencies.selfsigned);
  assert.equal(Object.keys(lock.packages).some((name) => /(^|\/)node_modules\/node-forge$/.test(name)), false);
});

test("security actions use immutable pins and every normal trigger", () => {
  for (const source of [security, codeql]) {
    const references = [...source.matchAll(/uses: ([^\s#]+)/g)].map((match) => match[1]);
    assert.ok(references.length > 0);
    for (const reference of references) {
      if (reference === "./.github/workflows/osv-scan.yml") continue;
      assert.match(reference, /@[a-f0-9]{40}$/);
    }
    for (const event of ["pull_request:", "push:", "schedule:", "workflow_dispatch:"]) {
      assert.ok(source.includes(event), `Missing ${event}`);
    }
  }
});

test("CodeQL analyzes frontend, Rust and workflow source without building releases", () => {
  for (const language of ["javascript-typescript", "rust", "actions"]) {
    assert.match(codeql, new RegExp(`language: ${language}\\n\\s+build-mode: none`));
  }
  assert.match(codeql, /queries: security-extended/);
});

test("report writing is job-scoped and local jobs have bounded runtimes", () => {
  for (const source of [security, codeql]) {
    assert.match(source, /^permissions:\n  contents: read\n\njobs:/m);
    assert.doesNotMatch(source, /^  (actions|security-events|packages):/m);
  }
  const analyze = codeql.split("  analyze:\n")[1];
  assert.match(analyze, /timeout-minutes: 30/);
  assert.match(analyze, /permissions:\n      actions: read\n      contents: read\n      security-events: write/);
  const policy = security.split("  policy:\n")[1].split("  dependencies:\n")[0];
  assert.match(policy, /timeout-minutes: 10/);
  assert.doesNotMatch(policy, /: write/);
  const dependencies = security.split("  dependencies:\n")[1].split("  secrets:\n")[0];
  assert.match(dependencies, /permissions:\n      actions: read\n      contents: read\n      security-events: write/);
});

test("secret scanning remains redacted and read-only", () => {
  const secrets = security.split("  secrets:\n")[1];
  assert.ok(secrets);
  assert.match(secrets, /pull-requests: read/);
  assert.doesNotMatch(secrets, /: write/);
  assert.match(secrets, /fetch-depth: 0/);
  assert.match(secrets, /gitleaks_8\.24\.3_linux_x64\.tar\.gz/);
  assert.match(secrets, /sha256sum --check --strict/);
  assert.match(runner, /"--redact=100"/);
  assert.doesNotMatch(secrets, /GITHUB_TOKEN|ENABLE_COMMENTS/);
});

test("Dependabot covers all build roots with bounded update batches", () => {
  for (const ecosystem of ["cargo", "npm", "github-actions"]) {
    assert.ok(dependabot.includes(`package-ecosystem: ${ecosystem}`));
  }
  for (const directory of ["/", "/apps/desktop/src-tauri", "/crates/gateway-local-data", "/crates/gateway-sqlx", "/apps/desktop", "/scripts"]) {
    assert.ok(dependabot.includes(`"${directory}"`), `Missing ${directory}`);
  }
  assert.equal((dependabot.match(/open-pull-requests-limit: [1-4]\b/g) ?? []).length, 3);
  assert.equal((dependabot.match(/interval: weekly/g) ?? []).length, 3);
  assert.match(security, /node --test scripts\/tests\/security-ci-policy\.test\.mjs/);
});
