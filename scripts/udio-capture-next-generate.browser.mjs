import { isGenerateProxySubmit, withTimeout } from "./udio-capture-next-generate.input.mjs";

let nextCdpCommandId = 1_000;

export async function createBrowserTargetMonitor(browser) {
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

export async function installGenerateHook(page) {
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

export async function readHookCapture(page) {
  return await page.evaluate(() => {
    return window.__udioGenerateCapture || null;
  });
}
