import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { evaluateRows, scanRepository, summarizeRows } from "../effective-code-lines.mjs";
import { loadRuntimeArtifacts } from "../effective-code-lines-runtime-artifacts.mjs";
import { makeArtifactFixture } from "./effective-code-lines-runtime-artifacts.fixtures.mjs";

test("classifies only exact approved bytes while retaining the measured inventory", (context) => {
  const fixture = makeArtifactFixture(context);
  const scan = scanRepository(fixture.root, fixture.policy);
  assert.deepEqual(scan.diagnostics, []);
  assert.equal(scan.rows[0].runtimeArtifact.rawSha256, fixture.registry.artifacts[0].sha256);
  assert.equal(scan.rows[0].effectiveLines, 800);
  assert.equal(summarizeRows(scan.rows).mandatory, 1);
  assert.deepEqual(evaluateRows(scan.rows, { files: [] }, new Map(), "strict"), { violations: [], warnings: [] });
});

test("without an approved registry the original strict rules still apply", (context) => {
  const fixture = makeArtifactFixture(context);
  delete fixture.policy.immutableRuntimeArtifacts;
  const scan = scanRepository(fixture.root, fixture.policy);
  assert.equal(scan.rows[0].runtimeArtifact, undefined);
  assert.equal(evaluateRows(scan.rows, { files: [] }, new Map(), "strict").violations.length, 1);
});

test("byte and line-ending drift remain visible even with identical effective lines", (context) => {
  const fixture = makeArtifactFixture(context);
  const original = scanRepository(fixture.root, fixture.policy).rows[0];
  for (const content of [fixture.content.replace("= 0;", "= 9;"), fixture.content.replaceAll("\n", "\r\n")]) {
    fixture.write(fixture.artifactPath, content);
    const scan = scanRepository(fixture.root, fixture.policy);
    assert.match(scan.diagnostics[0], /byte drift/);
    assert.equal(scan.rows[0].runtimeArtifact, undefined);
    assert.equal(scan.rows[0].effectiveLines, original.effectiveLines);
    assert.equal(evaluateRows(scan.rows, { files: [] }, new Map(), "strict").violations.length, 1);
  }
});

test("identical siblings, new versions and first-party files receive no classification", (context) => {
  const fixture = makeArtifactFixture(context);
  const paths = [`${fixture.packagePath}/sibling.js`, fixture.artifactPath.replace("1.2.3_0", "1.2.4_0"), "src/runtime.js"];
  for (const file of paths) fixture.write(file, fixture.content);
  const scan = scanRepository(fixture.root, fixture.policy);
  assert.deepEqual(scan.diagnostics, []);
  assert.equal(scan.rows.filter((row) => row.runtimeArtifact).length, 1);
  assert.equal(evaluateRows(scan.rows, { files: [] }, new Map(), "strict").violations.length, 3);
});

test("optional local runtime payloads may be absent from a clean checkout", (context) => {
  const fixture = makeArtifactFixture(context);
  fs.unlinkSync(path.join(fixture.root, fixture.artifactPath));
  assert.deepEqual(scanRepository(fixture.root, fixture.policy), { rows: [], diagnostics: [] });
});

test("missing registry or provenance evidence fails closed", (context) => {
  const fixture = makeArtifactFixture(context);
  for (const file of ["docs/registry.json", "docs/receipt.json"]) {
    fs.unlinkSync(path.join(fixture.root, file));
    assert.throws(() => scanRepository(fixture.root, fixture.policy), /ENOENT/);
    fixture.persist();
  }
});

test("unreviewed changes to either hash-bound document fail closed", (context) => {
  const fixture = makeArtifactFixture(context);
  for (const file of ["docs/registry.json", "docs/receipt.json"]) {
    fs.appendFileSync(path.join(fixture.root, file), " ");
    assert.throws(() => scanRepository(fixture.root, fixture.policy), /evidence hash mismatch/);
    fixture.persist();
  }
});

test("registry rejects escape paths, first-party targets and unknown entry fields", (context) => {
  const fixture = makeArtifactFixture(context);
  const entry = fixture.registry.artifacts[0];
  const valid = structuredClone(entry);
  for (const invalid of ["../outside.js", "/outside.js", "C:/outside.js", "C:outside.js", "src/runtime.js",
    "deploy\\runtime.js", `${fixture.packagePath}/../runtime.js`, `${fixture.packagePath}/%2e%2e/runtime.js`]) {
    entry.path = invalid;
    fixture.persist();
    assert.throws(() => scanRepository(fixture.root, fixture.policy), /artifact path|one JavaScript file/);
  }
  Object.assign(entry, valid, { blanketExclusion: true });
  fixture.persist();
  assert.throws(() => scanRepository(fixture.root, fixture.policy), /entry fields/);
});

test("registry rejects duplicate and invalid records", (context) => {
  const fixture = makeArtifactFixture(context);
  const valid = structuredClone(fixture.registry.artifacts[0]);
  fixture.registry.artifacts.push({ ...valid });
  fixture.persist();
  assert.throws(() => scanRepository(fixture.root, fixture.policy), /duplicate artifact path/);
  for (const change of [{ sha256: "bad" }, { bytes: -1 }, { reason: " " }, { version: "1.2.4" }, { itemId: "b".repeat(32) }]) {
    fixture.registry.artifacts = [{ ...valid, ...change }];
    fixture.persist();
    assert.throws(() => scanRepository(fixture.root, fixture.policy), /artifact/);
  }
});

test("receipt must bind the exact registered file and package identity", (context) => {
  const fixture = makeArtifactFixture(context);
  const validReceipt = structuredClone(fixture.receipt);
  const mutations = [
    (receipt) => { receipt.packages[0].files[0].sha256 = "c".repeat(64); },
    (receipt) => { receipt.packages[0].files[0].path = "sibling.js"; },
    (receipt) => { receipt.packages[0].files[0].byteMutationRejected = false; },
    (receipt) => { receipt.packages[0].webstoreSignatureVerified = false; },
    (receipt) => { receipt.publicKeySha256 = "c".repeat(64); },
    (receipt) => { receipt.packages.push(structuredClone(receipt.packages[0])); },
  ];
  for (const mutate of mutations) {
    Object.assign(fixture.receipt, structuredClone(validReceipt));
    mutate(fixture.receipt);
    fixture.persist();
    assert.throws(() => scanRepository(fixture.root, fixture.policy), /artifact/);
  }
});

test("evidence references reject traversal and directory junctions", (context) => {
  const fixture = makeArtifactFixture(context);
  for (const invalid of ["../receipt.json", "/receipt.json", "docs/../receipt.json", "C:/receipt.json"]) {
    const reference = { ...fixture.policy.immutableRuntimeArtifacts, path: invalid };
    assert.throws(() => loadRuntimeArtifacts(fixture.root, reference), /repository-relative/);
  }
  const oldDocs = path.join(fixture.root, "docs");
  const movedDocs = path.join(fixture.root, "original-docs");
  fs.renameSync(oldDocs, movedDocs);
  fs.symlinkSync(movedDocs, oldDocs, process.platform === "win32" ? "junction" : "dir");
  assert.throws(() => scanRepository(fixture.root, fixture.policy), /symbolic link or junction/);
});

test("oversized and malformed evidence documents are rejected before classification", (context) => {
  const fixture = makeArtifactFixture(context);
  fixture.write("docs/registry.json", " ".repeat(2 * 1024 * 1024 + 1));
  assert.throws(() => scanRepository(fixture.root, fixture.policy), /too large/);
  fixture.persist();
  fixture.registry.schemaVersion = 999;
  fixture.persist();
  assert.throws(() => scanRepository(fixture.root, fixture.policy), /schemaVersion/);
});
