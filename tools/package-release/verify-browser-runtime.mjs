import assert from "node:assert/strict";
import { constants, realpathSync } from "node:fs";
import { copyFile, lstat, mkdir, readFile, readdir } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

const json = async (file) => JSON.parse(await readFile(file, "utf8"));

async function copyOwned(source, destination) {
  const info = await lstat(source);
  assert.ok(!info.isSymbolicLink(), "browser runtime contains a linked path");
  if (info.isDirectory()) {
    await mkdir(destination, { recursive: true });
    for (const name of await readdir(source)) await copyOwned(path.join(source, name), path.join(destination, name));
  } else {
    assert.ok(info.isFile(), "browser runtime contains a non-regular file");
    await copyFile(source, destination, constants.COPYFILE_EXCL);
  }
}

async function packageBrowserRuntime(source, destination) {
  const manifest = await json(path.join(source, "package.json"));
  const version = manifest.dependencies?.["playwright-core"];
  // Reduced fixtures without browser workers need no browser package.
  if (version === undefined) return;
  const lock = await json(path.join(source, "package-lock.json"));
  const entry = lock.packages?.["node_modules/playwright-core"];
  assert.equal(entry?.version, version, "browser runtime differs from the lockfile");
  assert.equal(lock.packages?.[""]?.dependencies?.["playwright-core"], version);
  assert.ok(!entry.link && !entry.dev, "browser runtime must be a production package");
  for (const name of ["dependencies", "optionalDependencies", "peerDependencies"])
    assert.equal(Object.keys(entry[name] || {}).length, 0, "update the browser dependency closure before shipping");
  const modules = path.join(source, "node_modules");
  assert.ok(!(await lstat(modules)).isSymbolicLink(), "node_modules must not be linked");
  const packagePath = path.join(modules, "playwright-core");
  assert.ok(!(await lstat(packagePath)).isSymbolicLink(), "browser runtime must not be linked");
  assert.ok(!(await lstat(path.join(packagePath, "package.json"))).isSymbolicLink());
  const installed = await json(path.join(packagePath, "package.json"));
  assert.equal(installed.name, "playwright-core");
  assert.equal(installed.version, version, "installed browser runtime differs from the lockfile");
  await copyOwned(packagePath, path.join(destination, "node_modules", "playwright-core"));
  const scripts = realpathSync(destination);
  const owned = realpathSync(path.join(scripts, "node_modules", "playwright-core"));
  const require = createRequire(path.join(scripts, "package.json"));
  const resolved = realpathSync(require.resolve("playwright-core"));
  assert.ok(resolved.startsWith(owned + path.sep), "browser runtime escaped the package");
  assert.equal(typeof require("playwright-core").chromium.launch, "function");
  process.stdout.write("[gateway-package] locked ChatGPT browser runtime import passed\n");
}

await packageBrowserRuntime(path.resolve(process.argv[2]), path.resolve(process.argv[3]));
