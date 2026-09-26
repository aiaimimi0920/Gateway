// Classify exact installed bytes against a hash-bound, reviewed provenance receipt.
// Signature authentication belongs to the recorded audit; this gate checks its binding.
import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const WEBSTORE_KEY_SHA256 = "b5e4096227b83f82101fc2aa49f2a251accde3d98471464dd17d1819b43786b0";
const ENGINE_ID = "bjbcblmdcnggnibecjikpoljcgkbgphl";
const MAX_DOCUMENT_BYTES = 2 * 1024 * 1024;
const digest = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

function canonicalPath(value) {
  assert.equal(typeof value, "string", "artifact path must be a string");
  const parts = value.split("/");
  assert.ok(parts.every((part) => /^[A-Za-z0-9_.-]+$/.test(part) && part !== "." && part !== ".."),
    `artifact path must be canonical and repository-relative: ${value}`);
  return parts;
}

function objectKeys(value, expected, label) {
  assert.ok(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  assert.deepEqual(Object.keys(value).sort(), [...expected].sort(), `${label} fields are invalid`);
}

function readBoundDocument(root, reference) {
  objectKeys(reference, ["path", "sha256"], "artifact evidence reference");
  assert.match(reference.sha256, /^[0-9a-f]{64}$/, "artifact evidence hash is invalid");
  let absolute = fs.realpathSync(root);
  for (const part of canonicalPath(reference.path)) {
    absolute = path.join(absolute, part);
    assert.ok(!fs.lstatSync(absolute).isSymbolicLink(), "artifact evidence cannot follow a symbolic link or junction");
  }
  const fd = fs.openSync(absolute, "r");
  let bytes;
  try {
    const stat = fs.fstatSync(fd);
    assert.ok(stat.isFile() && stat.size <= MAX_DOCUMENT_BYTES, "artifact evidence document is too large or not a file");
    const buffer = Buffer.alloc(stat.size + 1);
    let length = 0;
    while (length < buffer.length) {
      const read = fs.readSync(fd, buffer, length, buffer.length - length, length);
      if (read === 0) break;
      length += read;
    }
    assert.equal(length, stat.size, "artifact evidence changed while reading");
    bytes = buffer.subarray(0, length);
  } finally {
    fs.closeSync(fd);
  }
  assert.equal(digest(bytes), reference.sha256, "artifact evidence hash mismatch");
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
}

function validatePackageIdentity(packagePath, itemId, version) {
  canonicalPath(packagePath);
  assert.match(itemId, /^[a-p]{32}$/, "artifact package item ID is invalid");
  assert.match(version, /^\d+(?:\.\d+)*$/, "artifact package version is invalid");
  const match = packagePath.match(/^deploy\/gateway_data\/browser-profiles\/(suno|udio)\/(.+)$/);
  assert.ok(match, "artifact package must be inside an explicit browser runtime profile");
  const suffix = match[2];
  if (suffix.startsWith("WasmTtsEngine/")) {
    assert.equal(itemId, ENGINE_ID, "artifact engine item ID mismatch");
    assert.equal(suffix, `WasmTtsEngine/${version}`, "artifact engine version directory mismatch");
  } else {
    assert.ok(new RegExp(`^Default/Extensions/${itemId}/${version.replaceAll(".", "\\.")}_\\d+$`).test(suffix),
      "artifact extension ID/version directory mismatch");
  }
}

function receiptIndex(receipt) {
  assert.equal(receipt.schemaVersion, 1, "artifact receipt schemaVersion must be 1");
  assert.equal(receipt.evidenceType, "chromium-webstore-rs256-treehash", "artifact receipt type mismatch");
  assert.equal(receipt.publicKeySha256, WEBSTORE_KEY_SHA256, "artifact receipt trust anchor mismatch");
  assert.match(receipt.chromiumCommit, /^[0-9a-f]{40}$/, "artifact receipt source revision is invalid");
  assert.ok(Array.isArray(receipt.packages) && receipt.packages.length > 0 && receipt.packages.length <= 64,
    "artifact receipt packages are invalid");
  const index = new Map();
  const packagePaths = new Set();
  for (const pkg of receipt.packages) {
    validatePackageIdentity(pkg.path, pkg.itemId, pkg.version);
    assert.ok(!packagePaths.has(pkg.path.toLowerCase()), "duplicate artifact receipt package");
    packagePaths.add(pkg.path.toLowerCase());
    assert.equal(pkg.webstoreSignatureVerified, true, "artifact receipt signature was not verified");
    assert.equal(pkg.signatureMutationRejected, true, "artifact receipt lacks a signature negative control");
    assert.match(pkg.verifiedContentsSha256, /^[0-9a-f]{64}$/, "artifact receipt metadata hash is invalid");
    assert.ok(Array.isArray(pkg.files) && pkg.files.length <= 512, "artifact receipt files are invalid");
    for (const file of pkg.files) {
      canonicalPath(file.path);
      const filePath = `${pkg.path}/${file.path}`;
      assert.ok(!index.has(filePath.toLowerCase()), "duplicate artifact receipt file");
      index.set(filePath.toLowerCase(), { pkg, file, path: filePath });
    }
  }
  return index;
}

export function loadRuntimeArtifacts(root, reference) {
  if (reference === undefined) return new Map();
  const registry = readBoundDocument(root, reference);
  objectKeys(registry, ["schemaVersion", "evidence", "artifacts"], "artifact registry");
  assert.equal(registry.schemaVersion, 1, "artifact registry schemaVersion must be 1");
  const receipt = receiptIndex(readBoundDocument(root, registry.evidence));
  assert.ok(Array.isArray(registry.artifacts) && registry.artifacts.length > 0 && registry.artifacts.length <= 256,
    "artifact registry must contain 1-256 exact files");
  const result = new Map();
  const seen = new Set();
  for (const entry of registry.artifacts) {
    objectKeys(entry, ["path", "packagePath", "itemId", "version", "bytes", "sha256", "reason"], "artifact entry");
    canonicalPath(entry.path);
    validatePackageIdentity(entry.packagePath, entry.itemId, entry.version);
    assert.ok(entry.path.startsWith(`${entry.packagePath}/`) && entry.path.endsWith(".js"),
      "artifact entry must name one JavaScript file in its exact package");
    assert.ok(!seen.has(entry.path.toLowerCase()), "duplicate artifact path");
    seen.add(entry.path.toLowerCase());
    assert.ok(Number.isSafeInteger(entry.bytes) && entry.bytes > 0 && entry.bytes <= 32 * 1024 * 1024,
      "artifact byte count is invalid");
    assert.match(entry.sha256, /^[0-9a-f]{64}$/, "artifact source hash is invalid");
    assert.ok(typeof entry.reason === "string" && entry.reason.trim(), "artifact reason is required");
    const signed = receipt.get(entry.path.toLowerCase());
    assert.ok(signed && signed.path === entry.path, "artifact is absent from the reviewed receipt");
    assert.equal(signed.pkg.path, entry.packagePath, "artifact receipt package path mismatch");
    assert.equal(signed.pkg.itemId, entry.itemId, "artifact receipt item ID mismatch");
    assert.equal(signed.pkg.version, entry.version, "artifact receipt version mismatch");
    assert.equal(signed.file.role, "strict-inventory-asset", "artifact receipt is not a measured source asset");
    assert.equal(signed.file.byteMutationRejected, true, "artifact receipt lacks a byte negative control");
    assert.match(signed.file.signedTreeHash, /^[A-Za-z0-9_-]{43}$/, "artifact receipt tree hash is invalid");
    assert.equal(signed.file.sha256, entry.sha256, "artifact receipt source hash mismatch");
    assert.equal(signed.file.bytes, entry.bytes, "artifact receipt byte count mismatch");
    result.set(entry.path, { ...entry, evidencePath: registry.evidence.path, evidenceSha256: registry.evidence.sha256 });
  }
  return result;
}

export function matchesRuntimeArtifact(bytes, entry) {
  return bytes.length === entry.bytes && digest(bytes) === entry.sha256;
}
