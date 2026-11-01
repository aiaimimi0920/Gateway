/** Opt-in native runtime payload: verify source bindings, license inventory and bytes before copying. */
import assert from "node:assert/strict";
import { constants } from "node:fs";
import { readFile, readdir, lstat, mkdir, copyFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { pathToFileURL } from "node:url";
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const revision = "9b694a6b7c6439db1767a577994dfcc87a3cf588";

async function regularFile(root, relative) {
  let cursor = root;
  assert.ok(!(await lstat(cursor)).isSymbolicLink(), "Runtime root cannot be a link");
  const pieces = relative.split("/");
  for (const [index, piece] of pieces.entries()) {
    cursor = path.join(cursor, piece);
    const stat = await lstat(cursor);
    assert.ok(!stat.isSymbolicLink() && (index === pieces.length - 1 ? stat.isFile() : stat.isDirectory()), "Runtime links/nonfiles rejected");
  }
  return cursor;
}

export async function copyAistudioRuntime(scriptsRoot, packageScripts, source, buildKind = "devtest") {
  if (source == null || source === "") return { included: false };
  assert.ok(path.isAbsolute(source), "Runtime source must be absolute");
  const manifestPath = await regularFile(source, "runtime-manifest.json");
  const encoded = await readFile(manifestPath);
  assert.ok(encoded.length <= 65536, "Runtime manifest too large");
  const manifest = JSON.parse(encoded.toString("utf8"));
  assert.ok(manifest.schemaVersion === 1 && manifest.sourceRevision === revision && manifest.buildKind === buildKind, "Runtime provenance mismatch");
  const helperRoot = path.resolve(scriptsRoot, "../tools/aistudio-proof");
  const expected = (await readdir(helperRoot)).filter(name => name.endsWith(".go") || ["go.mod", "go.sum"].includes(name)).sort();
  assert.ok(Array.isArray(manifest.helperSourceFiles), "Helper source inventory missing");
  assert.deepEqual(manifest.helperSourceFiles.map(item => item.name).sort(), expected, "Helper source inventory mismatch");
  for (const item of manifest.helperSourceFiles) {
    assert.equal(hash(await readFile(path.join(helperRoot, item.name))), item.sha256.toLowerCase(), "Helper source changed; rebuild required");
  }
  assert.ok(Array.isArray(manifest.files) && manifest.files.length <= 32, "Runtime inventory invalid");
  const selected = [], paths = new Set(); let total = 0;
  for (const item of manifest.files) {
    assert.ok(typeof item.path === "string" && /^(?:aistudio-proof\.exe|licenses\/[A-Za-z0-9._-]+\.(?:txt|html|json))$/.test(item.path), "Unexpected runtime payload path");
    assert.ok(!paths.has(item.path), "Duplicate runtime payload"); paths.add(item.path);
    assert.ok(/^[a-f0-9]{64}$/i.test(item.sha256) && Number.isSafeInteger(item.bytes) && item.bytes > 0, "Runtime hash/size missing");
    total += item.bytes; assert.ok(total <= 128 * 1024 * 1024, "Runtime payload too large");
    const file = await regularFile(source, item.path), stat = await lstat(file);
    assert.equal(stat.size, item.bytes, "Runtime size mismatch");
    assert.equal(hash(await readFile(file)), item.sha256.toLowerCase(), "Runtime checksum mismatch");
    selected.push({ ...item, source: file });
  }
  for (const required of ["aistudio-proof.exe", "licenses/NOTICE.txt", "licenses/AIStudio2API-LICENSE.txt", "licenses/MPL-2.0.txt"]) {
    assert.ok(paths.has(required), `Missing required runtime payload: ${required}`);
  }
  const destination = path.join(packageScripts, "aistudio-http/runtime");
  await mkdir(path.dirname(destination), { recursive: true }); await mkdir(destination);
  for (const item of [...selected, { path: "runtime-manifest.json", source: manifestPath, sha256: hash(encoded) }]) {
    const file = path.join(destination, item.path); await mkdir(path.dirname(file), { recursive: true });
    await copyFile(item.source, file, constants.COPYFILE_EXCL);
    assert.equal(hash(await readFile(file)), item.sha256.toLowerCase(), "Copied runtime checksum mismatch");
  }
  return { included: true, files: selected.length + 1, sourceRevision: revision };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const result = await copyAistudioRuntime(process.argv[2], process.argv[3], process.env.GATEWAY_AISTUDIO_RUNTIME_DIR, process.argv[4]);
  process.stdout.write(`[gateway-package] AI Studio native runtime ${JSON.stringify(result)}\n`);
}
