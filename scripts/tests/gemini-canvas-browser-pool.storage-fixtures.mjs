import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const fields = [
  "DRIVER", "LOCAL_DIR", "BUCKET", "REGION", "ENDPOINT",
  "ACCESS_KEY_ID", "SECRET_ACCESS_KEY", "FORCE_PATH_STYLE",
];

export function storageFixture(t) {
  const base = fs.realpathSync(os.tmpdir());
  const root = fs.mkdtempSync(path.join(base, "gateway-browser-storage-"));
  const names = ["CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR", ...fields.flatMap((field) => [
    `AI_GATEWAY_OBJECT_STORAGE_${field}`, `OBJECT_STORAGE_${field}`,
  ])];
  const previous = new Map(names.map((name) => [name, process.env[name]]));
  for (const name of names) delete process.env[name];
  process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR = root;
  t.after(() => {
    for (const [name, value] of previous) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  });
  t.after(() => {
    const resolved = fs.realpathSync(root);
    assert.equal(path.dirname(resolved), base);
    assert.ok(path.basename(resolved).startsWith("gateway-browser-storage-"));
    assert.equal(fs.lstatSync(root).isSymbolicLink(), false);
    fs.rmSync(resolved, { recursive: true });
  });
  return root;
}
