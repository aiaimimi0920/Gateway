import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { debugLog } from "../udio-browser/diagnostics.mjs";

test("worker diagnostics retain metadata without credentials or provider text", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "udio-diagnostic-contract-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const log = path.join(root, "worker.jsonl");
  await debugLog(log, "worker_error", {
    status: 403, hasAuthToken: true, tokenLength: 19,
    browserCdpUrl: "https://user:synthetic-secret@example.test/private?token=synthetic-secret",
    message: "Authorization: Bearer synthetic-secret", body: "provider synthetic-secret",
    nested: { authorization: "synthetic-secret" },
  });
  const raw = await readFile(log, "utf8");
  assert.equal(raw.includes("synthetic-secret"), false);
  const entry = JSON.parse(raw);
  assert.equal(entry.stage, "worker_error");
  assert.equal(entry.details.status, 403);
  assert.equal(entry.details.hasAuthToken, true);
  assert.equal(entry.details.tokenLength, 19);
});

test("diagnostic serialization is best effort for cyclic or throwing details", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "udio-diagnostic-contract-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const log = path.join(root, "worker.jsonl");
  const cycle = { status: 500 };
  cycle.self = cycle;
  await assert.doesNotReject(debugLog(log, "worker_error", cycle));
  const details = { get status() { throw new Error("synthetic getter"); } };
  await assert.doesNotReject(debugLog(log, "worker_error", details));
  await assert.doesNotReject(debugLog(null, "worker_error", cycle));
});

test("diagnostic entry size does not scale with provider payloads or track arrays", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "udio-diagnostic-contract-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const log = path.join(root, "worker.jsonl");
  await debugLog(log, "submit_json_ready", {
    body: "synthetic-secret".repeat(100_000), trackIds: new Array(100_000),
    status: Infinity, songCount: -1, targetAssetKind: "audio",
    source: "synthetic-secret", completed: true,
  });
  const raw = await readFile(log, "utf8");
  assert.ok(Buffer.byteLength(raw) < 1024);
  assert.deepEqual(JSON.parse(raw).details, { completed: true, targetAssetKind: "audio", trackCount: 100_000 });
});
