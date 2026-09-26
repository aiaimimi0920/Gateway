import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import * as io from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  isGenerateProxySubmit,
  normalizeString,
  normalizeTimeoutMs,
  normalizeUrlPath,
  parseCliArgs,
  withTimeout,
  writeOptionalJson,
} from "../udio-capture-next-generate.input.mjs";
import {
  createBrowserTargetMonitor,
  installGenerateHook,
} from "../udio-capture-next-generate.browser.mjs";

class FakeCdpSession {
  constructor() {
    this.listeners = new Map();
    this.calls = [];
  }

  on(name, listener) {
    const listeners = this.listeners.get(name) ?? new Set();
    listeners.add(listener);
    this.listeners.set(name, listeners);
  }

  off(name, listener) {
    this.listeners.get(name)?.delete(listener);
  }

  emit(name, event) {
    for (const listener of this.listeners.get(name) ?? []) {
      listener(event);
    }
  }

  async send(method, params = {}) {
    this.calls.push({ method, params });
    if (method === "Target.getTargets") {
      return {
        targetInfos: [
          {
            targetId: "target-1",
            type: "page",
            url: "https://www.udio.com/create",
            title: "Udio",
          },
        ],
      };
    }
    if (method === "Target.attachToTarget") {
      return { sessionId: "session-1" };
    }
    if (method === "Target.sendMessageToTarget") {
      const message = JSON.parse(params.message);
      queueMicrotask(() => {
        this.emit("Target.receivedMessageFromTarget", {
          sessionId: params.sessionId,
          message: JSON.stringify({ id: message.id, result: {} }),
        });
      });
    }
    return {};
  }

  async detach() {
    this.calls.push({ method: "Target.detachFromBrowser" });
  }
}

test("input helpers preserve URL, timeout, CLI and JSON contracts", async (t) => {
  assert.equal(normalizeString("  value "), "value");
  assert.equal(normalizeString(42), null);
  assert.equal(normalizeTimeoutMs("bad", 10, 1, 20), 10);
  assert.equal(normalizeTimeoutMs("99", 10, 1, 20), 20);
  assert.equal(normalizeTimeoutMs("2.9", 10, 1, 20), 2);
  assert.equal(normalizeUrlPath("https://www.udio.com/api/generate-proxy?q=1"), "/api/generate-proxy");
  assert.equal(normalizeUrlPath("http://["), null);
  assert.equal(isGenerateProxySubmit("/api/generate-proxy", " post "), true);
  assert.equal(isGenerateProxySubmit("/api/generate-proxy", "GET"), false);
  assert.deepEqual(parseCliArgs(["--browser-cdp-url", "http://127.0.0.1", "--verbose"]), {
    "browser-cdp-url": "http://127.0.0.1",
    verbose: true,
  });

  const root = await io.mkdtemp(path.join(os.tmpdir(), "udio-capture-contract-"));
  t.after(() => io.rm(root, { recursive: true, force: true }));
  const statusPath = path.join(root, "nested", "status.json");
  await writeOptionalJson(statusPath, { ok: true, phase: "armed" });
  assert.deepEqual(JSON.parse(await io.readFile(statusPath, "utf8")), { ok: true, phase: "armed" });
  await assert.rejects(withTimeout(Promise.reject(new Error("synthetic")), 100, "contract"), /synthetic/);
});

test("browser target monitor captures generate-proxy request and response", async () => {
  const root = new FakeCdpSession();
  const monitor = await createBrowserTargetMonitor({
    newBrowserCDPSession: async () => root,
  });
  assert.ok(monitor);

  root.emit("Target.receivedMessageFromTarget", {
    sessionId: "session-1",
    message: JSON.stringify({
      method: "Network.requestWillBeSent",
      params: {
        requestId: "request-1",
        request: {
          url: "https://www.udio.com/api/generate-proxy",
          method: "POST",
          headers: { "content-type": "application/json" },
          postData: "{}",
        },
      },
    }),
  });
  root.emit("Target.receivedMessageFromTarget", {
    sessionId: "session-1",
    message: JSON.stringify({
      method: "Network.responseReceived",
      params: {
        requestId: "request-1",
        response: { status: 200, headers: { "content-type": "application/json" } },
      },
    }),
  });

  assert.deepEqual(monitor.readCapture(), {
    source: "browser_target",
    sessionId: "session-1",
    targetType: "page",
    targetUrl: "https://www.udio.com/create",
    requestId: "request-1",
    request: {
      url: "https://www.udio.com/api/generate-proxy",
      method: "POST",
      headers: { "content-type": "application/json" },
      body: "{}",
    },
    response: {
      status: 200,
      headers: { "content-type": "application/json" },
      body: null,
    },
  });

  await monitor.dispose();
  assert.ok(root.calls.some(({ method }) => method === "Target.detachFromTarget"));
  assert.ok(root.calls.some(({ method }) => method === "Target.detachFromBrowser"));
});

test("page hook installation stays delegated to the browser owner", async () => {
  let hookSource = "";
  await installGenerateHook({
    evaluate: async (callback) => {
      hookSource = String(callback);
    },
  });
  assert.match(hookSource, /__udioGenerateHookInstalled/);
  assert.match(hookSource, /\/api\/generate-proxy/);
  assert.match(hookSource, /XMLHttpRequest/);
});

test("entry point preserves safe failure JSON when browser CDP input is missing", () => {
  const entry = fileURLToPath(new URL("../udio-capture-next-generate.mjs", import.meta.url));
  const result = spawnSync(process.execPath, [entry], {
    input: "{}",
    encoding: "utf8",
    timeout: 10_000,
    env: { PATH: process.env.PATH, SystemRoot: process.env.SystemRoot },
  });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 1, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    ok: false,
    phase: "failed",
    message: "browserCdpUrl is required.",
  });
});
