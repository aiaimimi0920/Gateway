import { writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { chromium } from "playwright-core";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
let resolvedInputCache = null;
let nextCdpCommandId = 1_000;
const UDIO_DEFAULT_ORIGIN = "https://www.udio.com";

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeTimeoutMs(value, fallback, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(Math.max(Math.trunc(parsed), min), max);
}

function normalizeUrlPath(rawUrl, baseUrl = UDIO_DEFAULT_ORIGIN) {
  const value = normalizeString(rawUrl);
  if (!value) {
    return null;
  }
  try {
    return new URL(value, baseUrl).pathname;
  } catch {
    return null;
  }
}

function isGenerateProxySubmit(rawUrl, method) {
  return (
    String(method ?? "").trim().toUpperCase() === "POST" &&
    normalizeUrlPath(rawUrl) === "/api/generate-proxy"
  );
}

async function ensureParentDir(filePath) {
  const fs = await import("node:fs/promises");
  await fs.mkdir(path.dirname(filePath), { recursive: true });
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function parseCliArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const entry = argv[index];
    if (!entry?.startsWith("--")) {
      continue;
    }
    const key = entry.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

async function resolveInput() {
  const raw = (await readStdin()).replace(/^\uFEFF/, "").trim();
  if (raw) {
    return JSON.parse(raw);
  }

  const cli = parseCliArgs(process.argv.slice(2));
  return {
    browserCdpUrl: cli["browser-cdp-url"],
    browserCdpTargetUrl: cli["browser-cdp-target-url"],
    outputPath: cli["output-file"],
    readyPath: cli["ready-file"],
    statusPath: cli["status-file"],
    timeoutMs: cli["timeout-ms"],
    browserCdpConnectTimeoutMs: cli["browser-cdp-connect-timeout-ms"],
  };
}

async function writeJson(filePath, value) {
  const fs = await import("node:fs/promises");
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, JSON.stringify(value, null, 2), "utf8");
}

async function writeOptionalJson(filePath, value) {
  const normalized = normalizeString(filePath);
  if (!normalized) {
    return;
  }
  await ensureParentDir(normalized);
  await writeJson(normalized, value);
}

function withTimeout(promise, timeoutMs, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

async function createBrowserTargetMonitor(browser) {
  if (typeof browser.newBrowserCDPSession !== "function") {
    return null;
  }

  const root = await browser.newBrowserCDPSession();
  const attached = new Map();
  const captures = [];

  const sendToTarget = async (sessionId, method, params = {}, timeoutMs = 2_000) => {
    const id = nextCdpCommandId++;
    return await withTimeout(
      new Promise(async (resolve, reject) => {
        const listener = (event) => {
          if (event.sessionId !== sessionId) {
            return;
          }
          let payload = null;
          try {
            payload = JSON.parse(event.message);
          } catch {
            return;
          }
          if (payload.id !== id) {
            return;
          }
          root.off("Target.receivedMessageFromTarget", listener);
          if (payload.error) {
            reject(new Error(payload.error.message ?? JSON.stringify(payload.error)));
            return;
          }
          resolve(payload.result ?? {});
        };

        root.on("Target.receivedMessageFromTarget", listener);
        try {
          await root.send("Target.sendMessageToTarget", {
            sessionId,
            message: JSON.stringify({ id, method, params }),
          });
        } catch (error) {
          root.off("Target.receivedMessageFromTarget", listener);
          reject(error);
        }
      }),
      timeoutMs,
      `${method} for target session ${sessionId}`,
    );
  };

  const onReceivedMessage = (event) => {
    const meta = attached.get(event.sessionId) ?? {};
    let payload = null;
    try {
      payload = JSON.parse(event.message);
    } catch {
      return;
    }

    if (payload.method === "Network.requestWillBeSent") {
      const request = payload.params?.request ?? {};
      if (isGenerateProxySubmit(request.url, request.method)) {
        captures.push({
          source: "browser_target",
          sessionId: event.sessionId,
          targetType: meta.type ?? null,
          targetUrl: meta.url ?? null,
          requestId: payload.params?.requestId ?? null,
          request: {
            url: request.url ?? null,
            method: request.method ?? null,
            headers: request.headers ?? {},
            body: request.postData ?? null,
          },
          response: null,
        });
      }
      return;
    }

    if (payload.method === "Network.responseReceived") {
      const response = payload.params?.response ?? {};
      const requestId = payload.params?.requestId ?? null;
      const match = captures.find(
        (entry) => entry.requestId && requestId && entry.requestId === requestId,
      );
      if (!match) {
        return;
      }
      match.response = {
        status: response.status ?? null,
        headers: response.headers ?? {},
        body: null,
      };
    }
  };

  root.on("Target.receivedMessageFromTarget", onReceivedMessage);

  const targets = await root.send("Target.getTargets");
  const targetInfos = Array.isArray(targets?.targetInfos) ? targets.targetInfos : [];
  for (const target of targetInfos) {
    if (!["page", "iframe", "worker", "shared_worker"].includes(target.type)) {
      continue;
    }
    try {
      const attach = await root.send("Target.attachToTarget", {
        targetId: target.targetId,
        flatten: false,
      });
      attached.set(attach.sessionId, {
        targetId: target.targetId,
        type: target.type,
        url: target.url,
        title: target.title,
      });
      await sendToTarget(attach.sessionId, "Network.enable");
    } catch {
      // Some transient targets do not accept attach/network enable; ignore them.
    }
  }

  return {
    readCapture() {
      return captures.at(-1) ?? null;
    },
    async dispose() {
      root.off("Target.receivedMessageFromTarget", onReceivedMessage);
      for (const sessionId of attached.keys()) {
        await root.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
      await root.detach().catch(() => undefined);
    },
  };
}

async function installGenerateHook(page) {
  await page.evaluate(() => {
    if (window.__udioGenerateHookInstalled) {
      return;
    }

    const isGenerateProxySubmit = (rawUrl, method) => {
      const normalizedMethod = String(method || "").trim().toUpperCase();
      if (normalizedMethod !== "POST") {
        return false;
      }
      try {
        return new URL(String(rawUrl || ""), window.location.origin).pathname === "/api/generate-proxy";
      } catch {
        return false;
      }
    };

    const makeEntry = (kind, url, method, body) => ({
      kind,
      url,
      method,
      body,
      ts: Date.now(),
    });
    const storeCapture = (patch) => {
      const current = window.__udioGenerateCapture || {};
      window.__udioGenerateCapture = {
        ...current,
        ...patch,
      };
    };

    window.__udioGenerateHookInstalled = true;
    window.__udioGenerateCapture = null;

    const originalFetch = window.fetch.bind(window);
    window.fetch = async (...args) => {
      const request = args[0];
      const init = args[1] || {};
      const url = typeof request === "string" ? request : request?.url || "";
      const method =
        init.method ||
        (typeof request === "object" && request?.method) ||
        "GET";
      const body =
        init.body ||
        (typeof request === "object" && request?.body) ||
        null;

      if (isGenerateProxySubmit(url, method)) {
        storeCapture({
          request: makeEntry("fetch", String(url), String(method).toUpperCase(), body),
        });
      }

      const response = await originalFetch(...args);
      if (isGenerateProxySubmit(url, method)) {
        const responseText = await response.clone().text().catch(() => null);
        storeCapture({
          request: makeEntry("fetch", String(url), String(method).toUpperCase(), body),
          response: {
            status: response.status,
            ok: response.ok,
            body: responseText,
            ts: Date.now(),
          },
        });
      }
      return response;
    };

    const originalOpen = XMLHttpRequest.prototype.open;
    const originalSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function(method, url, ...rest) {
      this.__udioCaptureUrl = url;
      this.__udioCaptureMethod = method;
      return originalOpen.call(this, method, url, ...rest);
    };
    XMLHttpRequest.prototype.send = function(body) {
      const url = String(this.__udioCaptureUrl || "");
      const method = String(this.__udioCaptureMethod || "GET").toUpperCase();
      if (isGenerateProxySubmit(url, method)) {
        storeCapture({
          request: makeEntry("xhr", url, method, body ?? null),
        });
        this.addEventListener(
          "loadend",
          () => {
            storeCapture({
              request: makeEntry("xhr", url, method, body ?? null),
              response: {
                status: Number(this.status || 0),
                ok: Number(this.status || 0) >= 200 && Number(this.status || 0) < 300,
                body: typeof this.responseText === "string" ? this.responseText : null,
                ts: Date.now(),
              },
            });
          },
          { once: true },
        );
      }
      return originalSend.call(this, body);
    };
  });
}

async function readHookCapture(page) {
  return await page.evaluate(() => {
    return window.__udioGenerateCapture || null;
  });
}

async function main() {
  const input = await resolveInput();
  resolvedInputCache = input;
  const browserCdpUrl = normalizeString(input.browserCdpUrl);
  if (!browserCdpUrl) {
    throw new Error("browserCdpUrl is required.");
  }
  const browserCdpTargetUrl =
    normalizeString(input.browserCdpTargetUrl) ?? "https://www.udio.com/create";
  const outputPath =
    normalizeString(input.outputPath) ??
    path.resolve(process.cwd(), ".runtime/udio-next-generate-capture.json");
  const readyPath = normalizeString(input.readyPath);
  const statusPath = normalizeString(input.statusPath);
  const timeoutMs = normalizeTimeoutMs(input.timeoutMs, DEFAULT_TIMEOUT_MS, 5_000, 20 * 60 * 1000);
  const cdpConnectTimeoutMs = normalizeTimeoutMs(
    input.browserCdpConnectTimeoutMs,
    300_000,
    5_000,
    10 * 60 * 1000,
  );

  await writeOptionalJson(statusPath, {
    ok: true,
    phase: "connecting",
    pid: process.pid,
    browserCdpUrl,
    browserCdpTargetUrl,
    outputPath,
    updatedAt: new Date().toISOString(),
  });

  const browser = await chromium.connectOverCDP(browserCdpUrl, {
    timeout: cdpConnectTimeoutMs,
  });
  const browserTargetMonitor = await createBrowserTargetMonitor(browser).catch(() => null);
  try {
    const context = browser.contexts()[0];
    const page =
      context.pages().find((entry) => entry.url().startsWith(browserCdpTargetUrl)) ??
      context.pages()[0];
    if (!page) {
      throw new Error("No Udio page found.");
    }
    await page.bringToFront().catch(() => undefined);
    await installGenerateHook(page).catch(() => undefined);
    const pageSnapshot = {
      url: page.url(),
      title: await page.title().catch(() => null),
    };

    const result = await new Promise((resolve, reject) => {
      let settled = false;
      const timer = setTimeout(() => {
        reject(new Error("Timed out waiting for the next POST /api/generate-proxy request."));
      }, timeoutMs);
      let pollHook = null;

      const cleanup = () => {
        clearTimeout(timer);
        if (pollHook) {
          clearInterval(pollHook);
        }
        context.off("requestfinished", onRequestFinished);
        context.off("requestfailed", onRequestFailed);
      };

      const resolveOnce = (value) => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        resolve(value);
      };

      const rejectOnce = (error) => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        reject(error);
      };

      const onRequestFinished = async (request) => {
        if (!isGenerateProxySubmit(request.url(), request.method())) {
          return;
        }
        try {
          let requestBody = null;
          try {
            requestBody = request.postDataJSON();
          } catch {
            requestBody = request.postData() ?? null;
          }
          const response = await request.response();
          const responseText = await response?.text().catch(() => null);
          resolveOnce({
            ok: true,
            capturedAt: new Date().toISOString(),
            request: {
              url: request.url(),
              method: request.method(),
              headers: request.headers(),
              body: requestBody,
            },
            response: response
              ? {
                  status: response.status(),
                  headers: await response.allHeaders().catch(() => ({})),
                  body: responseText,
                }
              : null,
          });
        } catch (error) {
          rejectOnce(error);
        }
      };

      const onRequestFailed = (request) => {
        if (!isGenerateProxySubmit(request.url(), request.method())) {
          return;
        }
        rejectOnce(new Error(`Generate request failed: ${request.failure()?.errorText ?? "unknown"}`));
      };

      context.on("requestfinished", onRequestFinished);
      context.on("requestfailed", onRequestFailed);

      const armedState = {
        ok: true,
        phase: "armed",
        pid: process.pid,
        browserCdpUrl,
        browserCdpTargetUrl,
        outputPath,
        page: pageSnapshot,
        timeoutMs,
        updatedAt: new Date().toISOString(),
      };
      void writeOptionalJson(readyPath, armedState);
      void writeOptionalJson(statusPath, armedState);
      process.stdout.write(`${JSON.stringify(armedState)}\n`);

      pollHook = setInterval(async () => {
        try {
          const hooked = await readHookCapture(page);
          const targetCapture = browserTargetMonitor?.readCapture?.() ?? null;
          if (targetCapture?.request) {
            resolveOnce({
              ok: true,
              capturedAt: new Date().toISOString(),
              request: targetCapture.request,
              response: targetCapture.response,
            });
            return;
          }
          if (
            hooked?.response &&
            isGenerateProxySubmit(hooked.request?.url, hooked.request?.method)
          ) {
            resolveOnce({
              ok: true,
              capturedAt: new Date().toISOString(),
              request: {
                url: hooked.request?.url ?? null,
                method: hooked.request?.method ?? null,
                headers: {},
                body: hooked.request?.body ?? null,
                source: hooked.request?.kind ?? "page_hook",
              },
              response: {
                status: hooked.response?.status ?? null,
                headers: {},
                body: hooked.response?.body ?? null,
              },
            });
          }
        } catch {
          // Ignore transient page-evaluate failures while the attached browser is busy.
        }
      }, 500);
    });

    await writeJson(outputPath, result);
    await writeOptionalJson(statusPath, {
      ok: true,
      phase: "captured",
      pid: process.pid,
      browserCdpUrl,
      browserCdpTargetUrl,
      outputPath,
      capturedAt: result.capturedAt,
      updatedAt: new Date().toISOString(),
    });
    process.stdout.write(JSON.stringify({ ok: true, phase: "captured", outputPath }));
  } finally {
    await browserTargetMonitor?.dispose?.().catch(() => undefined);
    await browser.close().catch(() => undefined);
  }
}

main().catch(async (error) => {
  const input = resolvedInputCache ?? (await resolveInput().catch(() => ({})));
  await writeOptionalJson(input.statusPath, {
    ok: false,
    phase: "failed",
    pid: process.pid,
    browserCdpUrl: normalizeString(input.browserCdpUrl),
    browserCdpTargetUrl: normalizeString(input.browserCdpTargetUrl),
    outputPath: normalizeString(input.outputPath),
    message: error?.message ?? String(error),
    updatedAt: new Date().toISOString(),
  }).catch(() => undefined);
  process.stdout.write(
    JSON.stringify({
      ok: false,
      phase: "failed",
      message: error?.message ?? String(error),
    }),
  );
  process.exit(1);
});
