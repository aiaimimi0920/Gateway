import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, mkdir, writeFile, readFile, rm, access } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { copyAistudioRuntime } from "../../tools/package-release/aistudio-runtime.mjs";
const hash = data => createHash("sha256").update(data).digest("hex");

async function fixture() {
  const root = await mkdtemp("C:/Users/Public/nas_home/AI/GameEditor/linshi/aistudio-package-test-");
  const source = path.join(root, "source"), scripts = path.join(root, "repo/scripts"), destination = path.join(root, "package/scripts");
  await mkdir(path.join(source, "licenses"), { recursive: true });
  await mkdir(path.join(root, "repo/tools/aistudio-proof"), { recursive: true });
  await mkdir(scripts); await mkdir(destination, { recursive: true });
  await writeFile(path.join(root, "repo/tools/aistudio-proof/main.go"), "fixture-source");
  const files = [];
  for (const name of ["aistudio-proof.exe", "licenses/NOTICE.txt", "licenses/AIStudio2API-LICENSE.txt", "licenses/MPL-2.0.txt"]) {
    const bytes = Buffer.from(`fixture-${name}`); await writeFile(path.join(source, name), bytes);
    files.push({ path: name, sha256: hash(bytes), bytes: bytes.length });
  }
  const manifest = { schemaVersion: 1, buildKind: "devtest", sourceRevision: "9b694a6b7c6439db1767a577994dfcc87a3cf588",
    helperSourceFiles: [{ name: "main.go", sha256: hash("fixture-source") }], files };
  const save = () => writeFile(path.join(source, "runtime-manifest.json"), JSON.stringify(manifest)); await save();
  return { root, source, scripts, destination, manifest, save };
}

test("package copies only hash-verified proof runtime with matching helper sources and licenses", async () => {
  const f = await fixture();
  try {
    assert.deepEqual(await copyAistudioRuntime(f.scripts, f.destination, undefined), { included: false });
    const result = await copyAistudioRuntime(f.scripts, f.destination, f.source);
    assert.equal(result.included, true);
    assert.equal((await readFile(path.join(f.destination, "aistudio-http/runtime/aistudio-proof.exe"))).toString(), "fixture-aistudio-proof.exe");
  } finally { await rm(f.root, { recursive: true }); }
});

test("runtime rejects drift, missing notices, traversal and a mismatched build kind before copying", async () => {
  for (const mode of ["hash", "sources", "notice", "traversal", "kind"]) {
    const f = await fixture();
    try {
      if (mode === "hash") f.manifest.files[0].sha256 = "0".repeat(64);
      if (mode === "sources") f.manifest.helperSourceFiles[0].sha256 = "0".repeat(64);
      if (mode === "notice") f.manifest.files.pop();
      if (mode === "traversal") f.manifest.files[0].path = "../secret.json";
      if (mode === "kind") f.manifest.buildKind = "release";
      await f.save();
      await assert.rejects(copyAistudioRuntime(f.scripts, f.destination, f.source));
      await assert.rejects(access(path.join(f.destination, "aistudio-http/runtime")));
    } finally { await rm(f.root, { recursive: true }); }
  }
});
