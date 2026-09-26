/**
 * Interactive page-owned AI Studio live probe.
 *
 * Purpose:
 * - Open a real AI Studio page from an existing browser-state/profile runtime
 * - Keep the browser visible so a human can log in or submit a prompt
 * - Capture the first real page-owned request contract instead of guessing that
 *   a raw `generateContent` fetch is sufficient
 * - Emit `target-rpc-summary.json` plus numbered pair files when the target
 *   MakerSuite text RPC contracts are captured
 *
 * Input JSON via stdin:
 * {
 *   "runtimeStateObjectKey": "credential-runtime/.../storage-state.json",
 *   "appUrl": "https://ai.studio/apps/...",
 *   "timeoutMs": 300000,
 *   "settleMs": 15000,
 *   "autoPrompt": "Reply with exactly OK.",
 *   "failUnlessTargetRpcCaptured": true,
 *   "responseBodyLimit": 65536,
 *   "browserExecutablePath": "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
 *   "authIndex": 0,
 *   "localWsPort": 9998,
 *   "requestUrlIncludes": ["ai.studio", "generativelanguage.googleapis.com"],
 *   "captureDir": ".runtime/aistudio-live-probe/manual"
 * }
 */

import { chromium } from "playwright-core";
import { pollProbeCapture } from "./aistudio-live-probe/capture-polling.mjs";
import { addHookScript } from "./aistudio-live-probe/browser-hook.mjs";
import { createCaptureWriter } from "./aistudio-live-probe/capture-writer.mjs";
import { createBrowserCapture } from "./aistudio-live-probe/browser-capture.mjs";
import { publishProbeCapture, publishProbeFailure } from "./aistudio-live-probe/publication.mjs";
import {
  normalizeString, normalizeStringArray, parseBoolean,
} from "./aistudio-live-probe/input-text.mjs";
import {
  buildNormalizedTargetRpcContract, isReplayReadyTargetRpcContract,
} from "./aistudio-live-probe/rpc-contract.mjs";
import { buildProbeSummary, extractAistudioUiSignals } from "./aistudio-live-probe/diagnostics.mjs";
import {
  bestEffortContinueIntoApp, bestEffortApplyRemixModal, bestEffortDismissAistudioOverlays,
  bestEffortSelectGoogleAccount, bestEffortAutoPrompt, shouldRetryAutoPromptDuringPolling,
  bestEffortLaunchOwnedApp,
} from "./aistudio-live-probe/ui-actions.mjs";
import { collectPageSnapshot, collectFrameDiagnostics } from "./aistudio-live-probe/page-observation.mjs";
import {
  sendActiveTrigger, maybeDispatchLocalProxyMessages, probeRunAppFrameFetch,
} from "./aistudio-live-probe/local-dispatch.mjs";
import { buildDefaultLocalProxyRequest } from "./aistudio-live-probe/request-config.mjs";
import { readStdin, parseInput, validateInput, ensureCaptureDir } from "./aistudio-live-probe/cli-io.mjs";
import { readRuntimeStateFile } from "./aistudio-live-probe/runtime-state-file.mjs";
import { writeAtomicCapture as writeFile } from "./aistudio-live-probe/atomic-capture-file.mjs";
import path from "node:path";
import {
  resolveExecutablePath, resolveBrowserProxySettings, buildBrowserProxyLaunchOptions,
  assertBrowserProxyReachable,
} from "./aistudio-live-probe/browser-preflight.mjs";
import {
  createCaptureRequestIdFactory, createResponseRequestAttributor, summarizeTargetRpcContracts,
} from "./aistudio-live-probe/rpc-attribution.mjs";
import { createLocalWebSocketCaptureServer } from "./aistudio-live-probe/websocket-capture.mjs";
import { resolveRuntimeStateSource, closeObjectStorageClient } from "./aistudio-live-probe/runtime-storage.mjs";

const DEFAULT_TIMEOUT_MS = 300_000;
const DEFAULT_SETTLE_MS = 15_000;
const DEFAULT_RESPONSE_BODY_LIMIT = 65_536;
const DEFAULT_LOCALE = "zh-CN";
const DEFAULT_APP_URL = "https://ai.studio/apps/fa9cb8e6-4d92-4fb6-a2b1-b947405c22ae";
const DEFAULT_LOCAL_PROXY_DELAY_MS = 1_200;
const DEFAULT_URL_PATTERNS = [
  "ai.studio",
  "aistudio.google.com",
  "makersuite.google.com",
  "clients6.google.com",
  "alkalimakersuite-pa.clients6.google.com",
  "generativelanguage.googleapis.com",
  "alkalimakersuite-pa.googleapis.com",
];

async function main() {
  let browser = null;
  let context = null;
  let page = null;
  let localWebSocketServer = null;
  let captureDir = null;
  let capture = null;
  let captureWriter = null;
  let browserCapture = null;
  try {
    const raw = await readStdin();
    const input = parseInput(raw);
    if (normalizeString(input.captureDir)) {
      captureDir = ensureCaptureDir(input.captureDir);
    }
    validateInput(input);

    const appUrl = normalizeString(input.appUrl) ?? DEFAULT_APP_URL;
    const timeoutMs = Math.max(Number(input.timeoutMs || DEFAULT_TIMEOUT_MS), 30_000);
    const settleMs = Math.max(Number(input.settleMs || DEFAULT_SETTLE_MS), 2_000);
    const responseBodyLimit = Math.max(
      Number(input.responseBodyLimit || DEFAULT_RESPONSE_BODY_LIMIT),
      1024,
    );
    const authIndex = Number.isFinite(Number(input.authIndex)) ? Number(input.authIndex) : 0;
    const localWsPort = Number.isFinite(Number(input.localWsPort))
      ? Number(input.localWsPort)
      : null;
    const locale = normalizeString(input.locale) ?? DEFAULT_LOCALE;
    const autoSelectGoogleAccountEmail =
      normalizeString(input.autoSelectGoogleAccountEmail) ??
      normalizeString(input.auto_select_google_account_email) ??
      normalizeString(process.env.AISTUDIO_PROBE_AUTO_SELECT_GOOGLE_ACCOUNT_EMAIL);
    const autoPromptMaxAttempts = Number.isFinite(Number(input.autoPromptMaxAttempts))
      ? Math.max(1, Number(input.autoPromptMaxAttempts))
      : Number.isFinite(Number(input.auto_prompt_max_attempts))
        ? Math.max(1, Number(input.auto_prompt_max_attempts))
        : 4;
    const autoPromptPollMs = Number.isFinite(Number(input.autoPromptPollMs))
      ? Math.max(0, Number(input.autoPromptPollMs))
      : Number.isFinite(Number(input.auto_prompt_poll_ms))
        ? Math.max(0, Number(input.auto_prompt_poll_ms))
        : 1000;
    captureDir = captureDir ?? ensureCaptureDir(input.captureDir);
    const requestUrlIncludes = normalizeStringArray(
      input.requestUrlIncludes,
      DEFAULT_URL_PATTERNS,
    );
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ??
        process.env.AISTUDIO_BROWSER_EXECUTABLE_PATH ??
        null,
    );
    if (!executablePath) {
      throw new Error(
        "Unable to locate a Chromium-compatible browser. Set AISTUDIO_BROWSER_EXECUTABLE_PATH.",
      );
    }
    const browserProxySettings = resolveBrowserProxySettings(input);
    const browserProxyLaunchOptions =
      buildBrowserProxyLaunchOptions(browserProxySettings);

    const runtimeState = await resolveRuntimeStateSource(input.runtimeStateObjectKey);
    const localProxyRequest = buildDefaultLocalProxyRequest(input);
    const localProxyLogLevel =
      normalizeString(input.localProxyLogLevel)?.toUpperCase() ?? "DEBUG";
    const localProxyDelayMs = Number.isFinite(Number(input.localProxyDelayMs))
      ? Math.max(Number(input.localProxyDelayMs), 0)
      : DEFAULT_LOCAL_PROXY_DELAY_MS;
    const localProxyMessages = localProxyRequest
      ? [
          { event_type: "set_log_level", level: localProxyLogLevel },
          localProxyRequest,
        ]
      : [];

    capture = {
      startedAt: new Date().toISOString(),
      runtimeStateObjectKey: input.runtimeStateObjectKey,
      runtimeStateMode: runtimeState.mode,
      runtimeStatePath: runtimeState.absolutePath,
      executablePath,
      appUrl,
      browserProxyMode: browserProxySettings?.mode ?? "system",
      browserProxyServer: browserProxySettings?.launchProxy?.server ?? null,
      browserProxyPreflight: null,
      autoSelectGoogleAccountEmail,
      autoPromptMaxAttempts,
      autoPromptPollMs,
      requestUrlIncludes,
      requests: [],
      responses: [],
      websockets: [],
      pageErrors: [],
      console: [],
      autoActions: [],
      localProxyRequest,
      localProxyLogLevel,
      localProxyDelayMs,
    };

    try {
      capture.browserProxyPreflight =
        await assertBrowserProxyReachable(browserProxySettings);
    } catch (error) {
      capture.browserProxyPreflight = {
        ok: false,
        code: error?.code ?? "aistudio_browser_proxy_preflight_failed",
        status: Number(error?.status ?? 503),
        message: error instanceof Error ? error.message : String(error),
        host: error?.host ?? null,
        port: error?.port ?? null,
        server: error?.server ?? capture.browserProxyServer,
        rawProxy: error?.rawProxy ?? browserProxySettings?.rawProxy ?? null,
      };
      throw error;
    }

    captureWriter = createCaptureWriter(async () => {
      await writeFile(
        path.join(captureDir, "capture.json"),
        `${JSON.stringify(capture, null, 2)}\n`,
        "utf8",
      );
    });
    const persistCapture = captureWriter.request;

    localWebSocketServer = await createLocalWebSocketCaptureServer(
      localWsPort,
      capture,
      persistCapture,
      {
        initialMessages: localProxyMessages,
        initialDelayMs: localProxyDelayMs,
      },
    );


    browserCapture = createBrowserCapture({
      capture, requestUrlIncludes, responseBodyLimit, persistCapture,
    });

    if (runtimeState.mode === "profile_dir") {
      context = await chromium.launchPersistentContext(runtimeState.absolutePath, {
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_PROBE_HEADLESS, false),
        ...(browserProxyLaunchOptions.proxy
          ? { proxy: browserProxyLaunchOptions.proxy }
          : {}),
        locale,
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
          ...browserProxyLaunchOptions.args,
        ],
      });
    } else {
      const storageStateJson = await readRuntimeStateFile(runtimeState.absolutePath);
      browser = await chromium.launch({
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_PROBE_HEADLESS, false),
        ...(browserProxyLaunchOptions.proxy
          ? { proxy: browserProxyLaunchOptions.proxy }
          : {}),
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
          ...browserProxyLaunchOptions.args,
        ],
      });
      context = await browser.newContext({
        storageState: storageStateJson,
        locale,
      });
    }

    await context.addInitScript((index) => {
      globalThis.__AISTUDIO_AUTH_INDEX__ = index;
    }, authIndex);
    await context.addInitScript(addHookScript);
    page = context.pages()[0] ?? (await context.newPage());
    await page.bringToFront().catch(() => undefined);

    browserCapture.attach(context, page);

    await page.goto(appUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await page.waitForTimeout(5000);
    await bestEffortSelectGoogleAccount(
      page,
      capture,
      autoSelectGoogleAccountEmail,
    );

    const initialSnapshot = await collectPageSnapshot(page, "initial");
    const initialFrames = await collectFrameDiagnostics(page, "initial");
    await writeFile(
      path.join(captureDir, "initial-page.json"),
      `${JSON.stringify(initialSnapshot, null, 2)}\n`,
      "utf8",
    );
    await writeFile(
      path.join(captureDir, "initial-frames.json"),
      `${JSON.stringify(initialFrames, null, 2)}\n`,
      "utf8",
    );
    await page.screenshot({
      path: path.join(captureDir, "initial-page.png"),
      fullPage: true,
    });
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortContinueIntoApp(page, capture);
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortApplyRemixModal(page, capture);
    await bestEffortLaunchOwnedApp(page, capture);
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortApplyRemixModal(page, capture);
    await page.waitForTimeout(2000);
    capture.captureArmedAt = new Date().toISOString();
    browserCapture.activate();
    await persistCapture();
    await sendActiveTrigger(page, capture);
    await persistCapture();
    await bestEffortDismissAistudioOverlays(page, capture);

    if (localWebSocketServer && localProxyMessages.length > 0) {
      await maybeDispatchLocalProxyMessages(
        page,
        capture,
        localWebSocketServer,
        localProxyMessages,
        localProxyDelayMs,
        "pre_prompt",
        persistCapture,
      );
    }

    capture.runAppDirectFetchProbe = await probeRunAppFrameFetch(page, {
      method: "GET",
      url: "https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger",
      headers: {},
    });
    capture.autoActions.push({
      action: "probe-runapp-direct-fetch",
      ok: Boolean(capture.runAppDirectFetchProbe?.ok),
      status: capture.runAppDirectFetchProbe?.status ?? null,
      error: capture.runAppDirectFetchProbe?.error ?? null,
    });
    await bestEffortDismissAistudioOverlays(page, capture);
    await bestEffortApplyRemixModal(page, capture);
    await persistCapture();

    let autoPromptSubmitted = false;
    const normalizedAutoPrompt = normalizeString(input.autoPrompt);
    if (normalizedAutoPrompt) {
      await bestEffortDismissAistudioOverlays(page, capture);
      autoPromptSubmitted = await bestEffortAutoPrompt(page, normalizedAutoPrompt, capture, {
        maxAttempts: autoPromptMaxAttempts,
        pollMs: autoPromptPollMs,
      });
      await page.waitForTimeout(1500);
      await bestEffortDismissAistudioOverlays(page, capture);
      await bestEffortApplyRemixModal(page, capture);
      await persistCapture();
      if (localWebSocketServer && localProxyMessages.length > 0) {
        await maybeDispatchLocalProxyMessages(
          page,
          capture,
          localWebSocketServer,
          localProxyMessages,
          localProxyDelayMs,
          "post_prompt",
          persistCapture,
        );
      }
    }

    await pollProbeCapture({
      page, capture, timeoutMs, settleMs, localWebSocketServer, localProxyMessages,
      localProxyDelayMs, normalizedAutoPrompt, autoPromptSubmitted, persistCapture,
      getMatchTimes: browserCapture.getMatchTimes,
    });

    await browserCapture.stop();
    await localWebSocketServer?.close();
    await publishProbeCapture({
      page, capture, captureDir, initialSnapshot, input, executablePath,
      runtimeState, appUrl, localProxyRequest, persistCapture,
    });
  } catch (error) {
    await browserCapture?.stop().catch(() => {});
    await localWebSocketServer?.close().catch(() => {});
    await captureWriter?.stop().catch(() => {});
    await publishProbeFailure({ error, capture, captureDir });
  } finally {
    await localWebSocketServer?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
    await captureWriter?.stop().catch(() => {});
    closeObjectStorageClient();
  }
}

export {
  assertBrowserProxyReachable,
  buildProbeSummary,
  buildBrowserProxyLaunchOptions,
  bestEffortApplyRemixModal,
  bestEffortDismissAistudioOverlays,
  bestEffortSelectGoogleAccount,
  bestEffortAutoPrompt,
  shouldRetryAutoPromptDuringPolling,
  createCaptureRequestIdFactory,
  createLocalWebSocketCaptureServer,
  createResponseRequestAttributor,
  extractAistudioUiSignals,
  isReplayReadyTargetRpcContract,
  resolveBrowserProxySettings,
  buildNormalizedTargetRpcContract,
  summarizeTargetRpcContracts,
};

if (process.env.AISTUDIO_PROBE_SUPPRESS_MAIN !== "1") {
  main();
}
