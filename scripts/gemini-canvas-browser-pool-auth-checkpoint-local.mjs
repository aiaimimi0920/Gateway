// SQLite provides crash-safe, cross-process CAS without stale lock-file reclamation.
import { chmodSync, lstatSync, mkdirSync, openSync, closeSync } from "node:fs";
import path from "node:path";
import { checkpointError, MAX_CHECKPOINT_BYTES } from "./gemini-canvas-browser-pool-auth-checkpoint-codec.mjs";

async function withDatabase(config, operation) {
  let db;
  try {
    const { DatabaseSync } = await import("node:sqlite");
    mkdirSync(config.directory, { recursive: true, mode: 0o700 });
    if (lstatSync(config.directory).isSymbolicLink()) throw new Error();
    const file = path.join(config.directory, "auth-checkpoints-v1.sqlite3");
    try { const fd = openSync(file, "wx", 0o600); closeSync(fd); }
    catch (error) { if (error.code !== "EEXIST") throw error; }
    if (!lstatSync(file).isFile() || lstatSync(file).isSymbolicLink()) throw new Error();
    if (process.platform !== "win32") chmodSync(file, 0o600);
    db = new DatabaseSync(file, { enableExtension: false });
    db.exec("PRAGMA busy_timeout=250; PRAGMA trusted_schema=OFF; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA max_page_count=65536;");
    db.exec("CREATE TABLE IF NOT EXISTS auth_checkpoint_v1 (binding TEXT PRIMARY KEY, revision TEXT NOT NULL, payload BLOB NOT NULL)");
    return operation(db);
  } catch (error) {
    if (error?.code?.startsWith("gemini_canvas_auth_checkpoint_")) throw error;
    throw checkpointError("local_storage_failed");
  } finally {
    db?.close();
  }
}

export async function readLocalCheckpoint(config, binding) {
  return withDatabase(config, db => {
    const row = db.prepare("SELECT revision, length(payload) AS size FROM auth_checkpoint_v1 WHERE binding=?").get(binding);
    if (!row) return { token: null, bytes: null };
    if (row.size > MAX_CHECKPOINT_BYTES) throw checkpointError("too_large", 413);
    const payload = db.prepare("SELECT payload FROM auth_checkpoint_v1 WHERE binding=? AND revision=?").get(binding, row.revision);
    // Another writer may win between the bounded-size check and fetch. Never load a mixed revision.
    if (!payload) throw checkpointError("conflict", 409);
    return { token: row.revision, bytes: Buffer.from(payload.payload) };
  });
}

export async function writeLocalCheckpoint(config, binding, token, record) {
  return withDatabase(config, db => {
    db.exec("BEGIN IMMEDIATE");
    try {
      let result;
      if (token === null) {
        if (db.prepare("SELECT count(*) AS count FROM auth_checkpoint_v1").get().count >= 1024) {
          throw checkpointError("capacity", 507);
        }
        result = db.prepare("INSERT OR IGNORE INTO auth_checkpoint_v1 (binding,revision,payload) VALUES (?,?,?)")
          .run(binding, record.revision, record.bytes);
      } else {
        result = db.prepare("UPDATE auth_checkpoint_v1 SET revision=?,payload=? WHERE binding=? AND revision=?")
          .run(record.revision, record.bytes, binding, token);
      }
      if (result.changes !== 1) throw checkpointError("conflict", 409);
      db.exec("COMMIT");
      return record.revision;
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  });
}
