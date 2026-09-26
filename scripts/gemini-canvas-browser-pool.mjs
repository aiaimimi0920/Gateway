import { createBrowserPoolServerOwner } from "./gemini-canvas-browser-pool-server.mjs";
import {
  bodyIndicatesMusicPendingOrBusy,
  invokeContractIndicatesConcreteProgress,
  detectMediaProviderGate,
  shouldBlockMediaProviderGate,
  recentMediaProviderGateText,
  shouldRetryMediaPromptSubmission,
} from "./gemini-canvas-browser-pool-media-policy.mjs";
import { createFetchExecutionOwner, isBrowserDownloadAssetUrl } from "./gemini-canvas-browser-pool-fetch-execution.mjs";
import { runFetchPageRequest } from "./gemini-canvas-browser-pool-fetch-page.mjs";
export { shouldAttemptConnectedClientFetchFallback } from "./gemini-canvas-browser-pool-fetch-execution.mjs";
import { createFetchPageMusicOwner } from "./gemini-canvas-browser-pool-fetch-page-music.mjs";
import { createFetchPreviewOwner } from "./gemini-canvas-browser-pool-fetch-preview.mjs";
import { createInvocationDispatcher } from "./gemini-canvas-browser-pool-dispatcher.mjs";
import { createMediaExecutionOwner } from "./gemini-canvas-browser-pool-media-execution.mjs";
export { shouldExitCanvasProgramSurfaceForMediaMode } from "./gemini-canvas-browser-pool-media-execution.mjs";
import { createMediaPollingOwner } from "./gemini-canvas-browser-pool-media-polling.mjs";
import { createBootstrapExecutionOwner } from "./gemini-canvas-browser-pool-bootstrap-execution.mjs";
import { createBootstrapPollingOwner } from "./gemini-canvas-browser-pool-bootstrap-polling.mjs";
import { createBootstrapPreviewResultOwner } from "./gemini-canvas-browser-pool-bootstrap-preview-result.mjs";
import { collectButtonSnapshot, collectPageSnapshot } from "./gemini-canvas-browser-pool-page-snapshot.mjs";
import { createDebugOperationOwner } from "./gemini-canvas-browser-pool-debug.mjs";
import { createTextOperationOwner } from "./gemini-canvas-browser-pool-text.mjs";
import { createTtsOperationOwner } from "./gemini-canvas-browser-pool-tts.mjs";
import { extractAudioBytes, extractImageBytes, downloadBinaryViaNavigation } from "./gemini-canvas-browser-pool-payload.mjs";
import { bodyTextSuggestsVideoTemplateSelection, hasVideoCreateAction, tryClickVideoCreateAction, trySelectVideoTemplateCard } from "./gemini-canvas-browser-pool-video-ui.mjs";
import { createMediaAssetSelectionOwner } from "./gemini-canvas-browser-pool-media-assets.mjs";
import { submitPrompt, tryClickSendButton, buildSendButtonCandidates } from "./gemini-canvas-browser-pool-composer.mjs";
import { createOperationUiOwner } from "./gemini-canvas-browser-pool-operation-ui.mjs";
import { createProgramHandleStateOwner } from "./gemini-canvas-browser-pool-program-state.mjs";
import { collectProgramHandleSnapshot } from "./gemini-canvas-browser-pool-program-snapshot.mjs";
import {
  isInterestingNetworkUrl, isLikelyAvatarUrl, isLikelyNoiseMediaUrl, inferMimeTypeFromUrl, isBlobLikeUrl,
  isAudioLikeMimeType, isAudioLikeUrl, isVideoLikeUrl, normalizeGeminiBrowserAssetUrl, pushUniqueMediaUrl,
} from "./gemini-canvas-browser-pool-media-urls.mjs";
import { createNetworkCaptureOwner } from "./gemini-canvas-browser-pool-network-capture.mjs";
import { createProgramCaptureOwner } from "./gemini-canvas-browser-pool-capture-metadata.mjs";
import {
  uniqueStrings, extractProgramAppPaths, extractAppPath, extractProgramHandleHintsFromText,
  mergeProgramHandleHints, strongestProgramHandleHint, deriveConversationIdForAppPath,
  dedupeProgramHandlePairs, extractProgramHandlePairs, isConcreteProgramUrlCandidate,
} from "./gemini-canvas-browser-pool-program-handles.mjs";
import { createConversationResetOwner } from "./gemini-canvas-browser-pool-conversation-reset.mjs";
import { executeCanvasProxyPreviewNoKeyFetch } from "./gemini-canvas-browser-pool-no-key-fetch.mjs";
import { executeCanvasProxyPreviewNoKeyMusic } from "./gemini-canvas-browser-pool-preview-music.mjs";
import { executeCanvasProgramPageNoKeyMusic } from "./gemini-canvas-browser-pool-page-music.mjs";
import { createMediaPageOwner } from "./gemini-canvas-browser-pool-media-page.mjs";
import { createPreviewPageOwner } from "./gemini-canvas-browser-pool-preview-page.mjs";
import { createConnectedClientOwner } from "./gemini-canvas-browser-pool-connected-client.mjs";
import { createGoogleAuthOwner } from "./gemini-canvas-browser-pool-google-auth.mjs";
import { installCanvasProxyPreviewAuthIndexBridge } from "./gemini-canvas-browser-pool-auth-bridge.mjs";
import { createPreviewOwner } from "./gemini-canvas-browser-pool-preview.mjs";
import { sanitizeCanvasProxyHeaders, sanitizeBrowserFetchHeaders } from "./gemini-canvas-browser-pool-headers.mjs";
import { createProxyLaunchOwner } from "./gemini-canvas-browser-pool-proxy-launch.mjs";
import { createInvokeAssemblyOwner } from "./gemini-canvas-browser-pool-invoke.mjs";
import { createProxyDiscoveryOwner } from "./gemini-canvas-browser-pool-proxy-discovery.mjs";
import { decodeEscapedCanvasProxyHtml, injectForcedCanvasProxyAuthIndex, extractCanvasProxyClientHtmlFromTexts } from "./gemini-canvas-browser-pool-proxy-html.mjs";
import { createProgramRpcOwner } from "./gemini-canvas-browser-pool-rpc-candidates.mjs";
import { createInvokeContractOwner } from "./gemini-canvas-browser-pool-invoke-merge.mjs";
import { createMediaTargetOwner } from "./gemini-canvas-browser-pool-media-targets.mjs";
import {
  mergeActionContract, extractCanvasProgramActionContractFromText, extractQuotedScalar,
  extractNumberScalar, extractDurationSecondsFromBodyText, inferInvokeUiState,
} from "./gemini-canvas-browser-pool-action.mjs";
import { createTransportHintOwner } from "./gemini-canvas-browser-pool-transport.mjs";
import { createProgramNavigationOwner } from "./gemini-canvas-browser-pool-navigation.mjs";
import { createCookieOwner } from "./gemini-canvas-browser-pool-cookies.mjs";
import {
  createAppPageOwner, hasPromptTextbox,
  shouldTreatGeminiPageAsAuthBlocked, pageLooksLikeReusableGeminiAppSurface, findAttachedGeminiAppPage,
} from "./gemini-canvas-browser-pool-app.mjs";
import { createContextOwner, inspectContextEntryForReuse } from "./gemini-canvas-browser-pool-context.mjs";
import { WebSocketServer } from "ws";
import { createServer } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { mkdirSync, readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  applyGeminiAccountScope,
  normalizeObject,
  normalizeString,
  scopeGeminiUrlToAuthUser,
} from "./gemini-canvas-browser-pool-input.mjs";
import { parseBoolean, resolveExecutablePath } from "./gemini-canvas-browser-pool-executable.mjs";
import { loadOrCreateTlsCertificate } from "./gemini-canvas-browser-pool-tls.mjs";
import { getStorageRoot, resolveRuntimeStateSource } from "./gemini-canvas-browser-pool-runtime-state.mjs";
import { createProfileCloneOwner } from "./gemini-canvas-browser-pool-profile.mjs";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 42321;
const DEFAULT_TLS_PORT = 42322;
const DEFAULT_IDLE_TIMEOUT_MS = 30 * 60 * 1000;
const DEFAULT_TIMEOUT_MS = 8 * 60 * 1000;
const DEFAULT_LOCALE = "zh-CN";
const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const APP_ROOT = path.resolve(SCRIPT_DIR, "..");

const { closePageSafely, acquireMediaOperationPage } = createMediaPageOwner({ log });

const { connectedClients, nextConnectedRequestId, listConnectedClients, cleanupConnectedClient,
  handleConnectedClientMessage, dispatchConnectedProxyRequest, ensureLoopbackConnectedClient,
} = createConnectedClientOwner({ log, SCRIPT_DIR, DEFAULT_HOST, DEFAULT_TLS_PORT });

function log(...parts) {
  console.log("[gemini-canvas-pool]", ...parts);
}

const { cloneProfileDirectory, removeManagedLaunchProfileClone, cleanupClonedLaunchProfile } = createProfileCloneOwner(log);

const { contexts, initializingContexts, createContextEntry, ensureContext, closeContext, evictIdleContexts } = createContextOwner({
  log, isFixtureCanvasBaseUrl, DEFAULT_IDLE_TIMEOUT_MS, DEFAULT_TIMEOUT_MS, DEFAULT_LOCALE,
  cloneProfileDirectory, removeManagedLaunchProfileClone, cleanupClonedLaunchProfile,
});

const { parseCookieHeader, syncCookieHeaderIntoContext, contextHasGeminiAuthCookies, syncEmbeddedStorageStateIntoContext } = createCookieOwner({ log });

const { inferGoogleAuthUser, buildGoogleFetchAuthHeaders } = createGoogleAuthOwner({ parseCookieHeader });

const { ensureAppPage, tryResolveGoogleConsent } = createAppPageOwner({
  log, inferGoogleAuthUser, syncCookieHeaderIntoContext, collectButtonSnapshot,
});

const { resolveProgramPageUrl, ensureProgramPage, ensureSharePage, waitForShareSurface, tryFollowShareEntryPoint } = createProgramNavigationOwner({ log, collectButtonSnapshot });

const { selectStableProgramPair, concreteAppPathFromUrl, selectCanonicalProgramPair, buildProgramHandleState } = createProgramHandleStateOwner({ resolveProgramPageUrl });

const { extractTransportHintsFromNetworkUrl, mergeTransportHints, hasTransportHints } = createTransportHintOwner({ uniqueStrings });

const { collectPlayerReadyTargetCandidates, scorePlayerReadyTargetCandidate } = createMediaTargetOwner({
  inferMimeTypeFromUrl, isBlobLikeUrl, isAudioLikeUrl, isVideoLikeUrl,
});

const { selectAudioAsset, selectAudioAssetFromInvokeContract, selectImageAssets, selectVideoAsset, selectMediaAssetsForOperation } = createMediaAssetSelectionOwner({ scorePlayerReadyTargetCandidate });

const { mergeInvokeContract } = createInvokeContractOwner({ scorePlayerReadyTargetCandidate });

const { deriveProgramRpcInvokeCandidate, extractModelHintFromRpcText } = createProgramRpcOwner({ extractAppPath });

const { collectCanvasProxyContractTexts, deriveCanvasProxyWsInvokeCandidate,
  extractCanvasProxyWsUrlFromText, extractCanvasProxyTargetDomainFromText,
} = createProxyDiscoveryOwner({
  extractAppPath, extractModelHintFromRpcText,
});

const {
  classifyHandlePairSurface, readRpcIdFromRequestUrl, shouldCaptureProgramHandleTraffic,
  trimProgramRpcCaptureText, classifyProgramRpcCapture, pushProgramRpcCapture,
  extractRequestCookieHeader, captureCookieHeaderFromContext,
} = createProgramCaptureOwner({ extractCanvasProxyWsUrlFromText, extractCanvasProxyTargetDomainFromText });

const { buildCanvasProgramInvokeContract } = createInvokeAssemblyOwner({
  collectPlayerReadyTargetCandidates, deriveCanvasProxyWsInvokeCandidate, deriveProgramRpcInvokeCandidate,
});

const { buildBootstrapPreviewResult } = createBootstrapPreviewResultOwner({
  buildProgramHandleState, mergeInvokeContract, buildCanvasProgramInvokeContract,
});

const { pollBootstrapProgram } = createBootstrapPollingOwner({
  collectProgramHandleSnapshot, buildProgramHandleState, hasConcreteProgramHandleState,
  invokeContractIndicatesConcreteProgress, hasTransportHints,
});

const { startNetworkCapture } = createNetworkCaptureOwner({
  extractTransportHintsFromNetworkUrl, mergeTransportHints, mergeInvokeContract, buildCanvasProgramInvokeContract,
  classifyHandlePairSurface, readRpcIdFromRequestUrl, shouldCaptureProgramHandleTraffic,
  trimProgramRpcCaptureText, classifyProgramRpcCapture, pushProgramRpcCapture,
  extractRequestCookieHeader, captureCookieHeaderFromContext,
  isInterestingNetworkUrl, isLikelyAvatarUrl, inferMimeTypeFromUrl, isAudioLikeMimeType, isAudioLikeUrl, pushUniqueMediaUrl,
});

const { tryLaunchCanvasProxyClientFromCapturedHtml } = createProxyLaunchOwner({
  collectCanvasProxyContractTexts, inferGoogleAuthUser, log,
});

const { tryOpenCanvasProxyPreview, stampCanvasProxyPreviewFrames } = createPreviewOwner({
  installCanvasProxyPreviewAuthIndexBridge, inferGoogleAuthUser,
});

const { isCanvasProxyPreviewFrameUrl, ensureCanvasProxyPreviewFrame } = createPreviewPageOwner({
  ensureSharePage, tryFollowShareEntryPoint, tryOpenCanvasProxyPreview,
  stampCanvasProxyPreviewFrames, inferGoogleAuthUser, log,
});

export { inspectContextEntryForReuse };

function isFixtureCanvasBaseUrl(baseUrl) {
  const normalized = normalizeString(baseUrl);
  if (!normalized) {
    return false;
  }
  try {
    const origin = new URL(normalized);
    return !/gemini\.google\.com$/i.test(origin.hostname);
  } catch {
    return false;
  }
}

export { shouldTreatGeminiPageAsAuthBlocked, pageLooksLikeReusableGeminiAppSurface, findAttachedGeminiAppPage };

const { clickNewChat, resetConversation, dismissGeminiAppInterstitials } = createConversationResetOwner({ log });

const { clickOperationMode, clickMediaActionButton, trySelectMusicStyleCard, openOperationModeSelector, operationModeAppearsSelected } = createOperationUiOwner({ operationConfig, operationNavLabel, dismissGeminiAppInterstitials, log });

const { buildTtsDiagnostics, listenControlCandidates, clickListenControlWithFallback, runTtsOperation } = createTtsOperationOwner({
  DEFAULT_TIMEOUT_MS, operationConfig, isFixtureCanvasBaseUrl, resolveProgramPageUrl,
  resetConversation, startNetworkCapture, collectPageSnapshot, collectButtonSnapshot,
  selectAudioAsset, buildProgramHandleState, log,
});

const {
  isTransientAssistantStatusLine, normalizeAssistantText, promptLooksLikeGreeting,
  textLooksLikeGenericWelcome, bodyContainsSubmittedPrompt, bodyIndicatesGenerationInProgress,
  extractExactAnswerDirective, augmentToolHistoryPrompt, collectTextSnapshot, runTextOperation,
} = createTextOperationOwner({
  DEFAULT_TIMEOUT_MS, operationConfig, resolveProgramPageUrl, startNetworkCapture,
  resetConversation, buildProgramHandleState, log,
});

const { runDebugOperation } = createDebugOperationOwner({
  DEFAULT_TIMEOUT_MS, resolveProgramPageUrl, resetConversation, startNetworkCapture,
  clickOperationMode, log, buildProgramHandleState,
});

const {
  normalizeBootstrapOperation, defaultBootstrapPrompt, runBootstrapProgramOperation,
} = createBootstrapExecutionOwner({
  DEFAULT_TIMEOUT_MS,
  operationConfig,
  resolveProgramPageUrl,
  startNetworkCapture,
  mergeInvokeContract,
  buildCanvasProgramInvokeContract,
  ensureProgramPage,
  ensureSharePage,
  waitForShareSurface,
  tryFollowShareEntryPoint,
  ensureAppPage,
  log,
  shouldStayOnCanvasProxyDiscoverySurface,
  clickOperationMode,
  tryOpenCanvasProxyPreview,
  stampCanvasProxyPreviewFrames,
  inferGoogleAuthUser,
  collectCanvasProxyContractTexts,
  canvasProxyPreviewNeedsDirectLaunch,
  tryLaunchCanvasProxyClientFromCapturedHtml,
  buildBootstrapPreviewResult,
  clickNewChat,
  trySelectMusicStyleCard,
  pollBootstrapProgram,
  buildProgramHandleState,
  clickMediaActionButton,
});

const { pollMediaOperation } = createMediaPollingOwner({
  mergeInvokeContract,
  buildCanvasProgramInvokeContract,
  selectMediaAssetsForOperation,
  log,
  shouldRetryMediaPromptSubmission,
  retryMediaPromptSubmission,
  resetConversation,
  clickOperationMode,
  detectMediaProviderGate,
  recentMediaProviderGateText,
  shouldBlockMediaProviderGate,
  clickMediaActionButton,
  bodyIndicatesMusicPendingOrBusy,
  buildProgramHandleState,
});

const { runMediaOperation } = createMediaExecutionOwner({
  DEFAULT_TIMEOUT_MS,
  operationConfig,
  isFixtureCanvasBaseUrl,
  acquireMediaOperationPage,
  resolveProgramPageUrl,
  resetConversation,
  startNetworkCapture,
  clickOperationMode,
  log,
  pollMediaOperation,
  closePageSafely,
});

function operationConfig(operation) {
  switch (operation) {
    case "text":
      return {
        buttonName: null,
        modeIndicator: null,
        resultTimeoutMs: 2 * 60 * 1000,
      };
    case "tts":
      return {
        buttonName: null,
        modeIndicator: null,
        resultTimeoutMs: 3 * 60 * 1000,
      };
    case "image":
      return {
        buttonName: /制作图片|Create image|Create images|Make image/i,
        selectedButtonName: /取消选择.?制作图片|取消选择\"制作图片\"|Deselect.*image|Cancel selection.*image/i,
        modeIndicator: /为图片选择风格|正在创建您的图片|Choose a style for your image|Creating your image/i,
        routePathPattern: /\/images(?:\/|$|\?)/i,
        resultTimeoutMs: 4 * 60 * 1000,
      };
    case "music":
      return {
        buttonName: /创作音乐|Create music/i,
        modeIndicator: /制作音乐|创作音乐|Create music/i,
        resultTimeoutMs: 8 * 60 * 1000,
      };
    case "video":
      return {
        buttonName: /创作视频|制作视频|Create video/i,
        selectedButtonName: /取消选择.?创作视频|取消选择.?制作视频|Deselect.*video|Cancel selection.*video/i,
        modeIndicator: /挑选一个模板|开始制作你的视频|choose a template|start creating your video|视频生成模板图片|video generation template/i,
        routePathPattern: /\/videos(?:\/|$|\?)/i,
        resultTimeoutMs: 12 * 60 * 1000,
      };
    default:
      throw Object.assign(new Error(`Unsupported Gemini Canvas operation '${operation}'.`), {
        status: 400,
        code: "gemini_canvas_invalid_operation",
      });
  }
}

function operationNavLabel(operation) {
  switch (operation) {
    case "image":
      return /图片|Images?|Image/i;
    case "video":
      return /视频|Videos?|Video/i;
    default:
      return null;
  }
}

function hasCanvasProxyProgramCandidate(handlePairs, invokeContract = null) {
  if (invokeContract?.transportKind === "canvas_program_ws_candidate") {
    return true;
  }
  return [...(handlePairs || [])].some((pair) => pair?.sourceSurface === "canvas_proxy_client");
}

function shouldStayOnCanvasProxyDiscoverySurface(discoveryOnly, snapshot, captureState) {
  return (
    discoveryOnly &&
    (/Browser API Proxy Client/i.test(String(snapshot?.bodyText || "")) ||
      hasCanvasProxyProgramCandidate(captureState?.handlePairs, captureState?.invokeContract))
  );
}

function canvasProxyPreviewNeedsDirectLaunch(
  canvasProxyPreview,
  bodyText,
  hasCapturedCanvasProxyHtml,
) {
  if (!hasCapturedCanvasProxyHtml) {
    return false;
  }
  const bridgeEvents = Array.isArray(canvasProxyPreview?.bridgeEvents)
    ? canvasProxyPreview.bridgeEvents
    : [];
  const reportsAuthIndexFailure = bridgeEvents.some(
    (event) =>
      event?.type === "error" &&
      /authIndex postMessage timeout/i.test(String(event?.errorMessage ?? event?.messagePreview ?? "")),
  );
  const reportsEmbeddedWebSocketFailure = bridgeEvents.some((event) =>
    /WebSocket initialization failed|WebSocket connection .* is not allowed in Canvas/i.test(
      String(event?.errorMessage ?? event?.messagePreview ?? ""),
    ),
  );
  const hasOnlyBootstrapBridgeTraffic =
    Number(canvasProxyPreview?.bridge?.eventCount ?? 0) <= 1 && bridgeEvents.length <= 1;
  const alreadyMaterialized = /System Logs Output|Connecting\.\.\.|Connected|Disconnected/i.test(
    String(bodyText || ""),
  );
  return (
    (reportsAuthIndexFailure || reportsEmbeddedWebSocketFailure || hasOnlyBootstrapBridgeTraffic) &&
    !alreadyMaterialized
  );
}

function hasConcreteProgramHandleState(state) {
  return Boolean(
    normalizeString(state?.canvasProgramUrl) &&
      normalizeString(state?.appPath) &&
      normalizeString(state?.conversationId),
  );
}

async function retryMediaPromptSubmission(page, prompt, timeoutMs) {
  const textbox = page
    .locator(
      '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
    )
    .first();
  try {
    await textbox.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 4_000) });
    await textbox.focus().catch(() => undefined);
    await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
    await page.keyboard.press("Backspace").catch(() => undefined);
    await page.keyboard.type(prompt, { delay: 18 }).catch(() => undefined);
    await page.waitForTimeout(250);
  } catch {
    // fall through to button/keyboard retries below
  }

  const submitCandidates = [
    page.getByRole("button", { name: /提交|Submit/i }).last(),
    page.locator('button[aria-label*="Submit"], button[aria-label*="提交"], button[title*="Submit"], button[title*="提交"]').last(),
  ];
  for (const candidate of submitCandidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 1_500) });
      await candidate.click({ timeout: Math.min(timeoutMs, 4_000), force: true });
      await page.waitForTimeout(600);
      return;
    } catch {
      // try next candidate
    }
  }

  try {
    await page.evaluate(() => {
      const nodes = Array.from(document.querySelectorAll('button,[role="button"],a'));
      const matcher = /提交|Submit/i;
      for (const node of nodes) {
        const text = `${node.textContent || ""}\n${node.getAttribute?.("aria-label") || ""}\n${node.getAttribute?.("title") || ""}`;
        if (!matcher.test(text)) {
          continue;
        }
        if (!(node instanceof HTMLElement)) {
          continue;
        }
        const style = window.getComputedStyle(node);
        const rect = node.getBoundingClientRect();
        if (style.display === "none" || style.visibility === "hidden" || rect.width <= 0 || rect.height <= 0) {
          continue;
        }
        node.scrollIntoView({ block: "center", inline: "center" });
        for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
          node.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, composed: true, view: window }));
        }
        if (typeof node.click === "function") {
          node.click();
        }
        return true;
      }
      return false;
    }).catch(() => undefined);
  } catch {
    // ignore
  }

  await page.keyboard.press(process.platform === "win32" ? "Control+Enter" : "Meta+Enter").catch(() => undefined);
  await page.waitForTimeout(200);
  await page.keyboard.press("Enter").catch(() => undefined);
}

const { runFetchPreviewOperation } = createFetchPreviewOwner({
  isFixtureCanvasBaseUrl,
  ensureProgramPage,
  tryOpenCanvasProxyPreview,
  isCanvasProxyPreviewFrameUrl,
  ensureCanvasProxyPreviewFrame,
  buildProgramHandleState,
});

const { runFetchPageMusicOperation } = createFetchPageMusicOwner({ buildProgramHandleState });

const { runFetchOperation } = createFetchExecutionOwner({
  DEFAULT_TIMEOUT_MS,
  buildGoogleFetchAuthHeaders,
  resolveProgramPageUrl,
  startNetworkCapture,
  runFetchPreviewOperation,
  runFetchPageMusicOperation,
  runFetchPageRequest,
  isFixtureCanvasBaseUrl,
  ensureProgramPage,
  buildProgramHandleState,
  ensureSharePage,
  ensureLoopbackConnectedClient,
  listConnectedClients,
  dispatchConnectedProxyRequest,
  log,
});

const { invokeGeminiCanvas } = createInvocationDispatcher({
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
});

const { main, sendJson, readJsonBody } = createBrowserPoolServerOwner({
  DEFAULT_HOST,
  DEFAULT_PORT,
  DEFAULT_TLS_PORT,
  evictIdleContexts,
  log,
  contexts,
  listConnectedClients,
  nextConnectedRequestId,
  connectedClients,
  cleanupConnectedClient,
  handleConnectedClientMessage,
  invokeGeminiCanvas,
});

if (process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN !== "1") {
  main().catch((error) => {
    console.error("[gemini-canvas-pool] fatal:", error);
    process.exit(1);
  });
}
