import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const verifier = fileURLToPath(new URL("../../tools/package-release/verify-browser-runtime.mjs", import.meta.url));
const json = async (file, value) => { await mkdir(path.dirname(file), { recursive: true }); await writeFile(file, JSON.stringify(value)); };

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-worker-package-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const source = path.join(root, "source"), destination = path.join(root, "payload/scripts");
  await mkdir(destination, { recursive: true });
  const dependencies = { "playwright-core": "1.59.1", ws: "8.21.0", selfsigned: "5.5.0", "@aws-sdk/client-s3": "3.1033.0" };
  const lock = { lockfileVersion: 3, packages: { "": { dependencies } } };
  for (const [name, version] of Object.entries(dependencies)) {
    lock.packages[`node_modules/${name}`] = { version };
    await json(path.join(source, "node_modules", name, "package.json"), { name, version, main: "index.cjs" });
    await writeFile(path.join(source, "node_modules", name, "index.cjs"), name === "playwright-core"
      ? "exports.chromium = { launch() {} };\n" : "module.exports = {};\n");
  }
  await json(path.join(source, "package.json"), { type: "module", dependencies });
  await json(path.join(destination, "package.json"), { type: "module", dependencies });
  const worker = `import 'ws'; import 'selfsigned'; import '@aws-sdk/client-s3';
    if (process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN !== '1') throw Error('must suppress main');\n`;
  await writeFile(path.join(destination, "gemini-canvas-browser-pool.mjs"), worker);
  const save = () => json(path.join(source, "package-lock.json"), lock);
  const run = async () => {
    await save();
    return spawnSync(process.execPath, [verifier, source, destination], { encoding: "utf8", timeout: 30_000 });
  };
  return { root, source, destination, lock, run };
}

test("packages all locked production roots and imports the worker without the source tree", async (t) => {
  const f = await fixture(t);
  await writeFile(path.join(f.source, "node_modules/extraneous.txt"), "do not ship");
  const result = await f.run();
  assert.equal(result.status, 0, result.stderr);
  for (const name of ["ws", "selfsigned", "@aws-sdk/client-s3", "playwright-core"]) {
    const installed = JSON.parse(await readFile(path.join(f.destination, "node_modules", name, "package.json")));
    assert.equal(installed.name, name);
  }
  await assert.rejects(readFile(path.join(f.destination, "node_modules/extraneous.txt")), { code: "ENOENT" });
  await rm(f.source, { recursive: true });
  const imported = spawnSync(process.execPath, [path.join(f.destination, "gemini-canvas-browser-pool.mjs")], {
    encoding: "utf8", timeout: 30_000, cwd: f.root,
    env: { ...process.env, NODE_PATH: "", GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN: "1" },
  });
  assert.equal(imported.status, 0, imported.stderr);
});

test("copies nested production closure once and excludes dev and extraneous nested packages", async (t) => {
  const f = await fixture(t);
  f.lock.packages["node_modules/ws"].dependencies = { child: "^2" };
  f.lock.packages["node_modules/ws/node_modules/child"] = { version: "2.1.0", dependencies: { ws: "*" } };
  f.lock.packages["node_modules/dev-only"] = { version: "1.0.0", dev: true };
  await json(path.join(f.source, "node_modules/ws/node_modules/child/package.json"), { name: "child", version: "2.1.0" });
  await json(path.join(f.source, "node_modules/ws/node_modules/unlocked/package.json"), { name: "unlocked", version: "1" });
  const result = await f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(await readFile(path.join(f.destination, "node_modules/ws/node_modules/child/package.json"))).version, "2.1.0");
  await assert.rejects(readFile(path.join(f.destination, "node_modules/ws/node_modules/unlocked/package.json")), { code: "ENOENT" });
});

for (const scenario of ["missing", "version", "dev", "unlocked", "traversal", "unresolved-worker-import"]) {
  test(`rejects unsafe or incomplete production closure: ${scenario}`, async (t) => {
    const f = await fixture(t);
    if (scenario === "missing") await rm(path.join(f.source, "node_modules/ws"), { recursive: true });
    if (scenario === "version") await json(path.join(f.source, "node_modules/ws/package.json"), { name: "ws", version: "0.0.0" });
    if (scenario === "dev") f.lock.packages["node_modules/ws"].dev = true;
    if (scenario === "unlocked") f.lock.packages["node_modules/ws"].dependencies = { missing: "^1" };
    if (scenario === "traversal") f.lock.packages["node_modules/ws"].dependencies = { "../outside": "^1" };
    if (scenario === "unresolved-worker-import") await writeFile(path.join(f.destination, "gemini-canvas-browser-pool.mjs"), "import 'not-shipped-runtime';\n");
    const result = await f.run();
    assert.notEqual(result.status, 0, result.stdout);
  });
}

test("rejects junctions in scoped package ancestry", async (t) => {
  const f = await fixture(t);
  const outside = path.join(f.root, "outside");
  await mkdir(outside);
  await rm(path.join(f.source, "node_modules/@aws-sdk"), { recursive: true });
  await symlink(outside, path.join(f.source, "node_modules/@aws-sdk"), "junction");
  const result = await f.run();
  assert.notEqual(result.status, 0, result.stdout);
});

test("optional absent packages and peers stay optional; installed peers join the closure", async (t) => {
  const f = await fixture(t);
  f.lock.packages["node_modules/ws"].optionalDependencies = { absent: "^1" };
  f.lock.packages["node_modules/absent"] = { version: "1.0.0", optional: true };
  f.lock.packages["node_modules/ws"].peerDependencies = { peer: "^1", "optional-peer": "^1" };
  f.lock.packages["node_modules/ws"].peerDependenciesMeta = { "optional-peer": { optional: true } };
  f.lock.packages["node_modules/peer"] = { version: "1.0.0" };
  await json(path.join(f.source, "node_modules/peer/package.json"), { name: "peer", version: "1.0.0" });
  const result = await f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(await readFile(path.join(f.destination, "node_modules/peer/package.json"))).name, "peer");
});

test("a missing required peer fails rather than resolving from a development ancestor", async (t) => {
  const f = await fixture(t);
  f.lock.packages["node_modules/ws"].peerDependencies = { missing: "^1" };
  assert.notEqual((await f.run()).status, 0);
});

test("manifest drift and nonexact production versions fail closed", async (t) => {
  const f = await fixture(t);
  const manifest = JSON.parse(await readFile(path.join(f.source, "package.json")));
  manifest.dependencies.ws = "^8.21.0";
  f.lock.packages[""].dependencies.ws = "^8.21.0";
  await json(path.join(f.source, "package.json"), manifest);
  assert.notEqual((await f.run()).status, 0);
});

test("a manifest and lock root disagreement blocks packaging", async (t) => {
  const f = await fixture(t);
  const manifest = JSON.parse(await readFile(path.join(f.source, "package.json")));
  delete manifest.dependencies.selfsigned;
  await json(path.join(f.source, "package.json"), manifest);
  assert.notEqual((await f.run()).status, 0);
});

test("linked destination ancestry cannot receive runtime files", async (t) => {
  const f = await fixture(t);
  const outside = path.join(f.root, "outside");
  await mkdir(outside);
  await symlink(outside, path.join(f.destination, "node_modules"), "junction");
  assert.notEqual((await f.run()).status, 0);
  await assert.rejects(readFile(path.join(outside, "playwright-core/package.json")), { code: "ENOENT" });
});

test("symlinked installed runtime files fail instead of leaking adjacent content", async (t) => {
  const f = await fixture(t);
  const outside = path.join(f.root, "outside");
  await mkdir(outside);
  await symlink(outside, path.join(f.source, "node_modules/ws/linked-directory"), "junction");
  assert.notEqual((await f.run()).status, 0);
});

test("reduced fixtures without runtime dependencies remain supported", async (t) => {
  const f = await fixture(t);
  await json(path.join(f.source, "package.json"), { name: "reduced-fixture" });
  const result = await f.run();
  assert.equal(result.status, 0, result.stderr);
});
