import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import path from "node:path";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { parseBoolean, resolveExecutablePath } from "./gemini-canvas-browser-pool-executable.mjs";
import { resolveRuntimeStateSource } from "./gemini-canvas-browser-pool-runtime-state.mjs";
import { normalizeMaxContexts, selectContextCapacityVictims } from "./gemini-canvas-browser-pool-resources.mjs";

export function inspectContextEntryForReuse(entry) {
  if (!entry?.context) {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  if (entry.contextClosed === true) {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  const owningBrowser =
    entry.browser ??
    (typeof entry.context?.browser === "function" ? entry.context.browser() : null);

  if (owningBrowser && typeof owningBrowser.isConnected === "function" && owningBrowser.isConnected() === false) {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  let openPages = [];
  try {
    openPages =
      typeof entry.context.pages === "function"
        ? entry.context.pages().filter((page) => !(typeof page?.isClosed === "function" && page.isClosed()))
        : [];
  } catch {
    return {
      recreate: true,
      adoptedPage: false,
    };
  }

  if (entry.page && typeof entry.page.isClosed === "function" && entry.page.isClosed() === false) {
    return {
      recreate: false,
      adoptedPage: false,
    };
  }

  const replacementPage = openPages[0] ?? null;
  if (replacementPage) {
    entry.page = replacementPage;
    return {
      recreate: false,
      adoptedPage: true,
    };
  }

  return {
    recreate: false,
    adoptedPage: false,
  };
}

export function createContextOwner({
  log, isFixtureCanvasBaseUrl, DEFAULT_IDLE_TIMEOUT_MS, DEFAULT_TIMEOUT_MS, DEFAULT_LOCALE,
  cloneProfileDirectory, removeManagedLaunchProfileClone, cleanupClonedLaunchProfile,
}) {
  const contexts = new Map();
  const initializingContexts = new Map();

  async function createContextEntry(args) {
    const browserCdpUrl = normalizeString(args.browserCdpUrl);
    const executablePath = resolveExecutablePath(
      args.browserExecutablePath ?? process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!browserCdpUrl && !executablePath) {
      throw Object.assign(
        new Error(
          "Unable to locate a Chromium-compatible browser. Set GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH.",
        ),
        {
          status: 500,
          code: "gemini_canvas_browser_not_found",
        },
      );
    }

    let browser = null;
    let context;
    let launchClonedProfile = false;
    let runtimeStateSource = null;
    let launchRuntimePath = browserCdpUrl ?? args.runtimeStateObjectKey;
    let defaultProfileDir = null;
    let page;
    // Own every acquired resource before context or page creation can reject.
    try {
      if (browserCdpUrl) {
        const requestedTimeoutMs = Number(args.timeoutMs || DEFAULT_TIMEOUT_MS);
        const cdpConnectTimeoutMs = Number.isFinite(requestedTimeoutMs)
          ? Math.max(5_000, Math.min(requestedTimeoutMs, 120_000))
          : 120_000;
        browser = await chromium.connectOverCDP(browserCdpUrl, {
          timeout: cdpConnectTimeoutMs,
        });
        context =
          browser.contexts()[0] ??
          (await browser.newContext({
            locale: normalizeString(args.locale) ?? DEFAULT_LOCALE,
            ignoreHTTPSErrors: true,
            bypassCSP: true,
          }));
      } else {
        runtimeStateSource = await resolveRuntimeStateSource(args.runtimeStateObjectKey, {
          allowFixtureEmptyProfile: isFixtureCanvasBaseUrl(args.baseUrl),
        });
        launchRuntimePath = runtimeStateSource.absolutePath;
        defaultProfileDir =
          runtimeStateSource.mode === "profile_dir"
            ? path.join(launchRuntimePath, "Default")
            : null;
        const launchOptions = {
          executablePath,
          headless: parseBoolean(process.env.GEMINI_CANVAS_BROWSER_HEADLESS, true),
          args: [
            "--disable-blink-features=AutomationControlled",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-features=BlockInsecurePrivateNetworkRequests,PrivateNetworkAccessRespectPreflightResults",
            "--ignore-certificate-errors",
            "--allow-insecure-localhost",
            ...(defaultProfileDir && existsSync(defaultProfileDir) ? ["--profile-directory=Default"] : []),
          ],
        };
        if (runtimeStateSource.mode === "profile_dir") {
          const persistentOptions = {
            ...launchOptions,
            locale: normalizeString(args.locale) ?? DEFAULT_LOCALE,
            ignoreHTTPSErrors: true,
            bypassCSP: true,
          };
          try {
            context = await chromium.launchPersistentContext(launchRuntimePath, persistentOptions);
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            const shouldCloneRetry =
              /Target page, context or browser has been closed|exitCode=21|Process singleton|Opening in existing browser session/i.test(
                message,
              );
            if (!shouldCloneRetry) {
              throw error;
            }
            launchRuntimePath = cloneProfileDirectory(runtimeStateSource.absolutePath);
            launchClonedProfile = true;
            context = await chromium.launchPersistentContext(launchRuntimePath, persistentOptions);
          }
        } else {
          browser = await chromium.launch(launchOptions);
          context = await browser.newContext({
            locale: normalizeString(args.locale) ?? DEFAULT_LOCALE,
            storageState: runtimeStateSource.absolutePath,
            ignoreHTTPSErrors: true,
            bypassCSP: true,
          });
        }
      }
      page = context.pages()[0] ?? (await context.newPage());
    } catch (error) {
      await context?.close().catch(() => undefined);
      await browser?.close().catch(() => undefined);
      if (launchClonedProfile) {
        await removeManagedLaunchProfileClone(launchRuntimePath).catch((cleanupError) => {
          log(
            "failed to remove browser profile clone after context creation error",
            cleanupError instanceof Error ? cleanupError.message : String(cleanupError),
          );
        });
      }
      throw error;
    }
    const entry = {
      browser,
      context,
      page,
      busy: false,
      lastUsedAt: Date.now(),
      runtimeStatePath: browserCdpUrl ?? runtimeStateSource.absolutePath,
      launchRuntimePath,
      runtimeStateMode: browserCdpUrl ? "browser_cdp" : runtimeStateSource.mode,
      launchClonedProfile,
      attachedCdp: Boolean(browserCdpUrl),
      contextClosed: false,
    };
    const markContextClosed = (reason) => {
      if (entry.contextClosed) {
        return;
      }
      entry.contextClosed = true;
      if (contexts.get(args.runtimeStateObjectKey) === entry) {
        contexts.delete(args.runtimeStateObjectKey);
      }
      log("Gemini Canvas context closed", JSON.stringify({ runtimeStateObjectKey: args.runtimeStateObjectKey, reason }));
      void cleanupClonedLaunchProfile(entry).catch((error) => {
        log(
          "failed to remove browser profile clone after unexpected close",
          error instanceof Error ? error.message : String(error),
        );
      });
    };
    if (typeof context?.on === "function") {
      context.on("close", () => {
        markContextClosed("context_close_event");
      });
    }
    if (browser && typeof browser.on === "function") {
      browser.on("disconnected", () => {
        markContextClosed("browser_disconnected");
      });
    }
    contexts.set(args.runtimeStateObjectKey, entry);
    return entry;
  }

  async function ensureContext(args) {
    const existing = contexts.get(args.runtimeStateObjectKey);
    if (existing) {
      const reuseInspection = inspectContextEntryForReuse(existing);
      if (reuseInspection.recreate) {
        log("evicting stale Gemini Canvas context", args.runtimeStateObjectKey);
        await closeContext(args.runtimeStateObjectKey);
      } else {
        if (reuseInspection.adoptedPage) {
          log("adopted surviving Gemini Canvas page for cached context", args.runtimeStateObjectKey);
        }
        existing.lastUsedAt = Date.now();
        return existing;
      }
    }

    const inflight = initializingContexts.get(args.runtimeStateObjectKey);
    if (inflight) {
      return inflight;
    }

    const capacityReady = reserveContextCapacity();
    let creation;
    creation = (async () => {
      await capacityReady;
      return createContextEntry(args);
    })().finally(() => {
      if (initializingContexts.get(args.runtimeStateObjectKey) === creation) {
        initializingContexts.delete(args.runtimeStateObjectKey);
      }
    });
    initializingContexts.set(args.runtimeStateObjectKey, creation);
    return creation;
  }

  async function closeContext(runtimeStateObjectKey) {
    const entry = contexts.get(runtimeStateObjectKey);
    if (!entry) {
      return;
    }
    contexts.delete(runtimeStateObjectKey);
    entry.contextClosed = true;
    try {
      if (entry.attachedCdp) {
        await entry.browser?.close().catch(() => undefined);
        return;
      }
      await entry.context.close().catch(() => undefined);
      if (entry.browser) {
        await entry.browser.close().catch(() => undefined);
      }
    } finally {
      await cleanupClonedLaunchProfile(entry).catch((error) => {
        log(
          "failed to remove browser profile clone while closing context",
          error instanceof Error ? error.message : String(error),
        );
      });
    }
  }

  async function evictIdleContexts() {
    const idleTimeoutMs = Number(
      process.env.GEMINI_CANVAS_BROWSER_IDLE_TIMEOUT_MS || DEFAULT_IDLE_TIMEOUT_MS,
    );
    const now = Date.now();
    for (const [key, entry] of contexts.entries()) {
      if (entry.busy) {
        continue;
      }
      if (now - entry.lastUsedAt < idleTimeoutMs) {
        continue;
      }
      log("evicting idle context", key);
      await closeContext(key);
    }
  }

  function reserveContextCapacity() {
    const maxContexts = normalizeMaxContexts(
      process.env.GEMINI_CANVAS_BROWSER_MAX_CONTEXTS,
    );
    const initializingCount = [...initializingContexts.keys()].filter(
      (key) => !contexts.has(key),
    ).length;
    const victims = selectContextCapacityVictims(contexts, initializingCount, maxContexts);
    const closingContexts = victims.map((victim) => {
      log("evicting least-recently-used context", victim);
      return closeContext(victim);
    });
    return Promise.all(closingContexts);
  }

  return { contexts, initializingContexts, createContextEntry, ensureContext, closeContext, evictIdleContexts };
}
