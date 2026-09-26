import { normalizeString, normalizeObject } from "./gemini-canvas-browser-pool-input.mjs";
import { sanitizeCanvasProxyHeaders, sanitizeBrowserFetchHeaders } from "./gemini-canvas-browser-pool-headers.mjs";
import { findAttachedGeminiAppPage } from "./gemini-canvas-browser-pool-app.mjs";
import { downloadBinaryViaNavigation } from "./gemini-canvas-browser-pool-payload.mjs";
import {
  assertTextWithinLimit,
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
  readFetchResponseBody,
} from "./gemini-canvas-browser-pool-body.mjs";

export function isBrowserDownloadAssetUrl(url) {
  return (
    typeof url === "string" &&
    /gg-dl|rd-gg-dl|googleusercontent|usercontent\.google\.com|work\.fife\.usercontent\.google\.com/i.test(url)
  );
}

export function shouldAttemptConnectedClientFetchFallback(error, options = {}) {
  const message = error instanceof Error ? error.message : String(error ?? "");
  const method = normalizeString(options.method)?.toUpperCase() ?? "GET";
  return (
    options.useCanvasProxyMode !== true &&
    method === "POST" &&
    /Failed to fetch/i.test(message)
  );
}

export function createFetchExecutionOwner({
  DEFAULT_TIMEOUT_MS,
  buildGoogleFetchAuthHeaders,
  resolveProgramPageUrl,
  startNetworkCapture,
  runFetchPreviewOperation,
  runFetchPageMusicOperation,
  isFixtureCanvasBaseUrl,
  ensureProgramPage,
  buildProgramHandleState,
  ensureSharePage,
  ensureLoopbackConnectedClient,
  listConnectedClients,
  dispatchConnectedProxyRequest,
  runFetchPageRequest,
  log,
}) {
  async function runFetchOperation(entry, args) {
    const fetchRequest = normalizeObject(args.fetchRequest);
    const url = normalizeString(fetchRequest.url);
    if (!url) {
      throw Object.assign(
        new Error("Gemini Canvas browser fetch requires fetchRequest.url."),
        {
          status: 400,
          code: "gemini_canvas_invalid_fetch_request",
        },
      );
    }

    const method = normalizeString(fetchRequest.method)?.toUpperCase() ?? "GET";
    const useCanvasProxyMode = normalizeString(args.googleFetchMode) === "canvas_proxy";
    const useCanvasPreviewNoKeyMode =
      normalizeString(args.googleFetchMode) === "canvas_preview_no_key";
    const useCanvasPreviewMusicNoKeyMode =
      normalizeString(args.googleFetchMode) === "canvas_preview_music_no_key";
    const useCanvasPageNoKeyMode =
      normalizeString(args.googleFetchMode) === "canvas_page_no_key";
    const useCanvasPageMusicNoKeyMode =
      normalizeString(args.googleFetchMode) === "canvas_page_music_no_key";
    const fixtureBaseUrl = normalizeString(args.baseUrl);
    const pageNoKeyAuthHeaders = useCanvasPageNoKeyMode
      ? await buildGoogleFetchAuthHeaders(
          entry,
          fixtureBaseUrl ?? "https://gemini.google.com",
          url,
        )
      : {};
    for (const headerName of Object.keys(pageNoKeyAuthHeaders)) {
      if (headerName.toLowerCase() === "x-origin") {
        delete pageNoKeyAuthHeaders[headerName];
      }
    }
    if (useCanvasPageNoKeyMode) {
      log(
        "canvas page no-key auth headers prepared",
        JSON.stringify({
          hasAuthorization: Object.keys(pageNoKeyAuthHeaders).some(
            (name) => name.toLowerCase() === "authorization",
          ),
          hasAuthUser: Object.keys(pageNoKeyAuthHeaders).some(
            (name) => name.toLowerCase() === "x-goog-authuser",
          ),
          hasXOrigin: Object.keys(pageNoKeyAuthHeaders).some(
            (name) => name.toLowerCase() === "x-origin",
          ),
        }),
      );
    }
    const requestedHeaders = useCanvasProxyMode
      ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
      : useCanvasPreviewNoKeyMode
      ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
      : useCanvasPreviewMusicNoKeyMode
      ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
      : useCanvasPageNoKeyMode
      ? sanitizeCanvasProxyHeaders({
          ...normalizeObject(fetchRequest.headers),
          ...pageNoKeyAuthHeaders,
        })
      : useCanvasPageMusicNoKeyMode
      ? sanitizeCanvasProxyHeaders(fetchRequest.headers)
      : sanitizeBrowserFetchHeaders({
          ...normalizeObject(fetchRequest.headers),
          ...(await buildGoogleFetchAuthHeaders(
            entry,
            normalizeString(args.baseUrl) ?? "https://gemini.google.com",
            url,
          )),
        });
    const timeoutMs = Math.max(
      Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
      DEFAULT_TIMEOUT_MS,
    );
    const requireSharePage = args.requireSharePage === true;
    const preferredProgramPageUrl = resolveProgramPageUrl(
      fixtureBaseUrl ?? "https://gemini.google.com",
      args,
    );
    const attachedAppPage =
      useCanvasPreviewNoKeyMode && preferredProgramPageUrl
        ? null
        : await findAttachedGeminiAppPage(
            entry,
            fixtureBaseUrl ?? "https://gemini.google.com",
          );
    const originalPage = entry.page;
    if (attachedAppPage && attachedAppPage !== entry.page) {
      await attachedAppPage.bringToFront().catch(() => undefined);
      entry.page = attachedAppPage;
      log(
        "reusing attached Gemini app page for fetch operation",
        JSON.stringify({
          pageUrl: normalizeString(attachedAppPage.url()) ?? "",
          runtimeStatePath: entry.runtimeStatePath,
        }),
      );
    }
    const capture = startNetworkCapture(entry.page, "text");
    let connectedClientBootstrapError = null;
    try {
    await capture.ready;
    const requestBodyText =
      typeof fetchRequest.bodyText === "string"
        ? fetchRequest.bodyText
        : fetchRequest.jsonBody !== undefined
          ? JSON.stringify(fetchRequest.jsonBody)
          : null;
    const requestReferrer =
      preferredProgramPageUrl ||
      (typeof fetchRequest.referrer === "string" && fetchRequest.referrer.trim()
        ? fetchRequest.referrer.trim()
        : null);
    const requestReferrerPolicy =
      typeof fetchRequest.referrerPolicy === "string" && fetchRequest.referrerPolicy.trim()
        ? fetchRequest.referrerPolicy.trim()
        : null;
    const connectedRequestSpec = {
      method,
      url,
      headers: requestedHeaders,
      body: requestBodyText,
      referrer: requestReferrer,
      referrerPolicy: requestReferrerPolicy,
    };
    if (useCanvasPreviewNoKeyMode || useCanvasPreviewMusicNoKeyMode) {
      return await runFetchPreviewOperation({
        entry, args, url, method, requestedHeaders, requestBodyText,
        timeoutMs, preferredProgramPageUrl, fixtureBaseUrl,
        useCanvasPreviewMusicNoKeyMode, fetchRequest, capture,
      });
    }

    if (preferredProgramPageUrl && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      await ensureProgramPage(
        entry,
        fixtureBaseUrl ?? "https://gemini.google.com",
        preferredProgramPageUrl,
        timeoutMs,
      );
    }

    if (useCanvasPageMusicNoKeyMode) {
      return await runFetchPageMusicOperation({
        entry, args, url, fetchRequest, timeoutMs, fixtureBaseUrl, capture,
      });
    }

    if ((useCanvasProxyMode || requireSharePage) && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      log(
        useCanvasProxyMode
          ? "fetch operation using canvas proxy mode"
          : "fetch operation preparing Gemini share-page context",
        url,
      );
      await ensureSharePage(
        entry,
        fixtureBaseUrl ?? "https://gemini.google.com",
        normalizeString(args.shareId),
        timeoutMs,
      );
    }

    if (preferredProgramPageUrl && !isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      await ensureProgramPage(
        entry,
        fixtureBaseUrl ?? "https://gemini.google.com",
        preferredProgramPageUrl,
        timeoutMs,
      );
    }

    if (useCanvasProxyMode && method === "GET" && isBrowserDownloadAssetUrl(url)) {
      const navigationResult = await downloadBinaryViaNavigation(entry, url, timeoutMs);
      return {
        operation: "fetch",
        ...buildProgramHandleState(
          fixtureBaseUrl ?? "https://gemini.google.com",
          args,
          navigationResult.finalUrl ?? entry.page.url(),
          capture.state,
        ),
        ...navigationResult,
        networkEvents: capture.state.events,
        rpcCaptures: capture.state.rpcCaptures,
      };
    }

    if (useCanvasProxyMode) {
      try {
        await ensureLoopbackConnectedClient(entry);
      } catch (error) {
        connectedClientBootstrapError = error;
        log(
          "failed to bootstrap Gemini Canvas loopback connected client",
          error instanceof Error ? error.message : String(error),
        );
      }
    }

    if (useCanvasProxyMode && listConnectedClients().length > 0) {
      try {
        const connectedResult = await dispatchConnectedProxyRequest({
          method,
          url,
          headers: requestedHeaders,
          ...connectedRequestSpec,
        });
        return {
          operation: "fetch",
          ...buildProgramHandleState(
            fixtureBaseUrl ?? "https://gemini.google.com",
            args,
            entry.page.url(),
            capture.state,
          ),
          status: connectedResult.status,
          ok: connectedResult.status >= 200 && connectedResult.status < 300,
          finalUrl: url,
          contentType:
            connectedResult.headers["content-type"] ??
            connectedResult.headers["Content-Type"] ??
            null,
          headers: connectedResult.headers,
          bodyText: connectedResult.bodyText,
          bodyBase64: Buffer.from(connectedResult.bodyText ?? "", "utf8").toString("base64"),
          networkEvents: capture.state.events,
          rpcCaptures: capture.state.rpcCaptures,
        };
      } catch (error) {
        if (error?.status === 413) {
          throw error;
        }
        connectedClientBootstrapError = error;
        log(
          "Gemini Canvas connected client fetch failed; trying browser navigation fallback",
          error instanceof Error ? error.message : String(error),
        );
      }
    }

    if (useCanvasProxyMode) {
      throw Object.assign(
        new Error(
          connectedClientBootstrapError instanceof Error
            ? `Gemini Canvas program websocket path is unavailable: ${connectedClientBootstrapError.message}`
            : "Gemini Canvas program websocket path is unavailable because no connected client is attached.",
        ),
        {
          status: 503,
          code:
            connectedClientBootstrapError instanceof Error
              ? "gemini_canvas_program_ws_bootstrap_failed"
              : "gemini_canvas_program_ws_client_unavailable",
          bodyText:
            connectedClientBootstrapError instanceof Error
              ? connectedClientBootstrapError.message
              : null,
        },
      );
    }

    if (args.requireAppPage === false && isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      const response = await fetch(url, {
        method,
        headers: requestedHeaders,
            body: requestBodyText ?? undefined,
        referrer: useCanvasProxyMode
          ? undefined
          : requestReferrer
            ? requestReferrer
            : undefined,
        referrerPolicy: useCanvasProxyMode
          ? undefined
          : requestReferrerPolicy
            ? requestReferrerPolicy
            : undefined,
      });
      const bodyBuffer = Buffer.from(
        await readFetchResponseBody(
          response,
          BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
          "fixture fetch",
        ),
      );
      const responseHeaders = {};
      response.headers.forEach((value, key) => {
        responseHeaders[key] = value;
      });
      const contentType = response.headers.get("content-type");
      const bodyTextResult =
        contentType && /(json|text|javascript|xml|html)/i.test(contentType)
          ? assertTextWithinLimit(
              bodyBuffer.toString("utf8"),
              BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
              "fixture fetch text",
            )
          : null;
      return {
        operation: "fetch",
        ...buildProgramHandleState(
          fixtureBaseUrl ?? "https://gemini.google.com",
          args,
          response.url,
          capture.state,
        ),
        status: response.status,
        ok: response.ok,
        finalUrl: response.url,
        contentType,
        headers: responseHeaders,
        bodyText: bodyTextResult,
        bodyBase64: bodyBuffer.toString("base64"),
        networkEvents: capture.state.events,
        rpcCaptures: capture.state.rpcCaptures,
      };
    }

    const pageFetchInput = {
      url,
      method,
      headers: requestedHeaders,
      bodyText: requestBodyText,
      referrer: requestReferrer,
      referrerPolicy: requestReferrerPolicy,
      timeoutMs,
      useCanvasProxyMode,
      maxBodyBytes: BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
      maxTextBytes: BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
    };
    const evaluatePageFetch = () => runFetchPageRequest(entry.page, pageFetchInput);
    let result;
    try {
      result = await evaluatePageFetch();
    } catch (error) {
      if (
        useCanvasPageNoKeyMode &&
        /Execution context was destroyed|frame was detached|navigation/i.test(
          error instanceof Error ? error.message : String(error),
        )
      ) {
        await entry.page.waitForTimeout(1200).catch(() => undefined);
        result = await evaluatePageFetch();
      } else if (shouldAttemptConnectedClientFetchFallback(error, { method, useCanvasProxyMode })) {
        try {
          if (listConnectedClients().length === 0) {
            await ensureLoopbackConnectedClient(entry);
          }
        } catch (connectedClientError) {
          connectedClientBootstrapError = connectedClientError;
          log(
            "failed to bootstrap Gemini Canvas loopback connected client after page.evaluate fetch error",
            connectedClientError instanceof Error ? connectedClientError.message : String(connectedClientError),
          );
        }
        if (listConnectedClients().length > 0) {
          log(
            "page.evaluate fetch failed; retrying Gemini Canvas fetch through connected client fallback",
            url,
          );
          const connectedResult = await dispatchConnectedProxyRequest(connectedRequestSpec);
          return {
            operation: "fetch",
            ...buildProgramHandleState(
              fixtureBaseUrl ?? "https://gemini.google.com",
              args,
              entry.page.url(),
              capture.state,
            ),
            status: connectedResult.status,
            ok: connectedResult.status >= 200 && connectedResult.status < 300,
            finalUrl: url,
            contentType:
              connectedResult.headers["content-type"] ??
              connectedResult.headers["Content-Type"] ??
              null,
            headers: connectedResult.headers,
            bodyText: connectedResult.bodyText,
            bodyBase64: Buffer.from(connectedResult.bodyText ?? "", "utf8").toString("base64"),
            networkEvents: capture.state.events,
            rpcCaptures: capture.state.rpcCaptures,
          };
        }
      } else {
        throw error;
      }
    }

    return {
      operation: "fetch",
      ...buildProgramHandleState(
        fixtureBaseUrl ?? "https://gemini.google.com",
        args,
        result.finalUrl ?? entry.page.url(),
        capture.state,
      ),
      ...result,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
    } finally {
      entry.page = originalPage;
      await capture.stop();
    }
  }

  return { runFetchOperation };
}
