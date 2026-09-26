import { randomUUID } from "node:crypto";
import { lstat, mkdir, open, realpath, rename, rm } from "node:fs/promises";
import path from "node:path";
import { assertRuntimeStateSize, readRuntimeStateBody } from "./state-body.mjs";

function invalidKey() {
  return Object.assign(new Error("Invalid local runtime-state object key."), {
    code: "udio_invalid_local_object_key",
  });
}

function keySegments(key) {
  const segments = key.split("/");
  if (segments.some((segment) =>
    !segment || segment === "." || segment === ".." ||
    segment !== segment.trim() || /[\\:<>"|?*\x00-\x1f]/.test(segment) ||
    segment.endsWith(".") ||
    /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(segment))) {
    throw invalidKey();
  }
  return segments;
}

async function inspectEntry(file, directory) {
  let info;
  try {
    info = await lstat(file);
  } catch (error) {
    if (error.code === "ENOENT") return false;
    throw error;
  }
  if (info.isSymbolicLink()) {
    throw Object.assign(new Error("Runtime-state path contains a symbolic link or junction."), {
      code: "udio_local_object_link",
    });
  }
  if (directory ? !info.isDirectory() : !info.isFile()) {
    throw Object.assign(new Error("Runtime-state path has an unexpected file type."), {
      code: "udio_local_object_type",
    });
  }
  return true;
}

async function resolveLocalState(root, key, createParents) {
  const segments = keySegments(key);
  if (createParents) await mkdir(root, { recursive: true });
  // The configured root is trusted and may itself be a NAS junction.
  // Descendant links are refused; the store must not be writable by hostile local users.
  let parent = await realpath(root);
  for (const segment of segments.slice(0, -1)) {
    parent = path.join(parent, segment);
    if (createParents) {
      await mkdir(parent).catch((error) => {
        if (error.code !== "EEXIST") throw error;
      });
    }
    if (!await inspectEntry(parent, true)) {
      throw Object.assign(new Error("Runtime-state parent is absent."), { code: "ENOENT" });
    }
  }
  const destination = path.join(parent, segments.at(-1));
  await inspectEntry(destination, false);
  return destination;
}

export async function readLocalState(root, key) {
  const destination = await resolveLocalState(root, key, false);
  const handle = await open(destination, "r");
  try {
    assertRuntimeStateSize((await handle.stat()).size);
    return await readRuntimeStateBody(handle.createReadStream({ autoClose: false }));
  } finally {
    await handle.close();
  }
}

export async function writeLocalState(root, key, body) {
  assertRuntimeStateSize(body.byteLength);
  const destination = await resolveLocalState(root, key, true);
  const temporary = path.join(path.dirname(destination), "." + randomUUID() + ".state.tmp");
  let handle;
  let created = false;
  try {
    handle = await open(temporary, "wx", 0o600);
    created = true;
    await handle.writeFile(body);
    await handle.sync();
    await handle.close();
    handle = null;
    // Publish a complete replacement without modifying an existing hard-linked inode.
    await rename(temporary, destination);
  } finally {
    if (handle) await handle.close().catch(() => undefined);
    if (created) await rm(temporary, { force: true });
  }
}
