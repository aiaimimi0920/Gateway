import { applyGeminiAccountScope, normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { parseBoolean } from "./gemini-canvas-browser-pool-executable.mjs";

export function createInvocationDispatcher({
  DEFAULT_TIMEOUT_MS,
  log,
  ensureContext,
  closeContext,
  contextHasGeminiAuthCookies,
  syncEmbeddedStorageStateIntoContext,
  syncCookieHeaderIntoContext,
  resolveProgramPageUrl,
  ensureProgramPage,
  ensureAppPage,
  runBootstrapProgramOperation,
  runFetchOperation,
  runTextOperation,
  runTtsOperation,
  runDebugOperation,
  runMediaOperation,
}) {
  async function invokeGeminiCanvas(args) {
    args = applyGeminiAccountScope(args);
    const runtimeStateObjectKey = normalizeString(args.runtimeStateObjectKey);
    const hasFetchRequest = Boolean(args?.fetchRequest);
    log(
      "invokeGeminiCanvas start",
      JSON.stringify({
        runtimeStateObjectKey,
        operation: normalizeString(args?.operation),
        requireAppPage: args?.requireAppPage !== false,
        hasFetchRequest,
        authUser: normalizeString(args?.authUser),
      }),
    );
    if (!runtimeStateObjectKey) {
      return {
        ok: false,
        error: {
          code: "gemini_canvas_invalid_request",
          message: "runtimeStateObjectKey is required.",
          status: 400,
        },
      };
    }

    let entry = null;
    let entryLeaseAcquired = false;
    try {
      entry = await ensureContext({
        runtimeStateObjectKey,
        baseUrl: args.baseUrl,
        locale: args.locale,
        browserExecutablePath: args.browserExecutablePath,
        browserCdpUrl: args.browserCdpUrl,
        timeoutMs: args.timeoutMs,
      });
      if (entry.busy) {
        return {
          ok: false,
          error: {
            code: "gemini_canvas_context_busy",
            message: "Gemini Canvas browser context is busy.",
            status: 429,
          },
        };
      }

      entry.busy = true;
      entryLeaseAcquired = true;
      entry.lastUsedAt = Date.now();
      if (!entry.page || entry.page.isClosed()) {
        entry.page = await entry.context.newPage();
        await entry.page.bringToFront().catch(() => undefined);
        log("recreated closed Gemini Canvas shared page", runtimeStateObjectKey);
      }
      const cookieSyncBaseUrl = normalizeString(args.baseUrl) ?? "https://gemini.google.com";
      const runtimeHasAuthCookies = await contextHasGeminiAuthCookies(entry.context, cookieSyncBaseUrl);
      const synchronizedEmbeddedStorageStateCookieCount =
        entry.runtimeStateMode === "profile_dir"
          ? await syncEmbeddedStorageStateIntoContext(entry.context, entry.runtimeStatePath).catch(
              (error) => {
                log(
                  "embedded storage-state cookie sync failed",
                  runtimeStateObjectKey,
                  error instanceof Error ? error.message : String(error),
                );
                return 0;
              },
            )
          : 0;
      const runtimeHasAuthCookiesAfterEmbeddedSync =
        synchronizedEmbeddedStorageStateCookieCount > 0
          ? await contextHasGeminiAuthCookies(entry.context, cookieSyncBaseUrl)
          : runtimeHasAuthCookies;
      const defaultCookieSyncEnabled =
        entry.runtimeStateMode === "storage_state_file" ||
        entry.launchClonedProfile === true ||
        runtimeHasAuthCookiesAfterEmbeddedSync === false;
      const cookieSyncEnabled = parseBoolean(
        args.forceCookieSync ?? process.env.GEMINI_CANVAS_BROWSER_FORCE_COOKIE_SYNC,
        defaultCookieSyncEnabled,
      );
      const runtimeAlreadyHasAuthCookies =
        cookieSyncEnabled &&
        entry.runtimeStateMode !== "storage_state_file" &&
        entry.launchClonedProfile !== true
          ? runtimeHasAuthCookiesAfterEmbeddedSync
          : false;
      const synchronizedCookieCount =
        cookieSyncEnabled && !runtimeAlreadyHasAuthCookies
          ? await syncCookieHeaderIntoContext(entry.context, args.cookieHeader, cookieSyncBaseUrl).catch(
              (error) => {
                log(
                  "cookie sync failed",
                  runtimeStateObjectKey,
                  error instanceof Error ? error.message : String(error),
                );
                return 0;
              },
            )
          : 0;
      if (!cookieSyncEnabled) {
        log("skipped cookie sync because runtime mirroring is authoritative", runtimeStateObjectKey);
      } else if (runtimeAlreadyHasAuthCookies) {
        log("skipped cookie sync because runtime already has Gemini auth cookies", runtimeStateObjectKey);
      }
      if (synchronizedEmbeddedStorageStateCookieCount > 0) {
        log(
          "synced Gemini auth cookies from embedded storage-state into browser context",
          runtimeStateObjectKey,
          synchronizedEmbeddedStorageStateCookieCount,
        );
      }
      if (synchronizedCookieCount > 0) {
        log("synced Gemini auth cookies into browser context", runtimeStateObjectKey, synchronizedCookieCount);
      }
      const operation = normalizeString(args.operation);
      if (operation === "bootstrap_program") {
        const result = await runBootstrapProgramOperation(entry, args);
        return {
          ok: true,
          result,
        };
      }
      const preferredProgramPageUrl = resolveProgramPageUrl(
        normalizeString(args.baseUrl) ?? "https://gemini.google.com",
        args,
      );
      if (args.enforceProgramOwner === true && !preferredProgramPageUrl) {
        return {
          ok: false,
          error: {
            code: "gemini_canvas_program_handle_required",
            message:
              "Gemini Canvas program-owned relay requires a concrete canvasProgramUrl/appPath/conversationId handle.",
            status: 400,
          },
        };
      }
      const shouldDeferAppPageEnsureToFetchOperation =
        args.requireAppPage !== false && hasFetchRequest && entry.attachedCdp === true;
      if (shouldDeferAppPageEnsureToFetchOperation) {
        log(
          "deferring outer ensureAppPage to fetch operation for attached CDP request",
          JSON.stringify({
            runtimeStateObjectKey,
            browserCdpUrl: normalizeString(args.browserCdpUrl) ?? null,
          }),
        );
      } else if (args.requireAppPage !== false) {
        if (preferredProgramPageUrl) {
          await ensureProgramPage(
            entry,
            normalizeString(args.baseUrl) ?? "https://gemini.google.com",
            preferredProgramPageUrl,
            Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
          );
        } else {
          await ensureAppPage(
            entry,
            normalizeString(args.baseUrl) ?? "https://gemini.google.com",
            Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
            {
              cookieHeader: args.cookieHeader,
              skipInitialNavigationWhenAppSurfaceReady: entry.attachedCdp === true,
            },
          );
        }
      }
      const result = hasFetchRequest
        ? await runFetchOperation(entry, args)
        : operation === "text"
          ? await runTextOperation(entry, args)
          : operation === "tts"
            ? await runTtsOperation(entry, args)
            : operation === "debug"
              ? await runDebugOperation(entry, args)
            : await runMediaOperation(entry, args);
      return {
        ok: true,
        result,
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const status = Number(error?.status ?? 500);
      if ([401, 403].includes(status)) {
        await closeContext(runtimeStateObjectKey);
      }
      log(
        "invokeGeminiCanvas returning error",
        JSON.stringify({
          runtimeStateObjectKey,
          operation: normalizeString(args.operation),
          status,
          code: error?.code ?? "gemini_canvas_browser_worker_failed",
          message,
        }),
      );
      return {
        ok: false,
        error: {
          code: error?.code ?? "gemini_canvas_browser_worker_failed",
          message,
          status,
          body: error?.bodyText ?? null,
        },
      };
    } finally {
      if (entry && entryLeaseAcquired) {
        entry.busy = false;
        entry.lastUsedAt = Date.now();
      }
      log(
        "invokeGeminiCanvas finally release",
        JSON.stringify({
          runtimeStateObjectKey,
          operation: normalizeString(args.operation),
          hadEntry: Boolean(entry),
          entryLeaseAcquired,
        }),
      );
    }
  }

  return { invokeGeminiCanvas };
}
