// Copy the production graph selected by the lock, never the installed tree wholesale.
import assert from "node:assert/strict";
import { constants } from "node:fs";
import { copyFile, lstat, mkdir, readFile, readdir, realpath } from "node:fs/promises";
import path from "node:path";

const json = async (file) => JSON.parse(await readFile(file, "utf8"));
const packageName = /^(?:@[a-z0-9._-]+\/)?[a-z0-9][a-z0-9._-]*$/i;

function assertName(name) {
  assert.ok(packageName.test(name), "invalid locked dependency name");
}

async function ownedPath(root, relative, allowMissing = false) {
  let current = root;
  for (const segment of relative.split("/")) {
    assert.ok(segment && segment !== "." && segment !== ".." && !segment.includes("\\"), "invalid package path");
    current = path.join(current, segment);
    let info;
    try { info = await lstat(current); } catch (error) {
      if (allowMissing && error.code === "ENOENT") return null;
      throw error;
    }
    assert.ok(!info.isSymbolicLink(), "worker runtime contains a linked path");
  }
  const resolved = await realpath(current);
  assert.ok(resolved.startsWith(root + path.sep), "worker runtime escaped its owner");
  return current;
}

function resolveLocked(packages, owner, name) {
  assertName(name);
  let directory = owner;
  for (;;) {
    if (path.posix.basename(directory) !== "node_modules") {
      const candidate = path.posix.join(directory, "node_modules", name);
      if (Object.hasOwn(packages, candidate)) return candidate;
    }
    if (!directory) return null;
    const parent = path.posix.dirname(directory);
    directory = parent === "." ? "" : parent;
  }
}

function edges(entry) {
  const optional = entry.optionalDependencies || {};
  const peers = entry.peerDependencies || {};
  const result = new Map(Object.keys(entry.dependencies || {}).map((name) => [name, false]));
  for (const name of Object.keys(optional)) result.set(name, true);
  for (const name of Object.keys(peers)) {
    if (!result.has(name)) result.set(name, entry.peerDependenciesMeta?.[name]?.optional === true);
  }
  return result;
}

async function productionGraph(source, packages, roots) {
  const selected = new Map();
  const queue = [...edges(roots)].map(([name, optional]) => ({ owner: "", name, optional }));
  for (let index = 0; index < queue.length; index += 1) {
    assert.ok(queue.length <= 50_000 && selected.size <= 4096, "worker dependency graph exceeds package limits");
    const { owner, name, optional } = queue[index];
    const key = resolveLocked(packages, owner, name);
    if (!key && optional) continue;
    assert.ok(key, `required worker dependency is absent from lock: ${name}`);
    const entry = packages[key];
    assert.ok(!entry.link && !entry.dev, "worker runtime must be a locked production package");
    const installedPath = await ownedPath(source, key, optional);
    if (!installedPath && optional) continue;
    if (selected.has(key)) continue;
    await ownedPath(source, `${key}/package.json`);
    const installed = await json(path.join(installedPath, "package.json"));
    assert.equal(installed.name, name, "installed worker package name differs from lock");
    assert.equal(installed.version, entry.version, "installed worker package version differs from lock");
    selected.set(key, installedPath);
    for (const [child, childOptional] of edges(entry)) queue.push({ owner: key, name: child, optional: childOptional });
  }
  return selected;
}

async function copyPackage(source, destination, budget, depth = 0) {
  assert.ok(depth < 128 && ++budget.entries <= 200_000, "worker runtime tree exceeds package limits");
  const info = await lstat(source);
  assert.ok(!info.isSymbolicLink(), "worker runtime contains a linked path");
  if (info.isDirectory()) {
    await mkdir(destination, { recursive: true });
    assert.ok(!(await lstat(destination)).isSymbolicLink(), "worker destination contains a linked path");
    for (const name of await readdir(source)) {
      // Nested dependencies are copied independently from the selected graph.
      if (depth === 0 && name === "node_modules") continue;
      await copyPackage(path.join(source, name), path.join(destination, name), budget, depth + 1);
    }
  } else {
    assert.ok(info.isFile(), "worker runtime contains a non-regular file");
    await copyFile(source, destination, constants.COPYFILE_EXCL);
  }
}

export async function packageLockedWorkerDependencies(source, destination) {
  source = await realpath(source);
  destination = await realpath(destination);
  await ownedPath(source, "package.json");
  const manifest = await json(path.join(source, "package.json"));
  if (edges(manifest).size === 0) return false; // Reduced fixtures have no workers.
  await ownedPath(source, "package-lock.json");
  const lock = await json(path.join(source, "package-lock.json"));
  assert.ok([2, 3].includes(lock.lockfileVersion) && lock.packages?.[""], "worker lockfile must include the package graph");
  for (const field of ["dependencies", "optionalDependencies", "peerDependencies", "peerDependenciesMeta"]) {
    assert.deepEqual(manifest[field] || {}, lock.packages[""][field] || {}, "worker manifest differs from lockfile");
  }
  for (const [name, version] of Object.entries(manifest.dependencies || {})) {
    assertName(name);
    assert.equal(lock.packages[`node_modules/${name}`]?.version, version, "worker production roots must be exact locked versions");
  }
  const graph = await productionGraph(source, lock.packages, manifest);
  const budget = { entries: 0 };
  for (const [key, installedPath] of graph) {
    await ownedPath(destination, path.posix.dirname(key), true);
    await mkdir(path.dirname(path.join(destination, key)), { recursive: true });
    await ownedPath(destination, key, true);
    // Check all existing ancestors, including scoped and nested node_modules.
    await ownedPath(destination, path.posix.dirname(key));
    await copyPackage(installedPath, path.join(destination, key), budget);
  }
  process.stdout.write(`[gateway-package] copied ${graph.size} locked production worker packages\n`);
  return true;
}
