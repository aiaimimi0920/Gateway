import assert from "node:assert/strict";
import test from "node:test";
import http from "node:http";
import { once } from "node:events";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { WebSocketServer } from "ws";
import { importBrowserSession } from "../chatgpt-web-session/browser-import.mjs";
import { connectPage } from "../chatgpt-web-session/browser-import-cdp.mjs";

test("selected-page CDP captures session and read-only catalog, then disconnects without closing the browser", { timeout: 10000 }, async () => {
  const methods = [];
  let evaluationResult;
  let catalogResult = { status: 200, models: ["real-web-slug"] };
  const server = http.createServer((request, response) => {
    assert.equal(request.url, "/json/list");
    response.setHeader("Content-Type", "application/json");
    response.end(JSON.stringify([{ type: "page", id: "selected", title: "ChatGPT", url: "https://chatgpt.com/",
      webSocketDebuggerUrl: `ws://127.0.0.1:${server.address().port}/devtools/page/selected` }]));
  });
  const wss = new WebSocketServer({ server });
  let disconnected;
  const closed = new Promise((resolve) => { disconnected = resolve; });
  wss.on("connection", (socket) => {
    socket.on("close", disconnected);
    socket.on("message", (bytes) => {
      const request = JSON.parse(bytes.toString());
      methods.push(request.method);
      let result;
      if (request.method === "Runtime.evaluate") {
        const catalog = request.params.expression.includes("https://chatgpt.com/backend-api/models");
        assert.ok(catalog || request.params.expression.includes("https://chatgpt.com/api/auth/session"));
        assert.ok(!request.params.expression.includes("/oauth/token"));
        assert.ok(!request.params.expression.includes("/backend-api/conversation"));
        const token = `e30.${Buffer.from(JSON.stringify({ exp: 4070908800 })).toString("base64url")}.sig`;
        result = catalog ? { result: { value: catalogResult } }
          : evaluationResult || { result: { value: { accessToken: token, userAgent: "Browser", accountName: "fixture@example.test" } } };
      } else {
        assert.equal(request.method, "Network.getCookies");
        assert.deepEqual(request.params.urls, ["https://chatgpt.com/"]);
        result = { cookies: [{ name: "oai-did", value: "device", domain: "chatgpt.com" }] };
      }
      socket.send(JSON.stringify({ id: request.id, result }));
    });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    const port = server.address().port;
    const list = await importBrowserSession({ operation: "list", port });
    assert.deepEqual(list.tabs, [{ id: "selected", title: "ChatGPT" }]);
    const worker = spawn(process.execPath, [fileURLToPath(new URL("../chatgpt-web-browser-import-worker.mjs", import.meta.url))], { windowsHide: true });
    const output = [];
    worker.stdout.on("data", (chunk) => output.push(chunk));
    worker.stdin.end(JSON.stringify({ operation: "list", port }));
    const [exitCode] = await once(worker, "close");
    assert.equal(exitCode, 0);
    assert.deepEqual(JSON.parse(Buffer.concat(output)), { ok: true, tabs: list.tabs });
    assert.equal(methods.length, 0);
    await assert.rejects(importBrowserSession({ operation: "prepare", port, targetId: "other" }), /tab_unavailable/);
    for (const [result, code] of [
      [{ exceptionDetails: { text: "private navigation exception" } }, "session_page_unavailable"],
      [{ result: { value: { failure: "session_request_failed" } } }, "session_request_failed"],
      [{ result: { value: { failure: "private page-controlled error" } } }, "session_response_invalid"],
    ]) {
      evaluationResult = result;
      await assert.rejects(importBrowserSession({ operation: "prepare", port, targetId: "selected" }),
        (error) => error.message === code);
    }
    evaluationResult = { result: { value: { failure: "session_service_unavailable" } } };
    const failedWorker = spawn(process.execPath, [fileURLToPath(new URL("../chatgpt-web-browser-import-worker.mjs", import.meta.url))], { windowsHide: true });
    const failureOutput = [];
    failedWorker.stdout.on("data", (chunk) => failureOutput.push(chunk));
    failedWorker.stdin.end(JSON.stringify({ operation: "prepare", port, targetId: "selected" }));
    const [failureExit] = await once(failedWorker, "close");
    assert.equal(failureExit, 0);
    assert.deepEqual(JSON.parse(Buffer.concat(failureOutput)), { ok: false, code: "session_service_unavailable" });
    evaluationResult = undefined;
    const captured = await importBrowserSession({ operation: "prepare", port, targetId: "selected" });
    assert.equal(captured.accountName, "fixture@example.test");
    assert.equal(captured.material.deviceId, "device");
    assert.deepEqual(captured.material.modelSlugs, ["real-web-slug"]);
    await closed;
    assert.deepEqual(methods, [...Array(5).fill("Runtime.evaluate"), "Network.getCookies", "Runtime.evaluate"]);
    catalogResult = { status: 403, code: "catalog_verification_required" };
    const deniedCatalog = await importBrowserSession({ operation: "prepare", port, targetId: "selected" });
    assert.equal(deniedCatalog.accountName, "fixture@example.test");
    assert.ok(deniedCatalog.material.authToken);
    assert.equal(deniedCatalog.material.modelSlugs, undefined);
    assert.deepEqual(deniedCatalog.catalog, { status: 403, code: "catalog_verification_required" });
    const beforeDeferred = methods.length;
    const deferred = await importBrowserSession({ operation: "prepare", port, targetId: "selected" }, { catalogDeadline: Date.now() - 1 });
    assert.ok(deferred.material.authToken);
    assert.equal(deferred.catalog.code, "catalog_unavailable");
    assert.deepEqual(methods.slice(beforeDeferred), ["Runtime.evaluate", "Network.getCookies"]);
  } finally {
    for (const client of wss.clients) client.terminate();
    wss.close(); server.closeAllConnections(); server.close();
  }
});

test("a stalled real CDP socket reports a read timeout, not a human-login timeout", { timeout: 3000 }, async () => {
  const server = http.createServer();
  const wss = new WebSocketServer({ server });
  const methods = [];
  wss.on("connection", (socket) => socket.on("message", (bytes) => methods.push(JSON.parse(bytes).method)));
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  let cdp;
  try {
    cdp = await connectPage(`ws://127.0.0.1:${server.address().port}/devtools/page/owned`, { commandTimeoutMs: 10 });
    await assert.rejects(cdp.command("Runtime.evaluate", { expression: "Promise.resolve(1)", awaitPromise: true }),
      (error) => error.message === "session_read_timeout");
    assert.deepEqual(methods, ["Runtime.evaluate"]);
  } finally {
    cdp?.close(); for (const client of wss.clients) client.terminate();
    wss.close(); server.closeAllConnections(); server.close();
  }
});
