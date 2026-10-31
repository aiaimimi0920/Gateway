import assert from "node:assert/strict";
import fs from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import test from "node:test";
import { refreshFixture, state } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";

test("conditional S3 source refresh ignores stale mirrors, replaces changed sessions and never falls back on failure", { timeout: 30000 }, async t => {
  const f = refreshFixture(t), requests = [];
  const mirror = f.write("stale-local-mirror", "remote/state.json");
  let value = "first", etag = '"v1"', status = 200;
  const server = createServer((request, response) => {
    requests.push({ method: request.method, pathname: new URL(request.url, "http://127.0.0.1").pathname,
      ifNoneMatch: request.headers["if-none-match"] ?? null,
      fixtureAuthorization: request.headers.authorization?.includes("Credential=FIXTUREACCESS/") === true });
    if (status !== 200) {
      response.writeHead(status, { "content-type": "application/xml" });
      response.end("<Error><Code>NoSuchKey</Code><Message>PRIVATE-ERROR-BODY</Message></Error>");
    } else if (etag && request.headers["if-none-match"] === etag) {
      response.writeHead(304, { etag }); response.end();
    } else {
      const body = JSON.stringify(state(value));
      response.writeHead(200, { "content-type": "application/json", "content-length": Buffer.byteLength(body), ...(etag ? { etag } : {}) });
      response.end(body);
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(async () => {
    server.closeAllConnections();
    await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  });
  Object.assign(process.env, { AI_GATEWAY_OBJECT_STORAGE_DRIVER: "s3", AI_GATEWAY_OBJECT_STORAGE_BUCKET: "fixture",
    AI_GATEWAY_OBJECT_STORAGE_REGION: "us-east-1", AI_GATEWAY_OBJECT_STORAGE_ENDPOINT: `http://127.0.0.1:${server.address().port}`,
    AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID: "FIXTUREACCESS", AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY: "fixture-only-not-real",
    AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE: "true" });
  const args = f.args("remote/state.json");
  const original = await f.owner.ensureContext(args);
  assert.equal(f.sessions[0].loaded.cookies[0].value, "first");
  assert.equal(await f.owner.ensureContext(args), original);
  assert.deepEqual(requests.map(request => request.ifNoneMatch), [null, '"v1"']);
  etag = '"v1-metadata-only"';
  assert.equal(await f.owner.ensureContext(args), original);
  assert.equal(await f.owner.ensureContext(args), original);
  assert.equal(requests.at(-1).ifNoneMatch, etag);
  value = "second"; etag = '"v2"';
  const replacement = await f.owner.ensureContext(args);
  assert.notEqual(replacement, original);
  assert.equal(f.sessions[1].loaded.cookies[0].value, "second");
  assert.equal(requests.at(-1).ifNoneMatch, '"v1-metadata-only"');
  status = 404;
  await assert.rejects(f.owner.ensureContext(args), error => error.code === "gemini_canvas_runtime_state_read_failed"
    && !error.message.includes("PRIVATE-ERROR-BODY"));
  assert.equal(f.launch.mock.callCount(), 2);
  assert.equal(f.owner.contexts.get(args.runtimeStateObjectKey), replacement);
  assert.equal(replacement.busy, false);
  status = 200; value = "third"; etag = null;
  assert.notEqual(await f.owner.ensureContext(args), replacement);
  const noEtag = await f.owner.ensureContext(args);
  assert.equal(noEtag, f.owner.contexts.get(args.runtimeStateObjectKey));
  assert.equal(requests.at(-1).ifNoneMatch, null);
  assert.equal(f.launch.mock.callCount(), 3);
  process.env.AI_GATEWAY_OBJECT_STORAGE_BUCKET = "different-namespace";
  await f.owner.ensureContext(args);
  assert.equal(requests.at(-1).ifNoneMatch, null);
  assert.equal(f.launch.mock.callCount(), 4);
  assert.ok(requests.every(request => request.method === "GET" && request.fixtureAuthorization));
  assert.ok(requests.at(-1).pathname.startsWith("/different-namespace/"));
  assert.equal(JSON.parse(fs.readFileSync(mirror, "utf8")).cookies[0].value, "stale-local-mirror");
  assert.equal(fs.existsSync(path.join(f.root, "remote/state.json")), true);
});
