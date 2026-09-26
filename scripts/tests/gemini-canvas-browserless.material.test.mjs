import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();
async function withTree(run) {
  const parent = path.resolve(os.tmpdir());
  const root = await fs.mkdtemp(path.join(parent, "gateway-browserless-material-"));
  const write = async (name, text = "{}", age = 10) => {
    const file = path.join(root, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, text, "utf8");
    await fs.utimes(file, new Date(1700000000000 + age * 1000), new Date(1700000000000 + age * 1000));
    return file;
  };
  try { await run({ root, write }); } finally {
    assert.equal(path.dirname(path.resolve(root)), parent);
    assert.ok(path.basename(root).startsWith("gateway-browserless-material-"));
    await fs.rm(root, { recursive: true, force: true });
  }
}

test("browserless material absent roots and failed stats retain null/zero fallbacks", async () => withTree(async ({ root }) => {
  assert.equal(app.fileExists(path.join(root, "absent")), false);
  assert.equal(app.pathStatMtime(path.join(root, "absent")), 0);
  assert.equal(app.findLatestBrowserState(root), null);
  assert.deepEqual(app.walkFiles(path.join(root, "absent")), []);
  assert.equal(app.resolveStorageStatePath(root, null, {}), null);
  assert.deepEqual(await app.resolveProgramHandle({ __path: path.join(root, "browser.json") }, root), { path: null, json: null });
}));

test("browserless material newest exact browser-state name wins across nested runtime directories", async () => withTree(async ({ root, write }) => {
  const prefix = ".runtime/gemini-canvas-program-runtime/";
  await write(prefix + "old/browser-state.json", "{}", 1);
  const newest = await write(prefix + "deep/nested/browser-state.json", "{}", 5);
  await write(prefix + "newer/browser-state.json.bak", "{}", 9);
  await write("other/browser-state.json", "{}", 10);
  assert.equal(app.findLatestBrowserState(root), newest);
}));

test("browserless material explicit handle wins then sibling then newest immediate probe child", async () => withTree(async ({ root, write }) => {
  const state = { __path: path.join(root, "state/browser-state.json") };
  const explicit = await write("explicit.json", "{}", 1);
  const sibling = await write("state/program-handle.json", "{}", 2);
  await write(".runtime/gemini-canvas-program-handle-probe/a/program-handle.json", "{}", 3);
  const newest = await write(".runtime/gemini-canvas-program-handle-probe/b/program-handle.json", "{}", 4);
  await write(".runtime/gemini-canvas-program-handle-probe/c/deeper/program-handle.json", "{}", 9);
  assert.equal(app.resolveProgramHandlePath({ ...state, sourceProgramHandlePath: " " + explicit + " " }, root), explicit);
  assert.equal(app.resolveProgramHandlePath({ ...state, sourceProgramHandlePath: "missing" }, root), sibling);
  await fs.rm(sibling);
  assert.equal(app.resolveProgramHandlePath(state, root), newest);
}));

test("browserless material selected JSON strips BOM and surfaces malformed content", async () => withTree(async ({ root, write }) => {
  const handle = await write("program-handle.json", String.fromCharCode(0xfeff) + '{"marker":7}');
  const state = { __path: path.join(root, "browser-state.json") };
  assert.deepEqual(await app.resolveProgramHandle(state, root), { path: handle, json: { marker: 7 } });
  await fs.writeFile(handle, "{broken", "utf8");
  await assert.rejects(app.resolveProgramHandle(state, root), SyntaxError);
}));

test("browserless material probe readdir error remains observable", async () => withTree(async ({ root, write }) => {
  await write(".runtime/gemini-canvas-program-handle-probe");
  assert.throws(() => app.resolveProgramHandlePath({ __path: path.join(root, "browser.json") }, root));
}));

test("browserless material profile precedence checks existence without requiring a directory", async () => withTree(async ({ root, write }) => {
  const explicit = await write("explicit"), handle = await write("handle"), browser = await write("browser");
  const object = await write(".runtime/ai-gateway-objects/a/profile");
  const state = { runtimeProfileDir: browser, runtimeStateObjectKey: "a/profile" };
  const program = { json: { runtimeProfileDir: handle } };
  assert.equal(app.resolveProfileDir(state, program, root, { "profile-dir": explicit }), explicit);
  assert.equal(app.resolveProfileDir(state, program, root, { "profile-dir": "missing" }), handle);
  assert.equal(app.resolveProfileDir(state, {}, root, {}), browser);
  assert.equal(app.resolveProfileDir({ ...state, runtimeProfileDir: "missing" }, {}, root, {}), object);
  assert.equal(app.resolveProfileDir({}, {}, root, {}), null);
}));

test("browserless material walk preserves BFS order exact names and inclusive depth", async () => withTree(async ({ root, write }) => {
  const direct = await write("storage-state.json");
  const first = await write("Gemini-Canvas/storage-state.json");
  const second = await write("Gemini-Canvas/deep/storage-state.json");
  await write("Gemini-Canvas/deep/too-deep/storage-state.json");
  await write("Gemini-Canvas/storage-state.json.bak");
  assert.deepEqual(app.walkFiles(root, { maxDepth: 0, fileName: "storage-state.json" }), [direct]);
  assert.deepEqual(app.walkFiles(root, { maxDepth: 2, fileName: "storage-state.json", includePath: "GEMINI-CANVAS" }), [first, second]);
}));

test("browserless material newest storage across all sources outranks explicit CLI", async () => withTree(async ({ root, write }) => {
  const explicit = await write("explicit.json", "{}", 1);
  const profile = path.join(root, "profile/current");
  await write("profile/current/storage-state.json", "{}", 2);
  await write("profile/storage-state.json", "{}", 3);
  await write(".runtime/ai-gateway-objects/gemini-canvas/a/storage-state.json", "{}", 4);
  const newest = await write(".runtime/gemini-canvas-program-runtime/new/storage-state.json", "{}", 5);
  await write(".runtime/unrelated/storage-state.json", "{}", 20);
  assert.equal(app.resolveStorageStatePath(root, profile, { "storage-state": explicit }), newest);
  await fs.rm(newest);
  assert.match(app.resolveStorageStatePath(root, profile, { "storage-state": explicit }), /ai-gateway-objects/);
}));

test("browserless material equal storage mtimes retain candidate precedence and normalize explicit paths", async () => withTree(async ({ root, write }) => {
  const explicit = await write("explicit.json");
  const profile = path.join(root, "profile");
  const storage = await write("profile/storage-state.json");
  assert.equal(app.resolveStorageStatePath(root, profile, { "storage-state": path.relative(process.cwd(), explicit) }), explicit);
  assert.equal(app.resolveStorageStatePath(root, profile, { "storage-state": storage }), storage);
}));

for (const [kind, base, depth] of [["object", ".runtime/ai-gateway-objects", 7], ["runtime", ".runtime", 6]]) {
  test("browserless material " + kind + " storage search respects maximum depth", async () => withTree(async ({ root, write }) => {
    const inside = await write(base + "/gemini-canvas/" + "d/".repeat(depth - 1) + "storage-state.json", "{}", 1);
    await write(base + "/gemini-canvas/" + "d/".repeat(depth) + "storage-state.json", "{}", 9);
    assert.equal(app.resolveStorageStatePath(root, null, {}), inside);
  }));
}

test("browserless material shared dedupe still supports derived program handle", () => {
  assert.deepEqual(app.uniqueStrings([" a ", "a", null, 2, "", 2]), ["a", "2"]);
  assert.deepEqual(app.deriveProgramHandle("/app/0123456789abcdef /app/0123456789abcdef c_0123456789abcdef r_fedcba9876543210 Browser API Proxy Client", "https://fixture.test/"), {
    appPath: "/app/0123456789abcdef", programUrl: "https://fixture.test/app/0123456789abcdef",
    conversationId: "c_0123456789abcdef", responseId: "r_fedcba9876543210", sourceSurface: "canvas_proxy_client",
  });
});
