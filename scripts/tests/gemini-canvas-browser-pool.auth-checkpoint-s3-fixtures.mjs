import { createServer } from "node:http";
import { checkpointEnv } from "./gemini-canvas-browser-pool.auth-checkpoint-fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

export async function checkpointS3(t) {
  const root = storageFixture(t), records = new Map(), calls = [], hooks = {};
  let revision = 0;
  const server = createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const bytes = Buffer.concat(chunks), call = { method: req.method, url: req.url, headers: req.headers, bytes };
    calls.push(call);
    if (hooks.respond && await hooks.respond(req, res, call)) return;
    const objectPath = new URL(req.url, "http://127.0.0.1").pathname;
    const record = records.get(objectPath);
    if (req.method === "GET") {
      if (!record) { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { ETag: record.etag, "Content-Length": record.bytes.length });
      res.end(record.bytes); return;
    }
    if (req.method === "PUT") {
      if ((req.headers["if-none-match"] === "*" && record) ||
          (req.headers["if-match"] && req.headers["if-match"] !== record?.etag)) {
        res.writeHead(412); res.end(); return;
      }
      const etag = `"checkpoint-${++revision}"`;
      records.set(objectPath, { bytes, etag });
      res.writeHead(200, { ETag: etag }); res.end(); return;
    }
    res.writeHead(405); res.end();
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(async () => { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  const endpoint = `http://127.0.0.1:${server.address().port}`;
  const fields = { BUCKET: "fixture-bucket", REGION: "us-east-1", ENDPOINT: endpoint,
    ACCESS_KEY_ID: "synthetic-access", SECRET_ACCESS_KEY: "synthetic-secret", FORCE_PATH_STYLE: "true" };
  for (const [suffix, value] of Object.entries(fields)) process.env[`AI_GATEWAY_OBJECT_STORAGE_${suffix}`] = value;
  checkpointEnv(t, root, { DRIVER: "s3", PREFIX: "private-auth-checkpoints" });
  const source = { mode: "storage_state_file", identity: "synthetic-seed-identity", fingerprint: "synthetic-seed-v1",
    storageState: { cookies: [], origins: [] } };
  return { root, records, calls, hooks, source, endpoint };
}
