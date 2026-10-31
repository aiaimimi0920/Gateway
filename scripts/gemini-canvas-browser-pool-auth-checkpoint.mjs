// Opt-in auth persistence is separate from import seeds and business request retries.
import { createHash } from "node:crypto";
import path from "node:path";
import { resolveObjectStorageConfig } from "./gemini-canvas-browser-pool-runtime-state.mjs";
import { boundedCheckpointTask, checkpointError, decryptCheckpoint, encryptCheckpoint } from "./gemini-canvas-browser-pool-auth-checkpoint-codec.mjs";
import { readLocalCheckpoint, writeLocalCheckpoint } from "./gemini-canvas-browser-pool-auth-checkpoint-local.mjs";
import { readS3Checkpoint, writeS3Checkpoint } from "./gemini-canvas-browser-pool-auth-checkpoint-s3.mjs";

const digest = value => createHash("sha256").update(value).digest("hex");

function checkpointConfig() {
  const env = process.env;
  const driver = (env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_DRIVER ?? "disabled").trim().toLowerCase();
  if (driver === "disabled") return null;
  const fail = () => { throw checkpointError("config_invalid", 500); };
  if (!["local", "s3"].includes(driver)) return fail();
  const encoded = env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_KEY ?? "";
  const key = Buffer.from(encoded, "base64");
  if (key.length !== 32 || key.toString("base64") !== encoded) return fail();
  const keyId = digest(key);
  if (driver === "local") {
    const directory = env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_DIR ?? "";
    if (!path.isAbsolute(directory) || directory.startsWith("\\\\")) return fail();
    const resolved = path.resolve(directory);
    return { driver, directory: resolved, key, namespace: JSON.stringify([driver, resolved, keyId]) };
  }
  const config = resolveObjectStorageConfig();
  const prefix = env.GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_PREFIX ?? "";
  if (!/^[A-Za-z0-9_-]+(?:\/[A-Za-z0-9_-]+)*$/.test(prefix) || prefix.length > 200 ||
      !config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) return fail();
  let endpoint;
  try { endpoint = new URL(config.endpoint); } catch { return fail(); }
  if (endpoint.username || endpoint.password || endpoint.search || endpoint.hash ||
      (endpoint.protocol !== "https:" && !(endpoint.protocol === "http:" && ["127.0.0.1", "localhost", "[::1]"].includes(endpoint.hostname)))) return fail();
  return { ...config, driver, key, prefix,
    namespace: JSON.stringify([driver, config.endpoint, config.bucket, config.region, config.forcePathStyle, prefix, keyId]) };
}

export async function loadAuthCheckpoint(source) {
  // CDP and persistent profiles retain their native owner; no automatic exports.
  if (source?.mode !== "storage_state_file") return null;
  const config = checkpointConfig();
  if (!config) return null;
  const binding = digest(JSON.stringify(["auth-checkpoint-v1", config.namespace, source.identity, source.fingerprint]));
  const stored = await (config.driver === "local" ? readLocalCheckpoint : readS3Checkpoint)(config, binding);
  const record = stored.bytes ? decryptCheckpoint(config, binding, stored.bytes) : null;
  if (record?.blocked) throw checkpointError("session_invalid", 401);
  if (record && config.driver === "local" && record.revision !== stored.token) throw checkpointError("invalid_record");
  return { config, binding, token: stored.token, storageState: record?.storageState ?? source.storageState };
}

export async function saveAuthCheckpoint(checkpoint, storageState, { blocked = false } = {}) {
  const { config, binding, token } = checkpoint;
  const record = encryptCheckpoint(config, binding, storageState, blocked);
  const nextToken = await (config.driver === "local" ? writeLocalCheckpoint : writeS3Checkpoint)(config, binding, token, record);
  checkpoint.token = nextToken;
  checkpoint.storageState = blocked ? null : storageState;
}

export async function settleAuthCheckpoint(entry, outcome, { closeContext, contextHasGeminiAuthCookies, log }) {
  if (!entry?.authCheckpoint) return;
  try {
    if (outcome.invalidAuth) {
      await saveAuthCheckpoint(entry.authCheckpoint, null, { blocked: true });
      await closeContext(outcome.runtimeStateObjectKey);
    } else if (outcome.succeeded && !entry.contextClosed) {
      const state = await boundedCheckpointTask(async signal => {
        if (!await contextHasGeminiAuthCookies(entry.context, outcome.baseUrl)) return null;
        signal.throwIfAborted();
        const snapshot = await entry.context.storageState({ indexedDB: true });
        signal.throwIfAborted();
        return snapshot;
      });
      if (state) await saveAuthCheckpoint(entry.authCheckpoint, state);
    }
  } catch (error) {
    // Persistence failure must not turn an accepted generation into a retryable failure.
    log("Gemini auth checkpoint not committed", JSON.stringify({ code:
      error?.code?.startsWith("gemini_canvas_auth_checkpoint_") ? error.code : "gemini_canvas_auth_checkpoint_capture_failed" }));
    await closeContext(outcome.runtimeStateObjectKey).catch(() => undefined);
  }
}
