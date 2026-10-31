// Imported JSON is a revisioned seed, not a destination for live browser cookies.
import { createHash } from "node:crypto";
import { createReadStream, lstatSync } from "node:fs";
import path from "node:path";
import { GetObjectCommand, S3Client } from "@aws-sdk/client-s3";
import { getStorageRoot, resolveObjectStorageConfig, resolveRuntimeStateSource } from "./gemini-canvas-browser-pool-runtime-state.mjs";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { resolveStorageObjectPath } from "./aistudio-live-probe/storage-path.mjs";
import { readRuntimeStateBody, withObjectStorageRequest } from "./aistudio-live-probe/object-storage-request.mjs";

function stateError(code, message, status) {
  return Object.assign(new Error(message), { code: `gemini_canvas_${code}`, status });
}

async function boundedStateRead(operation) {
  try {
    return await withObjectStorageRequest(operation);
  } catch (error) {
    if (error?.status === 413) throw stateError("runtime_state_too_large", "Gemini runtime state exceeds 16 MiB.", 413);
    if (error?.status === 504) throw stateError("runtime_state_read_timeout", "Gemini runtime state read exceeded 30 seconds.", 504);
    // SDK and filesystem errors can contain paths, object names or response bodies.
    throw stateError("runtime_state_read_failed", "Gemini runtime state could not be read; cached credentials were not used.", 502);
  }
}

function snapshot(source, bytes) {
  let storageState;
  try {
    storageState = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
    if (!storageState || !Array.isArray(storageState.cookies) || !Array.isArray(storageState.origins)) throw new Error();
  } catch {
    throw stateError("runtime_state_invalid_json", "Gemini runtime state must contain valid UTF-8 storageState JSON.", 400);
  }
  return { ...source, fingerprint: createHash("sha256").update(bytes).digest("hex"), storageState };
}

async function remoteSnapshot(config, key, previous) {
  let absolutePath;
  try { absolutePath = resolveStorageObjectPath(getStorageRoot(), key); }
  catch { throw stateError("runtime_state_invalid_path", "Gemini object key must remain inside its storage root.", 400); }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw stateError("object_storage_not_configured", "Remote object storage is not fully configured.", 500);
  }
  const identity = JSON.stringify(["remote", config.endpoint, config.bucket, config.region, config.forcePathStyle, absolutePath, key]);
  const cached = previous?.identity === identity ? previous : null;
  // One request-scoped client avoids reusing another storage configuration's credentials.
  // Destroy it on every path; no unbounded client/version cache or stale mirror fallback.
  const client = new S3Client({ region: config.region, endpoint: config.endpoint,
    forcePathStyle: config.forcePathStyle, credentials: { accessKeyId: config.accessKeyId, secretAccessKey: config.secretAccessKey } });
  try {
    const result = await boundedStateRead(async signal => {
      let response;
      try {
        response = await client.send(new GetObjectCommand({ Bucket: config.bucket, Key: key,
          ...(cached?.etag ? { IfNoneMatch: cached.etag } : {}) }), { abortSignal: signal });
      } catch (error) {
        if (cached?.etag && error?.$metadata?.httpStatusCode === 304) return { unchanged: true };
        throw error;
      }
      const bytes = await readRuntimeStateBody(response.Body, signal, response.ContentLength);
      const etag = typeof response.ETag === "string" && /^"[^"\r\n]{1,256}"$/.test(response.ETag) ? response.ETag : null;
      return { bytes, etag };
    });
    return result.unchanged ? cached : snapshot({ mode: "storage_state_file", absolutePath, identity, etag: result.etag }, result.bytes);
  } finally {
    client.destroy();
  }
}

export async function loadContextStateSnapshot(key, { allowFixtureEmptyProfile = false, previous } = {}) {
  const normalizedKey = normalizeString(key);
  const config = resolveObjectStorageConfig();
  if (config.driver !== "local" && normalizedKey && !path.isAbsolute(normalizedKey) && /\.json$/i.test(normalizedKey)) {
    // Fresh conditional GET is authoritative even when a legacy local mirror exists.
    // Do not overwrite that mirror: readers consume this exact parsed snapshot.
    return remoteSnapshot(config, normalizedKey, previous);
  }
  const jsonPath = normalizedKey && /\.json$/i.test(normalizedKey)
    ? (path.isAbsolute(normalizedKey) ? normalizedKey : resolveJsonPath(normalizedKey)) : null;
  // A removed JSON must not be recreated as an empty profile and reseeded by an old header.
  const source = jsonPath && !isExistingDirectory(jsonPath)
    ? { mode: "storage_state_file", absolutePath: jsonPath }
    : await resolveRuntimeStateSource(key, { allowFixtureEmptyProfile });
  if (previous?.mode === "storage_state_file" && source.mode !== "storage_state_file") {
    throw stateError("runtime_state_read_failed", "Gemini JSON source is no longer available; cached credentials were not used.", 502);
  }
  const identity = JSON.stringify(["local", source.mode, source.absolutePath]);
  if (source.mode !== "storage_state_file") return { ...source, identity, fingerprint: null };
  const bytes = await boundedStateRead(signal => readRuntimeStateBody(
    createReadStream(source.absolutePath, { signal, highWaterMark: 65536 }), signal));
  return snapshot({ ...source, identity, etag: null }, bytes);
}

function resolveJsonPath(key) {
  try { return resolveStorageObjectPath(getStorageRoot(), key); }
  catch { throw stateError("runtime_state_invalid_path", "Gemini object key must remain inside its storage root.", 400); }
}

function isExistingDirectory(file) {
  try { return lstatSync(file).isDirectory(); }
  // A racing removal or access failure is handled by the bounded reader's fixed error.
  catch { return false; }
}
