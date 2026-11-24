// Opt-in: production CLI in an isolated browser, all fixture routes on loopback.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:http";
import test from "node:test";
import { fileURLToPath } from "node:url";

const worker = fileURLToPath(new URL("../../scripts/lumalabs-browser-worker.mjs", import.meta.url));
const realmId = "fixture-realm";
const realmPath = "/api/vespa/realms/" + realmId;
const eventPath = "/events" + realmPath + "/events/stream/grip";
const sessionToken = "fixture-browser-session";
const userAgent = "GatewayLumaIdentityFixture/1.0";
const wsToken = "fixture-stream-token";
const actionBody = { type: "create_image_uni_1", fields: { prompt: "loopback fixture" } };

function runWorker(baseUrl, extra = {}) {
  const env = { ...process.env };
  delete env.NODE_OPTIONS; // Do not inherit preload hooks that could replace Playwright.
  return new Promise((resolve, reject) => {
    const child = execFile(process.execPath, [worker], { env, timeout: 20000, maxBuffer: 128 * 1024 },
      (error, stdout) => {
        if (error) { reject(error); return; }
        try { resolve(JSON.parse(stdout)); }
        catch (parseError) { reject(parseError); }
      });
    child.stdin.on("error", reject);
    child.stdin.end(JSON.stringify({ baseUrl, realmId, sessionToken, actionBody, userAgent,
      mediaOperation: "image", artifactField: "image", timeoutMs: 12000, ...extra }));
  });
}

function sendJson(res, body, status = 200) {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
}

function event(state, id, object_ref, boundary) {
  return "data: " + JSON.stringify({ artifact: { id, object_ref, state } }) + boundary;
}

async function withFixture(mode, check) {
  const calls = [];
  let currentToken = sessionToken;
  let events;
  let postBody;
  const server = createServer((req, res) => {
    const { pathname } = new URL(req.url, "http://127.0.0.1");
    if (pathname === "/favicon.ico") { res.writeHead(404).end(); return; }
    calls.push([req.method, pathname]);
    assert.equal(req.headers["user-agent"], userAgent, "browser requests retain the account User-Agent");
    const hasSession = req.headers.cookie?.split(/;\s*/).includes("wos-session=" + currentToken);
    if (!hasSession) { res.writeHead(401).end(); return; }
    if (pathname === "/board/" + realmId) {
      if (mode === "rotation-error") {
        currentToken = "fixture-after-board";
        res.setHeader("set-cookie", `wos-session=${currentToken}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=3600`);
      }
      res.writeHead(200, { "content-type": "text/html; charset=utf-8",
        "content-security-policy": "default-src 'none'; connect-src 'self'" });
      res.end("<!doctype html><title>Luma worker fixture</title>");
    } else if (pathname === realmPath + "/signature") {
      if (mode === "session") {
        currentToken = "fixture-after-check";
        res.setHeader("set-cookie", `wos-session=${currentToken}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=3600`);
      }
      sendJson(res, { ws_token: wsToken, cdn_url: "https://asset.fixture.invalid", query_params: "fixture=1" });
    } else if (pathname === eventPath) {
      if (req.headers.authorization !== "Bearer " + wsToken) { res.writeHead(401).end(); return; }
      events = res;
      res.writeHead(200, { "content-type": "text/event-stream" });
      res.flushHeaders(); // The worker waits for subscription headers before POST.
    } else if (pathname === realmPath + "/actions" && req.method === "POST" && events) {
      postBody = "";
      req.setEncoding("utf8");
      req.on("data", (chunk) => {
        postBody += chunk;
        if (postBody.length > 4096) req.destroy();
      });
      req.on("end", () => {
        if (mode === "rotation-error") {
          currentToken = "fixture-after-refusal";
          res.setHeader("set-cookie", `wos-session=${currentToken}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=3600`);
          sendJson(res, { error: "Not enough credits" }, 402);
        } else if (mode === "redirect") {
          res.writeHead(307, { location: realmPath + "/replayed" }).end();
        } else {
          sendJson(res, { action: { id: "original-action" }, output_artifacts: { image: ["original-output"] } });
          events.write(event("completed", "unrelated-output", "unrelated.png", "\n\n")
            + event("finalizing", "original-output", "unfinished.png", "\r\n\r\n")
            + event("completed", "original-output", "result.png", "\n\r\n"));
        }
      });
    } else {
      res.writeHead(404).end();
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    const result = await runWorker("http://127.0.0.1:" + server.address().port,
      mode === "rotation-error" || mode === "session" ? { sessionBinding: "fixture-attempt", sessionOnly: mode === "session" } : {});
    check(result);
    if (mode === "session") {
      assert.deepEqual(calls, [["GET", "/board/" + realmId], ["GET", realmPath + "/signature"]]);
      assert.equal(postBody, undefined);
      return;
    }
    assert.deepEqual(calls, [
      ["GET", "/board/" + realmId], ["GET", realmPath + "/signature"],
      ["GET", eventPath], ["POST", realmPath + "/actions"],
    ]);
    assert.deepEqual(JSON.parse(postBody), actionBody);
    assert.ok(events);
    if (!events.destroyed) await once(events, "close", { signal: AbortSignal.timeout(2000) });
    assert.ok(events.destroyed, "worker must close the open SSE connection");
  } finally {
    const closed = once(server, "close");
    server.close();
    server.closeAllConnections();
    await closed;
  }
}

test("production Luma worker returns only the original completed artifact in Chromium", { timeout: 25000 }, async () => {
  await withFixture("completed", (result) => assert.deepEqual(result, {
    ok: true, signedUrl: "https://asset.fixture.invalid/result.png?fixture=1",
    outputId: "original-output", actionId: "original-action", resolvedActionType: actionBody.type,
    realmId, mediaOperation: "image", artifactField: "image", state: "completed",
  }));
});

test("production Luma worker does not replay a Chromium POST redirect", { timeout: 25000 }, async () => {
  await withFixture("redirect", (result) => {
    assert.equal(result.ok, false);
    assert.equal(result.error?.code, "lumalabs_action_redirect");
    assert.equal(result.error.stage, "action");
    assert.equal(result.error.status, 502);
    assert.ok(!Object.hasOwn(result, "signedUrl"));
  });
});

test("production Luma worker retains the latest Cookie even when the paid action is refused", { timeout: 25000 }, async () => {
  await withFixture("rotation-error", (result) => {
    assert.equal(result.error?.status, 402);
    assert.equal(result.session?.binding, "fixture-attempt");
    assert.equal(result.session?.cookie?.value, "fixture-after-refusal");
    assert.equal(result.session.cookie.httpOnly, true);
    assert.equal(result.session.cookie.secure, true);
    assert.equal(result.session.reauthenticationRequired, false);
  });
});

test("production Luma session maintenance rotates through authenticated GET without creating an action", { timeout: 25000 }, async () => {
  await withFixture("session", (result) => {
    assert.equal(result.ok, true);
    assert.equal(result.session.cookie.value, "fixture-after-check");
    assert.equal(result.session.binding, "fixture-attempt");
    assert.equal(result.session.reauthenticationRequired, false);
  });
});
