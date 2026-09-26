import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { parseBoolean } from "./gemini-canvas-browser-pool-executable.mjs";
import { collectProgramHandleSnapshot } from "./gemini-canvas-browser-pool-program-snapshot.mjs";
import {
  mergeProgramHandleHints, strongestProgramHandleHint,
} from "./gemini-canvas-browser-pool-program-handles.mjs";
import { mergeActionContract, extractCanvasProgramActionContractFromText } from "./gemini-canvas-browser-pool-action.mjs";
import { extractCanvasProxyClientHtmlFromTexts } from "./gemini-canvas-browser-pool-proxy-html.mjs";
import { hasPromptTextbox } from "./gemini-canvas-browser-pool-app.mjs";
import { submitPrompt } from "./gemini-canvas-browser-pool-composer.mjs";
import { createBootstrapResultOwner } from "./gemini-canvas-browser-pool-bootstrap-result.mjs";

export function createBootstrapExecutionOwner({
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
}) {
  const { finalizeBootstrapProgram } = createBootstrapResultOwner({
    mergeInvokeContract,
    buildCanvasProgramInvokeContract,
    buildProgramHandleState,
    clickMediaActionButton,
  });

  function normalizeBootstrapOperation(value) {
    const normalized = normalizeString(value)?.toLowerCase() ?? null;
    switch (normalized) {
      case "text":
      case "tts":
        return "text";
      case "image":
      case "music":
      case "video":
        return normalized;
      default:
        return "image";
    }
  }

  function defaultBootstrapPrompt(operation) {
    const suffix = Date.now();
    switch (operation) {
      case "text":
        return `CANVAS_PROGRAM_BOOTSTRAP_TEXT_${suffix} 请只回复 ok`;
      case "music":
        return `CANVAS_PROGRAM_BOOTSTRAP_MUSIC_${suffix} 一段简短电子提示音，节奏清晰。`;
      case "video":
        return `CANVAS_PROGRAM_BOOTSTRAP_VIDEO_${suffix} 3秒发光立方体旋转动画，干净背景。`;
      case "image":
      default:
        return `CANVAS_PROGRAM_BOOTSTRAP_IMAGE_${suffix} 一枚发光霓虹徽章，中央写着 CANVAS PROGRAM，科技感，高清细节，Requested aspect ratio: 1:1.`;
    }
  }

  async function runBootstrapProgramOperation(entry, args) {
    const baseUrl = normalizeString(args.baseUrl) ?? "https://gemini.google.com";
    const shareId = normalizeString(args.shareId);
    const directProgramPageUrl = resolveProgramPageUrl(baseUrl, args);
    const preferExistingProgramPage =
      parseBoolean(String(args.preferExistingProgramPage ?? ""), false) ||
      (!shareId && Boolean(directProgramPageUrl));
    const discoveryOnly = parseBoolean(String(args.discoveryOnly ?? ""), true);
    const launchCanvasProxyPreview =
      discoveryOnly && parseBoolean(String(args.launchCanvasProxyPreview ?? ""), false);
    if (!shareId && !directProgramPageUrl) {
      throw Object.assign(
        new Error("Gemini Canvas program bootstrap requires either shareId or a concrete program page."),
        {
          status: 400,
          code: "gemini_canvas_missing_share_id",
        },
      );
    }

    const bootstrapOperation = normalizeBootstrapOperation(
      args.bootstrapOperation ?? args.targetOperation ?? args.operation,
    );
    const timeoutMs = Math.max(
      Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
      operationConfig(bootstrapOperation).resultTimeoutMs,
    );
    const shareUrl = shareId ? `${baseUrl.replace(/\/+$/, "")}/share/${shareId}` : null;
    const bootstrapPrompt =
      normalizeString(args.bootstrapPrompt) ??
      defaultBootstrapPrompt(bootstrapOperation);
    const aggregateHints = {
      appPaths: [],
      conversationIds: [],
      responseIds: [],
      sharePaths: [],
    };
    let shareMaterializationRetry = null;

    let activePage = entry.page;
    let capture = startNetworkCapture(activePage, bootstrapOperation);
    // Resolve capture at call time because page adoption replaces its owner.
    const mergeSnapshot = (snapshot) => {
      mergeProgramHandleHints(aggregateHints, snapshot.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(snapshot.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          bootstrapOperation,
          capture.state.actionContract,
          capture.state.transportHints,
          snapshot,
          bootstrapPrompt,
          capture.state,
        ),
      );
    };
    const adoptActivePage = async (nextPage) => {
      if (!nextPage || nextPage === activePage) {
        return;
      }
      await capture.stop();
      capture = startNetworkCapture(nextPage, bootstrapOperation, capture.state);
      await capture.ready;
      const previousPage = activePage;
      activePage = nextPage;
      entry.page = nextPage;
      if (previousPage && previousPage !== nextPage && !previousPage.isClosed()) {
        await previousPage.close().catch(() => undefined);
      }
    };

    try {
      let shareFollow = {
        kind: preferExistingProgramPage ? "direct_program_page" : "unattempted",
        buttonText: null,
        samePage: false,
        page: null,
      };
      await capture.ready;

      if (preferExistingProgramPage) {
        await ensureProgramPage(entry, baseUrl, directProgramPageUrl, timeoutMs);
        activePage = entry.page;
        await activePage.waitForTimeout(2000);
      } else {
        await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
        await waitForShareSurface(activePage, Math.min(timeoutMs, 20_000));
        await activePage.waitForTimeout(1500);
        log("bootstrap_program collecting shareBefore snapshot", JSON.stringify({ pageUrl: activePage.url() }));
        const shareBefore = await collectProgramHandleSnapshot(activePage);
        log(
          "bootstrap_program collected shareBefore snapshot",
          JSON.stringify({
            pageUrl: shareBefore.url,
            bodyPreview: String(shareBefore.bodyText || "").slice(0, 600),
          }),
        );
        mergeSnapshot(shareBefore);
        log("bootstrap_program trying share entry follow", JSON.stringify({ pageUrl: activePage.url() }));
        shareFollow = await tryFollowShareEntryPoint(activePage);
        log(
          "bootstrap_program share entry follow result",
          JSON.stringify({
            kind: shareFollow.kind,
            pageUrl: shareFollow.page?.url?.() ?? activePage.url(),
          }),
        );
        if (shareFollow.page) {
          await adoptActivePage(shareFollow.page);
        }
        await activePage.waitForTimeout(3000);
        log("bootstrap_program collecting shareAfter snapshot", JSON.stringify({ pageUrl: activePage.url() }));
        const shareAfter = await collectProgramHandleSnapshot(activePage);
        log(
          "bootstrap_program collected shareAfter snapshot",
          JSON.stringify({
            pageUrl: shareAfter.url,
            bodyPreview: String(shareAfter.bodyText || "").slice(0, 600),
          }),
        );
        mergeSnapshot(shareAfter);

        if (activePage.url().includes("/share/") && !(await hasPromptTextbox(activePage))) {
          shareFollow = await tryFollowShareEntryPoint(activePage);
          if (shareFollow.page) {
            await adoptActivePage(shareFollow.page);
          }
          await activePage.waitForTimeout(3000);
        }

        if (
          !discoveryOnly &&
          !(await hasPromptTextbox(activePage)) &&
          !strongestProgramHandleHint(aggregateHints)
        ) {
          entry.page = activePage;
          await ensureAppPage(entry, baseUrl, timeoutMs);
          activePage = entry.page;
          await activePage.waitForTimeout(2000);
        }

        if (activePage.url().includes("/share/") && !(await hasPromptTextbox(activePage))) {
          throw Object.assign(
            new Error("Gemini Canvas share page did not materialize an editable program context."),
            {
              status: 502,
              code: "gemini_canvas_program_bootstrap_share_follow_failed",
            },
          );
        }
      }

      const before = await collectProgramHandleSnapshot(activePage);
      mergeSnapshot(before);

      if (!discoveryOnly && !(await hasPromptTextbox(activePage))) {
        await clickOperationMode(activePage, bootstrapOperation, timeoutMs).catch(() => false);
        await activePage.waitForTimeout(2000);
      }

      const shouldStayOnProxyDiscoverySurface = shouldStayOnCanvasProxyDiscoverySurface(
        discoveryOnly,
        before,
        capture.state,
      );
      if (
        shouldStayOnProxyDiscoverySurface &&
        !/Browser API Proxy Client/i.test(String(before.bodyText || ""))
      ) {
        shareMaterializationRetry = {
          attempted: true,
          initialPageUrl: activePage.url(),
          initialBodyPreview: String(before.bodyText || "").slice(0, 400),
        };
        await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
        await waitForShareSurface(activePage, Math.min(timeoutMs, 20_000));
        await activePage.waitForTimeout(1500);
        const retryFollow = await tryFollowShareEntryPoint(activePage);
        if (retryFollow.page) {
          await adoptActivePage(retryFollow.page);
        }
        await activePage.waitForTimeout(3000);
        const retrySnapshot = await collectProgramHandleSnapshot(activePage);
        mergeSnapshot(retrySnapshot);
        shareMaterializationRetry.shareFollowKind = retryFollow.kind ?? "unknown";
        shareMaterializationRetry.retryPageUrl = retrySnapshot.url;
        shareMaterializationRetry.retryBodyPreview = String(retrySnapshot.bodyText || "").slice(0, 400);
        shareMaterializationRetry.proxyVisibleAfterRetry = /Browser API Proxy Client/i.test(
          String(retrySnapshot.bodyText || ""),
        );
        log(
          "canvas proxy share materialization retry",
          JSON.stringify(shareMaterializationRetry),
        );
      }
      let canvasProxyPreview = null;
      let previewSnapshot = null;
      if (launchCanvasProxyPreview && shouldStayOnProxyDiscoverySurface) {
        canvasProxyPreview = await tryOpenCanvasProxyPreview(activePage, timeoutMs).catch((error) => ({
          clicked: false,
          reason: "preview_open_error",
          errorMessage: error instanceof Error ? error.message : String(error),
        }));
        const afterPreview = await collectProgramHandleSnapshot(activePage);
        mergeSnapshot(afterPreview);
        previewSnapshot = afterPreview;
        if (Number(canvasProxyPreview?.bridge?.eventCount ?? 0) <= 1) {
          const stampedFrames = await stampCanvasProxyPreviewFrames(
            activePage,
            inferGoogleAuthUser(activePage.url()),
          ).catch(() => []);
          canvasProxyPreview = {
            ...(canvasProxyPreview || {}),
            stampedFrames,
          };
          await activePage.waitForTimeout(2500);
          const afterStamp = await collectProgramHandleSnapshot(activePage);
          mergeSnapshot(afterStamp);
          previewSnapshot = afterStamp;
        }
        const hasCapturedCanvasProxyHtml = Boolean(
          extractCanvasProxyClientHtmlFromTexts(
            collectCanvasProxyContractTexts(previewSnapshot ?? afterPreview, capture.state),
          ),
        );
        const shouldAttemptDirectCanvasProxyLaunch = canvasProxyPreviewNeedsDirectLaunch(
          canvasProxyPreview,
          afterPreview.bodyText,
          hasCapturedCanvasProxyHtml,
        );
        if (shouldAttemptDirectCanvasProxyLaunch) {
          const directLaunch = await tryLaunchCanvasProxyClientFromCapturedHtml(
            entry.context,
            activePage,
            afterPreview,
            capture.state,
            timeoutMs,
          );
          const adoptedLaunchPage = directLaunch?.page ?? null;
          canvasProxyPreview = {
            ...(canvasProxyPreview || {}),
            directLaunch: directLaunch
              ? Object.fromEntries(
                  Object.entries(directLaunch).filter(([key]) => key !== "page"),
                )
              : null,
          };
          if (adoptedLaunchPage) {
            await adoptActivePage(adoptedLaunchPage);
            previewSnapshot = await collectProgramHandleSnapshot(activePage);
          }
        }
        log(
          "canvas proxy preview probe",
          JSON.stringify({
            operation: bootstrapOperation,
            preview: canvasProxyPreview,
            pageUrl: previewSnapshot?.url ?? afterPreview.url,
            bodyPreview: String((previewSnapshot?.bodyText ?? afterPreview.bodyText) || "").slice(0, 400),
          }),
        );
        if (discoveryOnly && canvasProxyPreview?.directLaunch?.launched) {
          return buildBootstrapPreviewResult({
            baseUrl, args, previewSnapshot, afterPreview, captureState: capture.state, aggregateHints,
            bootstrapOperation, bootstrapPrompt, shareUrl, shareId, shareFollow,
            discoveryOnly, canvasProxyPreview, shareMaterializationRetry, before,
          });
        }
      }
      const newChatClicked = shouldStayOnProxyDiscoverySurface ? false : await clickNewChat(activePage);
      const modeSelected = shouldStayOnProxyDiscoverySurface
        ? false
        : bootstrapOperation === "text"
          ? true
          : await clickOperationMode(activePage, bootstrapOperation, timeoutMs);
      const afterNewChat = shouldStayOnProxyDiscoverySurface
        ? before
        : await collectProgramHandleSnapshot(activePage);
      mergeSnapshot(afterNewChat);

      if (
        !discoveryOnly &&
        bootstrapOperation === "music" &&
        /选择要混合制作的曲目/i.test(afterNewChat.bodyText)
      ) {
        const styleSelection = await trySelectMusicStyleCard(activePage, timeoutMs).catch(() => null);
        if (styleSelection?.clicked) {
          const afterStyleSelection = await collectProgramHandleSnapshot(activePage);
          mergeSnapshot(afterStyleSelection);
        }
      }

      if (!discoveryOnly) {
        await submitPrompt(activePage, bootstrapPrompt, timeoutMs);
      }

      let lastSnapshot = await pollBootstrapProgram({
        activePage, captureState: capture.state, baseUrl, args, bootstrapOperation, discoveryOnly,
        timeoutMs, initialSnapshot: previewSnapshot ?? afterNewChat, mergeSnapshot,
      });

      // Await all final media probes before the outer finally stops capture.
      return await finalizeBootstrapProgram({
        activePage, capture, baseUrl, args, bootstrapOperation, bootstrapPrompt,
        timeoutMs, discoveryOnly, lastSnapshot, aggregateHints, mergeSnapshot,
        shareUrl, shareId, shareFollow, before, canvasProxyPreview,
        shareMaterializationRetry, newChatClicked, modeSelected,
      });
    } finally {
      await capture.stop();
    }
  }

  return { normalizeBootstrapOperation, defaultBootstrapPrompt, runBootstrapProgramOperation };
}
