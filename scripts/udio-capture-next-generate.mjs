import path from "node:path";
import process from "node:process";
import { chromium } from "playwright-core";
import {
  DEFAULT_TIMEOUT_MS,
  isGenerateProxySubmit,
  normalizeString,
  normalizeTimeoutMs,
  resolveInput,
  writeJson,
  writeOptionalJson,
} from "./udio-capture-next-generate.input.mjs";
import {
  createBrowserTargetMonitor,
  installGenerateHook,
  readHookCapture,
} from "./udio-capture-next-generate.browser.mjs";

let resolvedInputCache = null;

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
