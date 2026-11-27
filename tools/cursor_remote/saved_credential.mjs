// Local operator diagnostic only: read one exact Gateway archive, never export or mutate secrets.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { tokenClaims } from "./auth.mjs";
import { emailHash } from "./qualification.mjs";
import { uniqueJson } from "./json_wire.mjs";
import { ProbeError } from "./transport.mjs";

const fail = (code) => { throw new ProbeError(code); };
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const samePath = (a, b) => process.platform === "win32" ? a.toLowerCase() === b.toLowerCase() : a === b;
const revisionPattern = /^r(?:0|[1-9]\d{0,19})-[a-f0-9]{12}$/;

function checkedPath(file, directory = false) {
  const root = path.parse(file).root;
  let current = root;
  for (const part of file.slice(root.length).split(path.sep).filter(Boolean)) {
    current = path.join(current, part);
    const info = fs.lstatSync(current);
    if (info.isSymbolicLink() || !samePath(fs.realpathSync(current), current)) fail("saved_path_redirected");
    if (current !== file && !info.isDirectory()) fail("saved_path_invalid");
  }
  const info = fs.lstatSync(file);
  if (directory ? !info.isDirectory() : !info.isFile()) fail("saved_path_invalid");
  return info;
}

function readRegular(file, limit = 512 * 1024) {
  const before = checkedPath(file);
  if (before.size > limit || before.nlink !== 1) fail("saved_file_invalid");
  const fd = fs.openSync(file, "r");
  const bytes = Buffer.alloc(limit + 1);
  try {
    const opened = fs.fstatSync(fd);
    if (opened.dev !== before.dev || opened.ino !== before.ino || !opened.isFile()) fail("saved_file_changed");
    let length = 0, count;
    do {
      count = fs.readSync(fd, bytes, length, bytes.length - length, null);
      length += count;
    } while (count > 0 && length < bytes.length);
    const after = fs.fstatSync(fd), named = checkedPath(file);
    if (length > limit || length !== before.size || after.size !== before.size ||
        after.mtimeMs !== before.mtimeMs || named.ino !== opened.ino || named.dev !== opened.dev ||
        named.mtimeMs !== before.mtimeMs) fail("saved_file_changed");
    return Buffer.from(bytes.subarray(0, length));
  } finally {
    bytes.fill(0);
    fs.closeSync(fd);
  }
}

function readJson(file, limit) {
  const bytes = readRegular(file, limit);
  try { return uniqueJson(bytes); }
  finally { bytes.fill(0); }
}

function guarded(action) {
  try { return action(); }
  catch (error) {
    // OS / JSON errors must not expose paths, contents or credentials.
    if (error instanceof ProbeError) throw error;
    throw new ProbeError("saved_credential_read_failed");
  }
}

export function openSavedCredential({ dataRoot, revision, providerId, credentialId, expectedEnabled = false }) {
  return guarded(() => {
    if (typeof dataRoot !== "string" || !path.isAbsolute(dataRoot) ||
        (process.platform === "win32" && !/^C:[\\/]/i.test(dataRoot)) ||
        !revisionPattern.test(revision ?? "") ||
        ![providerId, credentialId].every((s) => typeof s === "string" && /^[A-Za-z0-9_.:-]{1,256}$/.test(s))) {
      fail("invalid_saved_selection");
    }
    const root = path.resolve(dataRoot);
    checkedPath(root, true);
    const state = path.join(root, "local", "state", "console");
    const activeFile = path.join(state, "local-active.json");
    const archive = path.join(state, "revisions", revision);
    const documentFile = path.join(archive, "document.json");
    const checkActive = () => {
      if (readJson(activeFile, 256) !== revision) fail("saved_revision_changed");
    };
    checkActive();
    checkedPath(archive, true);
    if (fs.readdirSync(archive).sort().join(",") !== "document.json,metadata.json,routes.yaml") {
      fail("saved_archive_invalid");
    }
    const metadata = readJson(path.join(archive, "metadata.json"), 4096);
    // YAML expands repeated route keys more than JSON in a shared multi-provider archive.
    let bytes = readRegular(documentFile), yaml = readRegular(path.join(archive, "routes.yaml"), 2 * 1024 * 1024);
    let document;
    try {
      if (metadata?.id !== revision || !Number.isSafeInteger(metadata.sequence) ||
          !/^[a-f0-9]{64}$/.test(metadata.documentDigest ?? "") ||
          !/^[a-f0-9]{64}$/.test(metadata.yamlDigest ?? "") ||
          revision !== `r${metadata.sequence}-${metadata.documentDigest.slice(0, 12)}` ||
          digest(bytes) !== metadata.documentDigest || digest(yaml) !== metadata.yamlDigest) {
        fail("saved_archive_invalid");
      }
      // Rust serializes integral f64 route weights as 1.0; allow only exact .0 forms
      // for the hash-verified local archive, not upstream billing JSON.
      document = uniqueJson(bytes, { allowIntegerDecimals: true });
    } finally { bytes.fill(0); yaml.fill(0); bytes = yaml = undefined; }
    const providers = document?.providers?.filter((p) => p.id === providerId);
    const provider = providers?.length === 1 ? providers[0] : null;
    const matches = provider?.credentials?.filter((c) => c.id === credentialId);
    const credential = matches?.length === 1 ? matches[0] : null;
    if (typeof expectedEnabled !== "boolean" || provider?.auth_mode !== "cursor_remote" ||
        !credential || credential.enabled !== expectedEnabled ||
        typeof credential.account_name !== "string" || !credential.account_name.trim() ||
        credential.account_name.length > 320) fail("saved_credential_not_import_only");
    let token = credential.auth_token, expected = emailHash(credential.account_name);
    const claims = tokenClaims(token);
    if (Date.parse(credential.expires_at) !== claims.exp * 1000) fail("saved_expiry_mismatch");
    document = undefined;
    let released = false;
    const checkCurrent = () => guarded(() => {
      if (released) fail("saved_credential_released");
      checkActive();
      const current = readRegular(documentFile);
      try {
        if (digest(current) !== metadata.documentDigest) fail("saved_file_changed");
      } finally { current.fill(0); }
      tokenClaims(token);
    });
    checkCurrent();
    return {
      checkCurrent,
      // The callback owns no durable copy. JS strings are released, not claimed securely erased.
      use(action) { checkCurrent(); return action(token, expected); },
      release() { token = expected = undefined; released = true; },
    };
  });
}
