import path from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
import { extractCanvasProxyClientHtmlFromTexts, injectForcedCanvasProxyAuthIndex } from "./gemini-canvas-browser-pool-proxy-html.mjs";
import { BROWSER_POOL_TEXT_BODY_LIMIT_BYTES, readPlaywrightResponseText } from "./gemini-canvas-browser-pool-body.mjs";
import { createProgramResponseCapture } from "./gemini-canvas-program-handle-response-capture.mjs";

export function createProxyLaunchOwner({ collectCanvasProxyContractTexts, inferGoogleAuthUser, log, createResponseCapture = createProgramResponseCapture }) {
  async function tryLaunchCanvasProxyClientFromCapturedHtml(
    context,
    sourcePage,
    snapshot,
    captureState,
    timeoutMs,
  ) {
    const decodedHtml = extractCanvasProxyClientHtmlFromTexts(
      collectCanvasProxyContractTexts(snapshot, captureState),
    );
    if (!decodedHtml) {
      return {
        launched: false,
        reason: "canvas_proxy_client_html_missing",
      };
    }
    const authIndex = inferGoogleAuthUser(sourcePage?.url?.());
    const page = await context.newPage();
    const consoleEvents = [];
    const networkEvents = [];
    let responseCapture = null, stopped = false;
    const onConsole = (message) => {
      const entry = {
        type: message.type(),
        text: message.text(),
      };
      consoleEvents.push(entry);
      if (consoleEvents.length > 60) {
        consoleEvents.shift();
      }
      log("canvas proxy direct launch console", JSON.stringify(entry));
    };
    const onPageError = (error) => {
      const entry = {
        type: "pageerror",
        text: error instanceof Error ? error.message : String(error),
      };
      consoleEvents.push(entry);
      if (consoleEvents.length > 60) {
        consoleEvents.shift();
      }
      log("canvas proxy direct launch pageerror", JSON.stringify(entry));
    };
    const pushNetworkEvent = (event) => {
      if (stopped) return;
      networkEvents.push(event);
      if (networkEvents.length > 80) {
        networkEvents.shift();
      }
      log("canvas proxy direct launch network", JSON.stringify(event));
    };
    const shouldCaptureDirectLaunchNetwork = (url) =>
      /generativelanguage\.googleapis\.com|127\.0\.0\.1:9998/i.test(String(url || ""));
    const onRequest = (request) => {
      const url = request.url();
      if (!shouldCaptureDirectLaunchNetwork(url)) {
        return;
      }
      pushNetworkEvent({
        type: "request",
        method: request.method(),
        url,
        headers: Object.fromEntries(
          Object.entries(request.headers() || {}).filter(([key]) =>
            ["content-type", "origin", "referer", "cookie"].includes(String(key || "").toLowerCase()),
          ),
        ),
        postData:
          typeof request.postData() === "string"
            ? request.postData().slice(0, 2000)
            : null,
      });
    };
    const onResponse = async (response) => {
      const url = response.url();
      if (!shouldCaptureDirectLaunchNetwork(url)) {
        return;
      }
      let bodyPreview = null;
      try {
        if ((response.headers()["content-type"] || "").includes("json")) {
          bodyPreview = (
            await readPlaywrightResponseText(
              response,
              BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
              "proxy launch response",
            )
          ).slice(0, 2000);
        }
      } catch {
        bodyPreview = null;
      }
      pushNetworkEvent({
        type: "response",
        url,
        status: response.status(),
        headers: Object.fromEntries(
          Object.entries(response.headers() || {}).filter(([key]) =>
            ["content-type", "access-control-allow-origin", "www-authenticate"].includes(
              String(key || "").toLowerCase(),
            ),
          ),
        ),
        bodyPreview,
      });
    };
    const onRequestFailed = (request) => {
      const url = request.url();
      if (!shouldCaptureDirectLaunchNetwork(url)) {
        return;
      }
      pushNetworkEvent({
        type: "requestfailed",
        method: request.method(),
        url,
        failureText: request.failure()?.errorText ?? null,
      });
    };
    const onRequestFinished = (request) => {
      const url = request.url();
      if (!shouldCaptureDirectLaunchNetwork(url)) {
        return;
      }
      pushNetworkEvent({
        type: "requestfinished",
        method: request.method(),
        url,
      });
    };
    const listeners = [["console", onConsole], ["pageerror", onPageError], ["request", onRequest],
      ["requestfailed", onRequestFailed], ["requestfinished", onRequestFinished]];
    try {
      for (const [event, listener] of listeners) page.on(event, listener);
      responseCapture = createResponseCapture(page, {
        requestFilter: shouldCaptureDirectLaunchNetwork, onResponse, pageOnly: true,
        onError: (error) => pushNetworkEvent({ type: "capture-error", code: error.code, message: error.message }),
      });
      await responseCapture.ready;
      await page.bringToFront().catch(() => undefined);
      const patchedHtml = injectForcedCanvasProxyAuthIndex(decodedHtml, authIndex);
      const artifactDir = path.resolve(
        process.cwd(),
        ".runtime",
        "canvas-proxy-client-direct-launch",
      );
      await mkdir(artifactDir, { recursive: true });
      const artifactPath = path.join(
        artifactDir,
        `launch-${Date.now()}.html`,
      );
      await writeFile(artifactPath, patchedHtml, "utf8");
      await page.addInitScript(
        ({ authIndexValue }) => {
          window.chrome = window.chrome || {};
          window.chrome._contextId = authIndexValue;
          window.__NEURO_FORCED_AUTH_INDEX__ = authIndexValue;
        },
        { authIndexValue: Number(authIndex) || 0 },
      );
      const launchMode = "about_blank_set_content";
      await page.setContent(patchedHtml, {
        waitUntil: "domcontentloaded",
        timeout: Math.min(timeoutMs, 20_000),
      });
      const launchDeadline = Date.now() + Math.min(timeoutMs, 18_000);
      let bodyPreview = "";
      while (Date.now() < launchDeadline) {
        await page.waitForTimeout(800);
        bodyPreview = await page
          .evaluate(() => (document.body?.innerText ?? "").slice(0, 1600))
          .catch(() => "");
        if (
          /System Logs Output|Connecting\.\.\.|Connected|Disconnected|Connection successful/i.test(bodyPreview) ||
          consoleEvents.length > 0
        ) {
          break;
        }
      }
      const runtimeDiagnostics = await page
        .evaluate(() => ({
          url: location.href,
          readyState: document.readyState,
          scriptCount: document.scripts.length,
          chromeContextId: window.chrome?._contextId ?? null,
          forcedAuthIndex: window.__NEURO_FORCED_AUTH_INDEX__ ?? null,
          proxySystemType:
            typeof ProxySystem === "undefined"
              ? "undefined"
              : typeof ProxySystem,
          connectionManagerType:
            typeof ConnectionManager === "undefined"
              ? "undefined"
              : typeof ConnectionManager,
        }))
        .catch((error) => ({
          errorMessage: error instanceof Error ? error.message : String(error),
        }));
      return {
        launched: true,
        reason: launchMode,
        authIndex,
        pageUrl: page.url(),
        bodyPreview: String(bodyPreview || "").slice(0, 800),
        consoleEvents: consoleEvents.slice(-20),
        networkEvents: networkEvents.slice(-40),
        runtimeDiagnostics,
        artifactPath,
        htmlBytes: patchedHtml.length,
        htmlContainsRequestAuthIndex: /requestAuthIndex/.test(patchedHtml),
        htmlContainsProxySystemClass: /class\s+ProxySystem\b/.test(patchedHtml),
        htmlContainsProxySystemInitCall: /\.initialize\(/.test(patchedHtml) || /new\s+ProxySystem\b/.test(patchedHtml),
        htmlContainsSystemInitializingText: /System initializing/.test(patchedHtml),
        page,
      };
    } catch (error) {
      await page.close().catch(() => undefined);
      return {
        launched: false,
        reason: "captured_html_launch_error",
        authIndex,
        errorMessage: error instanceof Error ? error.message : String(error),
        consoleEvents: consoleEvents.slice(-20),
      };
    } finally {
      stopped = true;
      const errors = [];
      for (const [event, listener] of listeners) {
        try { page.off(event, listener); } catch (error) { errors.push(error); }
      }
      try { await responseCapture?.stop(); } catch (error) { errors.push(error); }
      if (errors.length) throw errors[0];
    }
  }

  return { tryLaunchCanvasProxyClientFromCapturedHtml };
}
