// Serve an isolated normal UI build and stream API traffic to one explicit loopback fixture.
import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { pipeline } from "node:stream/promises";

export async function serveMain(uiRoot, fixtureOrigin) {
  const target = new URL(fixtureOrigin);
  if (target.protocol !== "http:" || target.hostname !== "127.0.0.1" || !target.port ||
      target.username || target.password || target.pathname !== "/" || target.search || target.hash) {
    throw new Error("Only an explicit loopback fixture origin is accepted.");
  }
  const root = path.resolve(uiRoot);
  await stat(path.join(root, "index.html"));
  const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".png": "image/png" };
  const server = http.createServer((request, response) => {
    void handle(request, response).catch(error => {
      if (!response.destroyed) {
        if (!response.headersSent) response.writeHead(500);
        response.end("Isolated UI host failed");
      }
      console.error(error.message);
    });
  });
  async function handle(request, response) {
    if (request.url.startsWith("/v1/") || request.url === "/healthz" || request.url === "/readyz") {
      // Preserve request bytes, content type, grant and cancellation; never synthesize API responses.
      const upstream = http.request({ hostname: target.hostname, port: target.port,
        method: request.method, path: request.url, headers: request.headers }, reply => {
        response.writeHead(reply.statusCode, reply.headers);
        void pipeline(reply, response).catch(() => upstream.destroy());
      });
      upstream.setTimeout(190_000, () => upstream.destroy(new Error("Fixture request timed out")));
      upstream.on("error", () => {
        if (!response.headersSent) response.writeHead(502);
        response.end();
      });
      response.once("close", () => { if (!response.writableFinished) upstream.destroy(); });
      await pipeline(request, upstream).catch(() => upstream.destroy());
      return;
    }
    const url = new URL(request.url, "http://127.0.0.1");
    if (request.method !== "GET" || !url.pathname.startsWith("/ui/")) {
      response.writeHead(404).end(); return;
    }
    const relative = decodeURIComponent(url.pathname.slice(4)) || "index.html";
    const file = path.resolve(root, relative);
    if (!file.startsWith(root + path.sep)) { response.writeHead(403).end(); return; }
    let info;
    try { info = await stat(file); }
    catch (error) { if (error.code !== "ENOENT") throw error; }
    if (!info?.isFile()) { response.writeHead(404).end(); return; }
    response.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream",
      "cache-control": "no-store", "content-length": info.size });
    await pipeline(createReadStream(file), response);
  }
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  return {
    origin: `http://127.0.0.1:${server.address().port}`,
    close: () => new Promise((resolve, reject) => {
      server.close(error => error ? reject(error) : resolve());
      server.closeAllConnections();
    }),
  };
}
