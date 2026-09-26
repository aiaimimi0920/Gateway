async function sendActiveTrigger(page, capture) {
  try {
    await page.evaluate(async () => {
      try {
        await fetch("https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger", {
          method: "GET",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
        });
      } catch (error) {
        console.log("[AIStudioProbe] Active trigger sent");
      }
    });
    capture.autoActions.push({ action: "active-trigger", ok: true });
  } catch (error) {
    capture.autoActions.push({
      action: "active-trigger",
      ok: false,
      error: String(error),
    });
  }
}

function countOpenLocalWebSocketConnections(capture) {
  return (
    capture.localWebSocket?.connections?.filter((entry) => !entry?.closedAt).length ?? 0
  );
}

function countOpenUndispatchedLocalProxyConnections(capture) {
  return (
    capture.localWebSocket?.connections?.filter(
      (entry) =>
        !entry?.closedAt &&
        !(Array.isArray(entry?.sentEventTypes) && entry.sentEventTypes.includes("proxy_request")),
    ).length ?? 0
  );
}

function hasPendingLocalProxyResponse(capture) {
  return (
    capture.localWebSocket?.connections?.some(
      (entry) =>
        Array.isArray(entry?.sentEventTypes) &&
        entry.sentEventTypes.includes("proxy_request") &&
        !entry?.closedAt &&
        (!Array.isArray(entry?.receivedEventTypes) || entry.receivedEventTypes.length <= 0),
    ) ?? false
  );
}

async function waitForLocalWebSocketConnection(capture, timeoutMs = 15_000, options = {}) {
  const requireUndispatched = options?.requireUndispatched === true;
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const connectionCount = requireUndispatched
      ? countOpenUndispatchedLocalProxyConnections(capture)
      : countOpenLocalWebSocketConnections(capture);
    if (connectionCount > 0) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  return requireUndispatched
    ? countOpenUndispatchedLocalProxyConnections(capture) > 0
    : countOpenLocalWebSocketConnections(capture) > 0;
}

async function waitForRunAppBootstrap(page, timeoutMs = 15_000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    for (const frame of page.frames()) {
      try {
        const hasBootstrap = await frame.evaluate(() => {
          if (!Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)) {
            return false;
          }
          return window.__AISTUDIO_LIVE_CAPTURE__.some(
            (event) =>
              event?.kind === "window.message" &&
              typeof event.messagePreview === "string" &&
              event.messagePreview.includes("\"type\":\"bootstrap\""),
          );
        });
        if (hasBootstrap) {
          return {
            ok: true,
            frameUrl: frame.url(),
          };
        }
      } catch (_) {}
    }
    await page.waitForTimeout(250);
  }
  return { ok: false, frameUrl: null };
}

async function maybeDispatchLocalProxyMessages(
  page,
  capture,
  localWebSocketServer,
  localProxyMessages,
  localProxyDelayMs,
  stageLabel,
  persistCapture,
) {
  if (!localWebSocketServer || localProxyMessages.length === 0) {
    return false;
  }

  if (countOpenUndispatchedLocalProxyConnections(capture) <= 0) {
    return false;
  }

  const hasLocalConnection = await waitForLocalWebSocketConnection(capture, 20_000, {
    requireUndispatched: true,
  });
  capture.autoActions.push({
    action: `wait-local-ws-connection-${stageLabel}`,
    ok: hasLocalConnection,
    connectionCount: countOpenLocalWebSocketConnections(capture),
    undispatchedConnectionCount: countOpenUndispatchedLocalProxyConnections(capture),
  });
  const bootstrapResult = await waitForRunAppBootstrap(page, 20_000);
  capture.autoActions.push({
    action: `wait-runapp-bootstrap-${stageLabel}`,
    ok: bootstrapResult.ok,
    frameUrl: bootstrapResult.frameUrl,
  });
  if (!hasLocalConnection) {
    return false;
  }

  await page.waitForTimeout(localProxyDelayMs);
  const dispatchResult = await localWebSocketServer.sendMessages(localProxyMessages, {
    dispatchKey: "local-proxy-request",
    onlyUndispatched: true,
  });
  if ((dispatchResult?.sentConnectionCount ?? 0) <= 0) {
    return false;
  }
  capture.autoActions.push({
    action: `send-local-proxy-request-${stageLabel}`,
    ok: true,
    sentConnectionCount: dispatchResult.sentConnectionCount,
    sentEventTypes: localProxyMessages.map((entry) => entry?.event_type).filter(Boolean),
  });
  await persistCapture();
  return true;
}

async function probeRunAppFrameFetch(page, requestSpec) {
  const runAppFrame = page.frames().find((frame) => frame.url().includes("run.app"));
  if (!runAppFrame) {
    return {
      ok: false,
      error: "run_app_frame_not_found",
    };
  }

  try {
    return await runAppFrame.evaluate(async (spec) => {
      const fetchPromise = (async () => {
        try {
          const response = await fetch(spec.url, {
            method: spec.method || "GET",
            headers: spec.headers || {},
            body: typeof spec.body === "string" ? spec.body : undefined,
            credentials: spec.credentials || "omit",
          });
          const text = await response.text().catch(() => "");
          return {
            ok: response.ok,
            status: response.status,
            finalUrl: response.url,
            contentType: response.headers.get("content-type"),
            bodyPreview: text.slice(0, 2000),
          };
        } catch (error) {
          return {
            ok: false,
            error: String(error),
          };
        }
      })();

      const timeoutPromise = new Promise((resolve) => {
        setTimeout(
          () =>
            resolve({
              ok: false,
              error: "run_app_probe_timeout",
            }),
          12_000,
        );
      });

      return await Promise.race([fetchPromise, timeoutPromise]);
    }, requestSpec);
  } catch (error) {
    return {
      ok: false,
      error: String(error),
    };
  }
}

export { sendActiveTrigger, countOpenUndispatchedLocalProxyConnections, hasPendingLocalProxyResponse, maybeDispatchLocalProxyMessages, probeRunAppFrameFetch };
