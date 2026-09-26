import assert from "node:assert/strict";
import fs from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import test from "node:test";
import { S3Client } from "@aws-sdk/client-s3";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

const { resolveRuntimeStateSource } = await importTestableScript();

test("runtime state mirrors through the real S3 SDK and retains cached-client and error semantics", { timeout: 30000 }, async (t) => {
  const root = storageFixture(t), requests = [], clients = new Set();
  const payload = JSON.stringify({ cookies: [], origins: [] });
  const originalSend = S3Client.prototype.send;
  t.mock.method(S3Client.prototype, "send", function (...args) {
    clients.add(this);
    return Reflect.apply(originalSend, this, args);
  });
  const server = createServer((request, response) => {
    const pathname = new URL(request.url, "http://127.0.0.1").pathname;
    requests.push({ method: request.method, pathname,
      fixtureAuthorization: request.headers.authorization?.includes("Credential=FIXTUREACCESS/") === true });
    if (pathname.includes("missing")) {
      response.writeHead(404, { "content-type": "application/xml" });
      response.end("<Error><Code>NoSuchKey</Code><Message>Fixture object missing</Message></Error>");
    } else {
      response.writeHead(200, { "content-type": "application/json", "content-length": Buffer.byteLength(payload) });
      response.end(payload);
    }
  });
  t.after(async () => {
    for (const client of clients) client.destroy();
    server.closeAllConnections();
    await new Promise((resolve, reject) => server.close((error) => {
      if (error && error.code !== "ERR_SERVER_NOT_RUNNING") reject(error);
      else resolve();
    }));
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => { server.off("error", reject); resolve(); });
  });
  const address = server.address();
  assert.ok(address && typeof address === "object");
  Object.assign(process.env, {
    AI_GATEWAY_OBJECT_STORAGE_DRIVER: "s3",
    AI_GATEWAY_OBJECT_STORAGE_BUCKET: "fixture-bucket",
    AI_GATEWAY_OBJECT_STORAGE_REGION: "us-east-1",
    AI_GATEWAY_OBJECT_STORAGE_ENDPOINT: `http://127.0.0.1:${address.port}`,
    AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID: "FIXTUREACCESS",
    AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY: "fixture-only-not-a-real-credential",
    AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE: "true",
  });
  const first = await resolveRuntimeStateSource("folder/state name.json");
  assert.deepEqual(first, { mode: "storage_state_file", absolutePath: path.join(root, "folder/state name.json") });
  assert.equal(fs.readFileSync(first.absolutePath, "utf8"), payload);
  assert.deepEqual(requests, [{ method: "GET", pathname: "/fixture-bucket/folder/state%20name.json", fixtureAuthorization: true }]);
  await resolveRuntimeStateSource("folder/state name.json");
  assert.equal(requests.length, 1);

  process.env.AI_GATEWAY_OBJECT_STORAGE_ENDPOINT = "http://127.0.0.1:1";
  process.env.AI_GATEWAY_OBJECT_STORAGE_BUCKET = "next-bucket";
  delete process.env.AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID;
  delete process.env.AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY;
  await resolveRuntimeStateSource("second.json");
  assert.equal(clients.size, 1);
  assert.deepEqual(requests[1], { method: "GET", pathname: "/next-bucket/second.json", fixtureAuthorization: true });
  assert.equal(fs.readFileSync(path.join(root, "second.json"), "utf8"), payload);

  await assert.rejects(resolveRuntimeStateSource("missing/strict.json"), (error) => error.status === 500 && error.code === "gemini_canvas_remote_profile_not_supported");
  assert.equal(fs.existsSync(path.join(root, "missing/strict.json")), false);
  const fallback = await resolveRuntimeStateSource("missing/fixture", { allowFixtureEmptyProfile: true });
  assert.deepEqual(fallback, { mode: "profile_dir", absolutePath: path.join(root, "missing/fixture") });
  assert.equal(fs.statSync(fallback.absolutePath).isDirectory(), true);
  assert.equal(requests.length, 4);
});
