// Only authenticated ciphertext crosses the checkpoint storage boundary.
import { createCipheriv, createDecipheriv, randomBytes, randomUUID } from "node:crypto";

export const MAX_CHECKPOINT_BYTES = 16 * 1024 * 1024;
const MAGIC = Buffer.from("NGAC1");

export function checkpointError(kind, status = 502) {
  return Object.assign(new Error(`Gemini auth checkpoint ${kind.replaceAll("_", " ")}.`), {
    code: `gemini_canvas_auth_checkpoint_${kind}`, status,
  });
}

export function checkpointState(state) {
  if (!state || !Array.isArray(state.cookies) || !Array.isArray(state.origins)) {
    throw checkpointError("invalid_state", 400);
  }
  return state;
}

export function encryptCheckpoint(config, binding, storageState, blocked = false) {
  const revision = randomUUID();
  const encoded = JSON.stringify({ version: 1, revision, blocked,
    storageState: blocked ? null : checkpointState(storageState) });
  if (Buffer.byteLength(encoded, "utf8") > MAX_CHECKPOINT_BYTES - 64) throw checkpointError("too_large", 413);
  const plaintext = Buffer.from(encoded, "utf8");
  const nonce = randomBytes(12);
  const cipher = createCipheriv("aes-256-gcm", config.key, nonce);
  cipher.setAAD(Buffer.from(binding));
  const encrypted = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  return { revision, bytes: Buffer.concat([MAGIC, nonce, cipher.getAuthTag(), encrypted]) };
}

export function decryptCheckpoint(config, binding, bytes) {
  try {
    if (bytes.length < 34 || bytes.length > MAX_CHECKPOINT_BYTES || !bytes.subarray(0, 5).equals(MAGIC)) throw new Error();
    const decipher = createDecipheriv("aes-256-gcm", config.key, bytes.subarray(5, 17));
    decipher.setAAD(Buffer.from(binding));
    decipher.setAuthTag(bytes.subarray(17, 33));
    const plaintext = Buffer.concat([decipher.update(bytes.subarray(33)), decipher.final()]);
    const record = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(plaintext));
    if (record.version !== 1 || !/^[a-f0-9-]{36}$/.test(record.revision) || typeof record.blocked !== "boolean") throw new Error();
    if (record.blocked ? record.storageState !== null : !checkpointState(record.storageState)) throw new Error();
    return record;
  } catch {
    // Never leak crypto/JSON error details, plaintext, filesystem paths or SDK bodies.
    throw checkpointError("invalid_record", 502);
  }
}

export async function boundedCheckpointTask(operation) {
  const controller = new AbortController();
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => {
      const error = checkpointError("timeout", 504);
      controller.abort(error);
      reject(error);
    }, 5000);
  });
  try {
    return await Promise.race([Promise.resolve().then(() => operation(controller.signal)), deadline]);
  } finally {
    clearTimeout(timer);
  }
}
